use crate::domain::aggregates::Symbol;
use crate::domain::traits::code_intelligence::{
    CodeIntelligenceError, CodeIntelligenceProvider, DocumentSymbol, HoverInfo, PrecisionTier,
    ProviderDiagnostic, ProviderOutcome, Reference, TieredCodeIntelligenceProvider, TieredOutcome,
    TypeHierarchy,
};
use crate::domain::value_objects::Location;
use crate::infrastructure::graph::LightweightIndex;
use crate::infrastructure::lsp::error::{LspProcessError, ProgressCallback};
use crate::infrastructure::lsp::providers::fallback::TreesitterFallbackProvider;
use crate::infrastructure::lsp::providers::lsp::LspIntelligenceProvider;
use crate::infrastructure::parser::Language;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::{debug, warn};

/// Fixed highest-first pipeline order (LSI M4): the precedence every op's
/// walk follows when selecting its support-matrix tiers — each op attempts
/// only its eligible tiers and silently skips the rest — and the ordering
/// of the `status()` counter rows.
pub const TIER_ORDER: [PrecisionTier; 3] =
    [PrecisionTier::S2, PrecisionTier::S1, PrecisionTier::S0];

/// Per-tier enablement policy for the composite pipeline.
///
/// A policy-disabled tier is skipped entirely: no attempt, no diagnostic,
/// no counter increment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierPolicy {
    /// Allow the S2 LSP tier to be attempted.
    pub lsp: bool,
    /// Allow the S1 local-resolver tier to be attempted.
    pub local_resolver: bool,
    /// Allow the S0 tree-sitter tier to be attempted.
    pub tree_sitter: bool,
    /// Bounded readiness for an S2 attempt that has an enabled lower tier:
    /// the readiness gate waits at most this long before falling through
    /// (W3, default 2s). Irrelevant when the lower eligible tier is
    /// policy-disabled — then the full `wait_timeout_secs` applies.
    pub fallback_readiness: Duration,
}

impl TierPolicy {
    /// The default bound on an S2 readiness attempt that has a lower tier
    /// fallback.
    pub const DEFAULT_FALLBACK_READINESS: Duration = Duration::from_secs(2);

    /// Every pipeline tier enabled (the default for the existing ctors).
    pub const fn all() -> Self {
        Self {
            lsp: true,
            local_resolver: true,
            tree_sitter: true,
            fallback_readiness: Self::DEFAULT_FALLBACK_READINESS,
        }
    }

    /// Whether `tier` participates in the pipeline under this policy.
    /// Reserved tiers (S3/S4) have no producer and are never enabled.
    pub const fn enabled(self, tier: PrecisionTier) -> bool {
        match tier {
            PrecisionTier::S2 => self.lsp,
            PrecisionTier::S1 => self.local_resolver,
            PrecisionTier::S0 => self.tree_sitter,
            PrecisionTier::S3 | PrecisionTier::S4 => false,
        }
    }

    /// The S2 readiness budget for an op whose eligible lower tier is
    /// `fallback_tier`: the smaller of the configured full wait and
    /// [`Self::fallback_readiness`] when that tier is enabled, the full wait
    /// when it is policy-disabled (nothing below can serve, so S2 gets the
    /// whole budget).
    pub fn readiness_budget(
        self,
        wait_timeout_secs: u64,
        fallback_tier: PrecisionTier,
    ) -> Duration {
        let full_wait = Duration::from_secs(wait_timeout_secs);
        if self.enabled(fallback_tier) {
            full_wait.min(self.fallback_readiness)
        } else {
            full_wait
        }
    }
}

impl Default for TierPolicy {
    fn default() -> Self {
        Self::all()
    }
}

/// Lock-free per-tier attempt counters.
#[derive(Debug, Default)]
struct TierAtomics {
    attempts: AtomicU64,
    served: AtomicU64,
    unavailable: AtomicU64,
    errors: AtomicU64,
    degraded: AtomicU64,
}

impl TierAtomics {
    /// Records one attempted tier that did not serve.
    fn record(&self, outcome: ProviderOutcome) {
        self.attempts.fetch_add(1, Ordering::Relaxed);
        let bucket = match outcome {
            ProviderOutcome::Unavailable => &self.unavailable,
            ProviderOutcome::Error => &self.errors,
            ProviderOutcome::Degraded => &self.degraded,
        };
        bucket.fetch_add(1, Ordering::Relaxed);
    }

    /// Records one attempted tier that served a result.
    fn record_served(&self) {
        self.attempts.fetch_add(1, Ordering::Relaxed);
        self.served.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self, tier: PrecisionTier) -> TierCounters {
        TierCounters {
            tier,
            attempts: self.attempts.load(Ordering::Relaxed),
            served: self.served.load(Ordering::Relaxed),
            unavailable: self.unavailable.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            degraded: self.degraded.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of one tier's attempt counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierCounters {
    /// The tier these counters belong to.
    pub tier: PrecisionTier,
    /// Total attempts made against the tier.
    pub attempts: u64,
    /// Attempts that produced a served result.
    pub served: u64,
    /// Attempts that found the provider unavailable.
    pub unavailable: u64,
    /// Attempts that failed with an error.
    pub errors: u64,
    /// Attempts that returned a degraded (empty) result.
    pub degraded: u64,
}

/// Observable composite pipeline status: one counter row per pipeline tier,
/// in fixed highest-first order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeStatus {
    /// Counter rows for the pipeline tiers (S2, S1, S0).
    pub tiers: Vec<TierCounters>,
}

impl CompositeStatus {
    /// The counter row for `tier`, when the tier participates in the pipeline.
    pub fn tier(&self, tier: PrecisionTier) -> Option<&TierCounters> {
        self.tiers.iter().find(|row| row.tier == tier)
    }
}

fn tier_index(tier: PrecisionTier) -> usize {
    match tier {
        PrecisionTier::S0 => 0,
        PrecisionTier::S1 => 1,
        PrecisionTier::S2 => 2,
        PrecisionTier::S3 | PrecisionTier::S4 => usize::MAX,
    }
}

/// Internal result of one tier attempt.
enum Attempt<T> {
    /// The tier served a value.
    Served(T),
    /// The tier answered with a degraded (empty) result; fall through.
    Degraded(ProviderDiagnostic),
    /// The tier did not serve: a readiness-gate failure (no provider answer)
    /// or a provider `Err`, whose originating error variant is preserved for
    /// the old-trait deepest-answer mapping (W2).
    Failed(ProviderDiagnostic, Option<CodeIntelligenceError>),
}

/// The deepest attempted tier's answer, used by the old-trait mapping of
/// `get_definition`/`hover` (W2).
enum DeepestAnswer {
    /// No provider `Err` reached the deepest attempt: a clean miss, a
    /// readiness-gate failure, or no attempt at all.
    Miss,
    /// The deepest attempted provider returned this error.
    Errored(CodeIntelligenceError),
}

/// One `get_definition`/`hover` chain run: the public tiered outcome plus
/// the deepest attempted provider answer.
struct ChainOutcome<T> {
    outcome: TieredOutcome<T>,
    deepest: DeepestAnswer,
}

pub struct CompositeProvider {
    lsp: LspIntelligenceProvider,
    fallback: TreesitterFallbackProvider,
    policy: TierPolicy,
    wait_timeout_secs: u64,
    counters: [TierAtomics; 3],
}

impl CompositeProvider {
    /// Counter slot for `tier` (pipeline tiers only).
    fn counters_for(&self, tier: PrecisionTier) -> &TierAtomics {
        &self.counters[tier_index(tier)]
    }

    /// Records a non-serving attempt and emits its structured diagnostic.
    fn record_failure(&self, op: &'static str, diagnostic: &ProviderDiagnostic) {
        let tier = diagnostic.attempted_tier;
        match diagnostic.outcome {
            ProviderOutcome::Degraded => debug!(
                op,
                tier = %tier,
                provider = %diagnostic.provider,
                outcome = %diagnostic.outcome,
                message = %diagnostic.message,
                "tier degraded; falling through"
            ),
            ProviderOutcome::Unavailable | ProviderOutcome::Error => warn!(
                op,
                tier = %tier,
                provider = %diagnostic.provider,
                outcome = %diagnostic.outcome,
                message = %diagnostic.message,
                "tier failed; falling through"
            ),
        }
        self.counters_for(tier).record(diagnostic.outcome);
    }

    /// Records a served attempt.
    fn record_served(&self, tier: PrecisionTier) {
        self.counters_for(tier).record_served();
    }

    /// Records a served attempt and wraps the value with its serving tier
    /// and the diagnostics of the preceding fall-throughs.
    fn serve<T>(
        &self,
        value: T,
        tier: PrecisionTier,
        diagnostics: Vec<ProviderDiagnostic>,
    ) -> TieredOutcome<T> {
        self.record_served(tier);
        TieredOutcome::served_with(value, tier, diagnostics)
    }

    /// Build a LightweightIndex for the given workspace root
    fn build_index(workspace_root: &Path) -> Arc<LightweightIndex> {
        let mut index = LightweightIndex::new();
        index.build_index(workspace_root).ok();
        Arc::new(index)
    }

    /// Builds a provider with every tier enabled, a 30s full readiness wait
    /// and the default bounded fallback (W3: an S2 attempt with an enabled
    /// lower tier waits at most `fallback_readiness`, 2s).
    pub fn new(workspace_root: &Path) -> Self {
        Self::with_policy_and_timeout(workspace_root, TierPolicy::all(), 30)
    }

    /// Builds a provider with every tier enabled, bounding the full LSP
    /// readiness wait. The bounded fallback of the policy still applies: an
    /// S2 attempt with an enabled lower tier waits at most
    /// `min(timeout_secs, policy.fallback_readiness)`.
    pub fn with_wait_timeout(workspace_root: &Path, timeout_secs: u64) -> Self {
        Self::with_policy_and_timeout(workspace_root, TierPolicy::all(), timeout_secs)
    }

    /// Builds a provider with an explicit per-tier enablement policy.
    pub fn with_policy(workspace_root: &Path, policy: TierPolicy) -> Self {
        Self::with_policy_and_timeout(workspace_root, policy, 30)
    }

    fn with_policy_and_timeout(
        workspace_root: &Path,
        policy: TierPolicy,
        timeout_secs: u64,
    ) -> Self {
        let arc_index = Self::build_index(workspace_root);
        Self {
            lsp: LspIntelligenceProvider::new(workspace_root),
            fallback: TreesitterFallbackProvider::with_index(arc_index.clone()),
            policy,
            wait_timeout_secs: timeout_secs,
            counters: std::array::from_fn(|_| TierAtomics::default()),
        }
    }

    pub fn with_progress_callback<F>(workspace_root: &Path, _callback: F) -> Self
    where
        F: ProgressCallback,
    {
        Self::with_policy_and_timeout(workspace_root, TierPolicy::all(), 30)
    }

    pub fn wait_timeout_secs(&self) -> u64 {
        self.wait_timeout_secs
    }

    /// The per-tier policy this provider was built with.
    pub fn policy(&self) -> TierPolicy {
        self.policy
    }

    /// Snapshot of the per-tier attempt counters in fixed highest-first
    /// pipeline order (S2, S1, S0).
    pub fn status(&self) -> CompositeStatus {
        CompositeStatus {
            tiers: TIER_ORDER
                .iter()
                .map(|tier| self.counters_for(*tier).snapshot(*tier))
                .collect(),
        }
    }

    fn language_from_location(location: &Location) -> Option<Language> {
        Language::from_extension(Path::new(location.file()).extension())
    }

    async fn wait_for_lsp_ready(
        &self,
        language: Language,
        progress_callback: Option<Box<dyn ProgressCallback>>,
    ) -> Result<(), LspProcessError> {
        let pm = self.lsp.process_manager();
        match pm
            .wait_for_ready(language, self.wait_timeout_secs, progress_callback)
            .await
        {
            Ok(status) => {
                if status.is_ready() {
                    Ok(())
                } else {
                    Err(LspProcessError::ServerNotReady {
                        language: language.name().to_string(),
                        status,
                        waited_secs: self.wait_timeout_secs,
                    })
                }
            }
            Err(e) => Err(e),
        }
    }
}

/// Attempt helpers for the fixed tier pipeline (D2).
///
/// Per-op support matrix:
///
/// | op | S2 (LSP) | S1 (local resolver) | S0 (tree-sitter) |
/// |----|----------|---------------------|------------------|
/// | `get_symbols` | yes | — | yes |
/// | `find_references` | yes | — | yes |
/// | `get_hierarchy` | yes | — | yes |
/// | `get_definition` | yes (authoritative `Ok(None)`) | yes | — |
/// | `get_document_symbols` | yes | — | yes |
/// | `hover` | yes | — | yes |
///
/// A successful-but-empty result from a non-terminal tier is `Degraded` and
/// falls through (today's serving semantics); the same result from the
/// terminal tier is a served answer. Unsupported tiers for an op are skipped
/// silently, exactly like policy-disabled tiers.
impl CompositeProvider {
    /// Summary error for an op whose eligible tiers were all exhausted.
    fn exhausted(op: &str, diagnostics: &[ProviderDiagnostic]) -> CodeIntelligenceError {
        let detail = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; ");
        CodeIntelligenceError::Internal(format!("{op}: all tiers exhausted ({detail})"))
    }

    fn diagnostic(
        tier: PrecisionTier,
        provider: &str,
        outcome: ProviderOutcome,
        message: impl Into<String>,
    ) -> ProviderDiagnostic {
        ProviderDiagnostic::new(provider, tier, outcome, message)
    }

    /// Readiness gate for an S2 attempt driven by a file path.
    ///
    /// `fallback_tier` is the op's lower eligible tier (fixed support
    /// matrix): the readiness wait is bounded when that tier is enabled.
    async fn gate_lsp_for_path(
        &self,
        path: &Path,
        op: &'static str,
        fallback_tier: PrecisionTier,
    ) -> Result<(), ProviderDiagnostic> {
        match Language::from_extension(path.extension()) {
            Some(language) => {
                self.gate_lsp_language(language, op, path.display().to_string(), fallback_tier)
                    .await
            }
            None => Err(Self::diagnostic(
                PrecisionTier::S2,
                LspIntelligenceProvider::PROVIDER_ID,
                ProviderOutcome::Unavailable,
                format!("{op}: unsupported language for {}", path.display()),
            )),
        }
    }

    /// Readiness gate for an S2 attempt driven by a location.
    async fn gate_lsp_for_location(
        &self,
        location: &Location,
        op: &'static str,
        fallback_tier: PrecisionTier,
    ) -> Result<(), ProviderDiagnostic> {
        match Self::language_from_location(location) {
            Some(language) => {
                self.gate_lsp_language(language, op, location.file().to_string(), fallback_tier)
                    .await
            }
            None => Err(Self::diagnostic(
                PrecisionTier::S2,
                LspIntelligenceProvider::PROVIDER_ID,
                ProviderOutcome::Unavailable,
                format!("{op}: unsupported language for {}", location.file()),
            )),
        }
    }

    async fn gate_lsp_language(
        &self,
        language: Language,
        op: &'static str,
        file: String,
        fallback_tier: PrecisionTier,
    ) -> Result<(), ProviderDiagnostic> {
        let budget = self
            .policy
            .readiness_budget(self.wait_timeout_secs, fallback_tier);
        match Self::readiness_within(budget, self.wait_for_lsp_ready(language, None)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(Self::diagnostic(
                PrecisionTier::S2,
                LspIntelligenceProvider::PROVIDER_ID,
                ProviderOutcome::Unavailable,
                format!("{op}: {file}: {error}"),
            )),
            // The bound expired mid-readiness — typically inside
            // `LspProcess::initialize`, whose own request timeout (30s) the
            // `wait_timeout_secs` poll loop cannot bound. The dropped future
            // was never registered with the process manager (registration
            // happens only after `initialize` succeeds), so a later query
            // re-spawns from scratch; no lock is held across the drop.
            Err(_expired) => Err(Self::diagnostic(
                PrecisionTier::S2,
                LspIntelligenceProvider::PROVIDER_ID,
                ProviderOutcome::Unavailable,
                format!(
                    "{op}: {file}: LSP readiness exceeded the {:.3}s bounded fallback to \
                     {fallback_tier}",
                    budget.as_secs_f64()
                ),
            )),
        }
    }

    /// Awaits `readiness` under `budget` — the W3 fall-through bound on an
    /// S2 readiness attempt that has a lower eligible tier.
    async fn readiness_within<F>(
        budget: Duration,
        readiness: F,
    ) -> Result<Result<(), LspProcessError>, tokio::time::error::Elapsed>
    where
        F: std::future::Future<Output = Result<(), LspProcessError>>,
    {
        tokio::time::timeout(budget, readiness).await
    }

    // ── S2 (LSP) attempts ───────────────────────────────────────────────

    async fn attempt_lsp_get_symbols(&self, path: &Path) -> Attempt<Vec<Symbol>> {
        if let Err(diagnostic) = self
            .gate_lsp_for_path(path, "get_symbols", PrecisionTier::S0)
            .await
        {
            return Attempt::Failed(diagnostic, None);
        }
        match self.lsp.get_symbols(path).await {
            Ok(symbols) if !symbols.is_empty() => Attempt::Served(symbols),
            Ok(_) => Attempt::Degraded(Self::diagnostic(
                PrecisionTier::S2,
                LspIntelligenceProvider::PROVIDER_ID,
                ProviderOutcome::Degraded,
                "get_symbols: LSP returned no symbols",
            )),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S2,
                    LspIntelligenceProvider::PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("get_symbols: {error}"),
                ),
                Some(error),
            ),
        }
    }

    async fn attempt_lsp_find_references(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> Attempt<Vec<Reference>> {
        if let Err(diagnostic) = self
            .gate_lsp_for_location(location, "find_references", PrecisionTier::S0)
            .await
        {
            return Attempt::Failed(diagnostic, None);
        }
        match self
            .lsp
            .find_references(location, include_declaration)
            .await
        {
            Ok(references) if !references.is_empty() => Attempt::Served(references),
            Ok(_) => Attempt::Degraded(Self::diagnostic(
                PrecisionTier::S2,
                LspIntelligenceProvider::PROVIDER_ID,
                ProviderOutcome::Degraded,
                "find_references: LSP returned no references",
            )),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S2,
                    LspIntelligenceProvider::PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("find_references: {error}"),
                ),
                Some(error),
            ),
        }
    }

    async fn attempt_lsp_hierarchy(&self, location: &Location) -> Attempt<TypeHierarchy> {
        if let Err(diagnostic) = self
            .gate_lsp_for_location(location, "get_hierarchy", PrecisionTier::S0)
            .await
        {
            return Attempt::Failed(diagnostic, None);
        }
        match self.lsp.get_hierarchy(location).await {
            Ok(hierarchy) => Attempt::Served(hierarchy),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S2,
                    LspIntelligenceProvider::PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("get_hierarchy: {error}"),
                ),
                Some(error),
            ),
        }
    }

    async fn attempt_lsp_definition(&self, location: &Location) -> Attempt<Option<Location>> {
        // S1 is the definition op's lower eligible tier (fixed matrix).
        if let Err(diagnostic) = self
            .gate_lsp_for_location(location, "get_definition", PrecisionTier::S1)
            .await
        {
            return Attempt::Failed(diagnostic, None);
        }
        match self.lsp.get_definition(location).await {
            // LSP `Ok(None)` is an authoritative unresolved answer: no
            // lower tier may second-guess it (composite.rs historical rule).
            Ok(definition) => Attempt::Served(definition),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S2,
                    LspIntelligenceProvider::PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("get_definition: {error}"),
                ),
                Some(error),
            ),
        }
    }

    async fn attempt_lsp_document_symbols(&self, path: &Path) -> Attempt<Vec<DocumentSymbol>> {
        if let Err(diagnostic) = self
            .gate_lsp_for_path(path, "get_document_symbols", PrecisionTier::S0)
            .await
        {
            return Attempt::Failed(diagnostic, None);
        }
        match self.lsp.get_document_symbols(path).await {
            Ok(symbols) if !symbols.is_empty() => Attempt::Served(symbols),
            Ok(_) => Attempt::Degraded(Self::diagnostic(
                PrecisionTier::S2,
                LspIntelligenceProvider::PROVIDER_ID,
                ProviderOutcome::Degraded,
                "get_document_symbols: LSP returned no symbols",
            )),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S2,
                    LspIntelligenceProvider::PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("get_document_symbols: {error}"),
                ),
                Some(error),
            ),
        }
    }

    async fn attempt_lsp_hover(&self, location: &Location) -> Attempt<Option<HoverInfo>> {
        if let Err(diagnostic) = self
            .gate_lsp_for_location(location, "hover", PrecisionTier::S0)
            .await
        {
            return Attempt::Failed(diagnostic, None);
        }
        match self.lsp.hover(location).await {
            Ok(Some(info)) if !info.content.is_empty() => {
                tracing::debug!("LSP hover returned: {}", info.content);
                Attempt::Served(Some(info))
            }
            Ok(Some(_)) => Attempt::Degraded(Self::diagnostic(
                PrecisionTier::S2,
                LspIntelligenceProvider::PROVIDER_ID,
                ProviderOutcome::Degraded,
                "hover: LSP returned empty content",
            )),
            Ok(None) => Attempt::Degraded(Self::diagnostic(
                PrecisionTier::S2,
                LspIntelligenceProvider::PROVIDER_ID,
                ProviderOutcome::Degraded,
                "hover: LSP returned no hover",
            )),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S2,
                    LspIntelligenceProvider::PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("hover: {error}"),
                ),
                Some(error),
            ),
        }
    }

    // ── S1 (local resolver) attempts ────────────────────────────────────

    async fn attempt_local_resolver_definition(
        &self,
        location: &Location,
    ) -> Attempt<Option<Location>> {
        match self.fallback.get_definition(location).await {
            Ok(Some(found)) => Attempt::Served(Some(found)),
            Ok(None) => Attempt::Degraded(Self::diagnostic(
                PrecisionTier::S1,
                TreesitterFallbackProvider::LOCAL_RESOLVER_PROVIDER_ID,
                ProviderOutcome::Degraded,
                "get_definition: local resolver found no definition",
            )),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S1,
                    TreesitterFallbackProvider::LOCAL_RESOLVER_PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("get_definition: {error}"),
                ),
                Some(error),
            ),
        }
    }

    // ── S0 (tree-sitter) attempts: terminal tier for their ops ──────────

    async fn attempt_tree_sitter_get_symbols(&self, path: &Path) -> Attempt<Vec<Symbol>> {
        match self.fallback.get_symbols(path).await {
            Ok(symbols) => Attempt::Served(symbols),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S0,
                    TreesitterFallbackProvider::PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("get_symbols: {error}"),
                ),
                Some(error),
            ),
        }
    }

    async fn attempt_tree_sitter_find_references(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> Attempt<Vec<Reference>> {
        match self
            .fallback
            .find_references(location, include_declaration)
            .await
        {
            Ok(references) => Attempt::Served(references),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S0,
                    TreesitterFallbackProvider::PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("find_references: {error}"),
                ),
                Some(error),
            ),
        }
    }

    async fn attempt_tree_sitter_hierarchy(&self, location: &Location) -> Attempt<TypeHierarchy> {
        match self.fallback.get_hierarchy(location).await {
            Ok(hierarchy) => Attempt::Served(hierarchy),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S0,
                    TreesitterFallbackProvider::PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("get_hierarchy: {error}"),
                ),
                Some(error),
            ),
        }
    }

    async fn attempt_tree_sitter_document_symbols(
        &self,
        path: &Path,
    ) -> Attempt<Vec<DocumentSymbol>> {
        match self.fallback.get_document_symbols(path).await {
            Ok(symbols) => Attempt::Served(symbols),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S0,
                    TreesitterFallbackProvider::PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("get_document_symbols: {error}"),
                ),
                Some(error),
            ),
        }
    }

    async fn attempt_tree_sitter_hover(&self, location: &Location) -> Attempt<Option<HoverInfo>> {
        match self.fallback.hover(location).await {
            Ok(hover) => Attempt::Served(hover),
            Err(error) => Attempt::Failed(
                Self::diagnostic(
                    PrecisionTier::S0,
                    TreesitterFallbackProvider::PROVIDER_ID,
                    ProviderOutcome::Error,
                    format!("hover: {error}"),
                ),
                Some(error),
            ),
        }
    }

    // ── Definition/hover chains: deepest-answer tracking (W2) ───────────

    /// Runs the definition chain (S2 → S1) and reports the public tiered
    /// outcome together with the deepest attempted provider answer.
    async fn definition_chain(&self, location: &Location) -> ChainOutcome<Option<Location>> {
        let mut diagnostics = Vec::new();
        let mut deepest = DeepestAnswer::Miss;

        if self.policy.enabled(PrecisionTier::S2) {
            match self.attempt_lsp_definition(location).await {
                // `Ok(None)` is served at S2 and never falls through.
                Attempt::Served(value) => {
                    return ChainOutcome {
                        outcome: self.serve(value, PrecisionTier::S2, diagnostics),
                        deepest,
                    };
                }
                Attempt::Degraded(d) => {
                    self.record_failure("get_definition", &d);
                    deepest = DeepestAnswer::Miss;
                    diagnostics.push(d);
                }
                Attempt::Failed(d, error) => {
                    self.record_failure("get_definition", &d);
                    if let Some(error) = error {
                        deepest = DeepestAnswer::Errored(error);
                    }
                    diagnostics.push(d);
                }
            }
        }

        if self.policy.enabled(PrecisionTier::S1) {
            match self.attempt_local_resolver_definition(location).await {
                Attempt::Served(value) => {
                    return ChainOutcome {
                        outcome: self.serve(value, PrecisionTier::S1, diagnostics),
                        deepest,
                    };
                }
                // S1 is the lowest definition-capable tier: a resolver
                // `Ok(None)` exhausts the definition chain.
                Attempt::Degraded(d) => {
                    self.record_failure("get_definition", &d);
                    deepest = DeepestAnswer::Miss;
                    diagnostics.push(d);
                }
                Attempt::Failed(d, error) => {
                    self.record_failure("get_definition", &d);
                    if let Some(error) = error {
                        deepest = DeepestAnswer::Errored(error);
                    }
                    diagnostics.push(d);
                }
            }
        }

        ChainOutcome {
            outcome: TieredOutcome::Unresolved(diagnostics),
            deepest,
        }
    }

    /// Runs the hover chain (S2 → S0, the latter terminal) and reports the
    /// public tiered outcome together with the deepest provider answer.
    async fn hover_chain(&self, location: &Location) -> ChainOutcome<Option<HoverInfo>> {
        let mut diagnostics = Vec::new();
        let mut deepest = DeepestAnswer::Miss;

        if self.policy.enabled(PrecisionTier::S2) {
            match self.attempt_lsp_hover(location).await {
                Attempt::Served(value) => {
                    return ChainOutcome {
                        outcome: self.serve(value, PrecisionTier::S2, diagnostics),
                        deepest,
                    };
                }
                Attempt::Degraded(d) => {
                    self.record_failure("hover", &d);
                    deepest = DeepestAnswer::Miss;
                    diagnostics.push(d);
                }
                Attempt::Failed(d, error) => {
                    self.record_failure("hover", &d);
                    if let Some(error) = error {
                        deepest = DeepestAnswer::Errored(error);
                    }
                    diagnostics.push(d);
                }
            }
        }

        if self.policy.enabled(PrecisionTier::S0) {
            match self.attempt_tree_sitter_hover(location).await {
                // Terminal tier: even a clean miss is the chain's answer.
                Attempt::Served(value) => {
                    return ChainOutcome {
                        outcome: self.serve(value, PrecisionTier::S0, diagnostics),
                        deepest,
                    };
                }
                Attempt::Degraded(d) => {
                    self.record_failure("hover", &d);
                    deepest = DeepestAnswer::Miss;
                    diagnostics.push(d);
                }
                Attempt::Failed(d, error) => {
                    self.record_failure("hover", &d);
                    if let Some(error) = error {
                        deepest = DeepestAnswer::Errored(error);
                    }
                    diagnostics.push(d);
                }
            }
        }

        ChainOutcome {
            outcome: TieredOutcome::Unresolved(diagnostics),
            deepest,
        }
    }
}

/// Old-shape mapping of the pipeline outcome: the six-op contract survives
/// unchanged for `Arc<dyn CodeIntelligenceProvider>` consumers
/// (`workspace_session`, `lsp_handlers`, CLI, proxy service).
///
/// `Served` maps to the value. `Unresolved` for the collection ops maps to
/// a diagnostic `Err(Internal)`; `get_definition`/`hover` take the deepest
/// attempted tier's answer — a provider `Err` keeps its original variant
/// (W2: the pre-e39 passthrough), a clean miss or a gate-only failure stays
/// `Ok(None)`.
#[async_trait::async_trait]
impl CodeIntelligenceProvider for CompositeProvider {
    async fn get_symbols(&self, path: &Path) -> Result<Vec<Symbol>, CodeIntelligenceError> {
        match self.get_symbols_tiered(path).await {
            TieredOutcome::Served(tiered) => Ok(tiered.value),
            TieredOutcome::Unresolved(diagnostics) => {
                Err(Self::exhausted("get_symbols", &diagnostics))
            }
        }
    }

    async fn find_references(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> Result<Vec<Reference>, CodeIntelligenceError> {
        match self
            .find_references_tiered(location, include_declaration)
            .await
        {
            TieredOutcome::Served(tiered) => Ok(tiered.value),
            TieredOutcome::Unresolved(diagnostics) => {
                Err(Self::exhausted("find_references", &diagnostics))
            }
        }
    }

    async fn get_hierarchy(
        &self,
        location: &Location,
    ) -> Result<TypeHierarchy, CodeIntelligenceError> {
        match self.get_hierarchy_tiered(location).await {
            TieredOutcome::Served(tiered) => Ok(tiered.value),
            TieredOutcome::Unresolved(diagnostics) => {
                Err(Self::exhausted("get_hierarchy", &diagnostics))
            }
        }
    }

    async fn get_definition(
        &self,
        location: &Location,
    ) -> Result<Option<Location>, CodeIntelligenceError> {
        let chain = self.definition_chain(location).await;
        match chain.outcome {
            TieredOutcome::Served(tiered) => Ok(tiered.value),
            // W2: propagate the deepest attempted provider's original error
            // variant; a clean miss or a gate-only failure stays `Ok(None)`.
            TieredOutcome::Unresolved(_) => match chain.deepest {
                DeepestAnswer::Errored(error) => Err(error),
                DeepestAnswer::Miss => Ok(None),
            },
        }
    }

    async fn get_document_symbols(
        &self,
        path: &Path,
    ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
        match self.get_document_symbols_tiered(path).await {
            TieredOutcome::Served(tiered) => Ok(tiered.value),
            TieredOutcome::Unresolved(diagnostics) => {
                Err(Self::exhausted("get_document_symbols", &diagnostics))
            }
        }
    }

    async fn hover(&self, location: &Location) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
        let chain = self.hover_chain(location).await;
        match chain.outcome {
            TieredOutcome::Served(tiered) => Ok(tiered.value),
            // W2: same deepest-answer rule as `get_definition`.
            TieredOutcome::Unresolved(_) => match chain.deepest {
                DeepestAnswer::Errored(error) => Err(error),
                DeepestAnswer::Miss => Ok(None),
            },
        }
    }
}

/// The fixed highest-first, policy-gated tier pipeline (D2/D3).
///
/// Each op walks [`TIER_ORDER`] and attempts only the tiers that support it.
/// A serving tier is declared on the result, together with the diagnostics
/// of every failed/degraded attempt that preceded it. Tier counters are
/// updated per attempt; every fall-through emits structured `tracing`.
#[async_trait::async_trait]
impl TieredCodeIntelligenceProvider for CompositeProvider {
    async fn get_symbols_tiered(&self, path: &Path) -> TieredOutcome<Vec<Symbol>> {
        let mut diagnostics = Vec::new();

        if self.policy.enabled(PrecisionTier::S2) {
            match self.attempt_lsp_get_symbols(path).await {
                Attempt::Served(value) => {
                    return self.serve(value, PrecisionTier::S2, diagnostics);
                }
                Attempt::Degraded(d) | Attempt::Failed(d, _) => {
                    self.record_failure("get_symbols", &d);
                    diagnostics.push(d);
                }
            }
        }
        // S1 has no get_symbols op (fixed support matrix): skipped silently.

        if self.policy.enabled(PrecisionTier::S0) {
            match self.attempt_tree_sitter_get_symbols(path).await {
                Attempt::Served(value) => {
                    return self.serve(value, PrecisionTier::S0, diagnostics);
                }
                Attempt::Degraded(d) | Attempt::Failed(d, _) => {
                    self.record_failure("get_symbols", &d);
                    diagnostics.push(d);
                }
            }
        }

        TieredOutcome::Unresolved(diagnostics)
    }

    async fn find_references_tiered(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> TieredOutcome<Vec<Reference>> {
        let mut diagnostics = Vec::new();

        if self.policy.enabled(PrecisionTier::S2) {
            match self
                .attempt_lsp_find_references(location, include_declaration)
                .await
            {
                Attempt::Served(value) => {
                    return self.serve(value, PrecisionTier::S2, diagnostics);
                }
                Attempt::Degraded(d) | Attempt::Failed(d, _) => {
                    self.record_failure("find_references", &d);
                    diagnostics.push(d);
                }
            }
        }

        if self.policy.enabled(PrecisionTier::S0) {
            match self
                .attempt_tree_sitter_find_references(location, include_declaration)
                .await
            {
                Attempt::Served(value) => {
                    return self.serve(value, PrecisionTier::S0, diagnostics);
                }
                Attempt::Degraded(d) | Attempt::Failed(d, _) => {
                    self.record_failure("find_references", &d);
                    diagnostics.push(d);
                }
            }
        }

        TieredOutcome::Unresolved(diagnostics)
    }

    async fn get_hierarchy_tiered(&self, location: &Location) -> TieredOutcome<TypeHierarchy> {
        let mut diagnostics = Vec::new();

        // Behavior delta (D2): the S2 attempt now precedes the tree-sitter
        // tier for hierarchy (it used to hard-code the fallback).
        if self.policy.enabled(PrecisionTier::S2) {
            match self.attempt_lsp_hierarchy(location).await {
                Attempt::Served(value) => {
                    return self.serve(value, PrecisionTier::S2, diagnostics);
                }
                Attempt::Degraded(d) | Attempt::Failed(d, _) => {
                    self.record_failure("get_hierarchy", &d);
                    diagnostics.push(d);
                }
            }
        }

        if self.policy.enabled(PrecisionTier::S0) {
            match self.attempt_tree_sitter_hierarchy(location).await {
                Attempt::Served(value) => {
                    return self.serve(value, PrecisionTier::S0, diagnostics);
                }
                Attempt::Degraded(d) | Attempt::Failed(d, _) => {
                    self.record_failure("get_hierarchy", &d);
                    diagnostics.push(d);
                }
            }
        }

        TieredOutcome::Unresolved(diagnostics)
    }

    async fn get_definition_tiered(&self, location: &Location) -> TieredOutcome<Option<Location>> {
        self.definition_chain(location).await.outcome
    }

    async fn get_document_symbols_tiered(&self, path: &Path) -> TieredOutcome<Vec<DocumentSymbol>> {
        let mut diagnostics = Vec::new();

        if self.policy.enabled(PrecisionTier::S2) {
            match self.attempt_lsp_document_symbols(path).await {
                Attempt::Served(value) => {
                    return self.serve(value, PrecisionTier::S2, diagnostics);
                }
                Attempt::Degraded(d) | Attempt::Failed(d, _) => {
                    self.record_failure("get_document_symbols", &d);
                    diagnostics.push(d);
                }
            }
        }

        if self.policy.enabled(PrecisionTier::S0) {
            match self.attempt_tree_sitter_document_symbols(path).await {
                Attempt::Served(value) => {
                    return self.serve(value, PrecisionTier::S0, diagnostics);
                }
                Attempt::Degraded(d) | Attempt::Failed(d, _) => {
                    self.record_failure("get_document_symbols", &d);
                    diagnostics.push(d);
                }
            }
        }

        TieredOutcome::Unresolved(diagnostics)
    }

    async fn hover_tiered(&self, location: &Location) -> TieredOutcome<Option<HoverInfo>> {
        self.hover_chain(location).await.outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The old-trait passthrough (W2, task 1.1): a definition query for a
    /// missing/unreadable file is the deepest provider's error, never a
    /// synthetic `Ok(None)`. The root does not exist, so the S2 readiness
    /// gate fails without spawning and the S1 resolver answers with
    /// `FileNotFound`.
    #[tokio::test]
    async fn test_composite_falls_back_on_error() {
        let provider = CompositeProvider::new(std::path::Path::new("/nonexistent"));
        let loc = Location::new("/nonexistent/test.rs".to_string(), 1, 1);
        let result = provider.get_definition(&loc).await;
        assert!(
            matches!(result, Err(CodeIntelligenceError::FileNotFound(_))),
            "a missing file must surface the original variant: {result:?}"
        );
    }

    /// The old-trait passthrough (W2, task 1.2): hover for a missing file
    /// propagates `FileNotFound` instead of swallowing it as `Ok(None)`.
    #[tokio::test]
    async fn test_composite_hover_fallback_on_missing_file() {
        let provider = CompositeProvider::new(std::path::Path::new("/nonexistent"));
        let loc = Location::new("/nonexistent/test.rs".to_string(), 1, 1);
        let result = provider.hover(&loc).await;
        assert!(
            matches!(result, Err(CodeIntelligenceError::FileNotFound(_))),
            "a missing file must surface the original variant: {result:?}"
        );
    }

    /// Design D2 behavior delta: hierarchy attempts the S2 tier before the
    /// tree-sitter tier (it used to hard-code the fallback). The unknown
    /// extension makes the S2 attempt `Unavailable` without spawning, so the
    /// assertion is deterministic and fast.
    #[tokio::test]
    async fn test_hierarchy_attempts_s2_before_s0_and_maps_to_internal_error() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("thing.unknownlang");
        std::fs::write(&file, "fn helper() {}\n").unwrap();
        let location = Location::new(file.to_string_lossy().to_string(), 1, 4);

        let provider = CompositeProvider::with_policy(tmp.path(), TierPolicy::all());
        let error = provider
            .get_hierarchy(&location)
            .await
            .expect_err("neither S2 nor S0 can serve hierarchy");
        let message = error.to_string();
        let s2_at = message
            .find("lsp@S2 unavailable")
            .unwrap_or_else(|| panic!("S2 diagnostic named in summary: {message}"));
        let s0_at = message
            .find("tree-sitter@S0 error")
            .unwrap_or_else(|| panic!("S0 diagnostic named in summary: {message}"));
        assert!(s2_at < s0_at, "S2 must be attempted before S0: {message}");

        let status = provider.status();
        let s2 = status.tier(PrecisionTier::S2).expect("S2 counters present");
        assert_eq!((s2.attempts, s2.unavailable, s2.served), (1, 1, 0));
        let s0 = status.tier(PrecisionTier::S0).expect("S0 counters present");
        assert_eq!((s0.attempts, s0.errors), (1, 1));
    }

    /// Historical tree-sitter-only hierarchy behavior survives as the
    /// policy-disabled variant: S0 errors → `Err(Internal)`, and the gated-off
    /// S2 tier is never attempted.
    #[tokio::test]
    async fn test_composite_hierarchy_uses_tree_sitter_when_lsp_gated_off() {
        let provider = CompositeProvider::with_policy(
            std::path::Path::new("/tmp"),
            TierPolicy {
                lsp: false,
                ..TierPolicy::all()
            },
        );
        let location = Location::new("/tmp/test.rs".to_string(), 1, 1);
        let error = provider
            .get_hierarchy(&location)
            .await
            .expect_err("tree-sitter hierarchy is unsupported");
        assert!(
            error.to_string().contains("tree-sitter@S0 error"),
            "{error}"
        );
        assert!(
            !error.to_string().contains("lsp@S2"),
            "gated-off S2 must not appear in the summary: {error}"
        );
        let status = provider.status();
        assert_eq!(
            status
                .tier(PrecisionTier::S2)
                .expect("S2 counters")
                .attempts,
            0,
            "gated-off S2 must not be attempted"
        );
    }

    // ── LSI M4 tier pipeline (RED: task 2.1) ────────────────────────────

    /// Scenario "Gated or failed tier falls through": with the LSP tier
    /// policy-disabled, tree-sitter must serve and DECLARE S0; the gated-off
    /// tier is skipped without a diagnostic or a counter increment.
    #[tokio::test]
    async fn test_gated_lsp_tier_falls_through_declaring_serving_tier() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("sample.rs");
        std::fs::write(&file, "fn helper() {}\n").unwrap();

        let provider = CompositeProvider::with_policy(
            tmp.path(),
            TierPolicy {
                lsp: false,
                ..TierPolicy::all()
            },
        );
        let outcome = provider.get_symbols_tiered(&file).await;

        assert!(outcome.is_served(), "tree-sitter must serve: {outcome:?}");
        assert_eq!(outcome.tier(), Some(PrecisionTier::S0));
        assert!(
            outcome.diagnostics().is_empty(),
            "a policy-disabled tier is skipped without a diagnostic: {:?}",
            outcome.diagnostics()
        );

        let status = provider.status();
        let s2 = status.tier(PrecisionTier::S2).expect("S2 counters present");
        assert_eq!(
            (s2.attempts, s2.served),
            (0, 0),
            "gated-off tier must not be attempted or served"
        );
        let s0 = status.tier(PrecisionTier::S0).expect("S0 counters present");
        assert_eq!((s0.attempts, s0.served), (1, 1));
    }

    /// Scenario "Fallback diagnostic is attached and counted": an unavailable
    /// S2 attempt attaches a diagnostic naming provider + tier + outcome,
    /// falls through, and is counted; the serving tier is declared.
    #[tokio::test]
    async fn test_fallback_diagnostic_is_attached_and_counted() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        // Unknown extension: no LSP language mapping, so the S2 attempt is
        // declared Unavailable without spawning any server process.
        let file = tmp.path().join("notes.unknownlang");
        std::fs::write(&file, "fn helper() {}\n").unwrap();

        let provider = CompositeProvider::with_policy(tmp.path(), TierPolicy::all());
        let location = Location::new(file.to_string_lossy().to_string(), 1, 4);
        let outcome = provider.hover_tiered(&location).await;

        assert!(
            outcome.is_served(),
            "tree-sitter hover must serve: {outcome:?}"
        );
        assert_eq!(outcome.tier(), Some(PrecisionTier::S0));

        let diagnostics = outcome.diagnostics();
        assert_eq!(
            diagnostics.len(),
            1,
            "the failed LSP attempt must attach exactly one diagnostic"
        );
        assert_eq!(diagnostics[0].attempted_tier, PrecisionTier::S2);
        assert_eq!(diagnostics[0].outcome, ProviderOutcome::Unavailable);
        assert!(
            !diagnostics[0].provider.is_empty(),
            "diagnostic must name the provider"
        );

        let status = provider.status();
        let s2 = status.tier(PrecisionTier::S2).expect("S2 counters present");
        assert_eq!((s2.attempts, s2.served, s2.unavailable), (1, 0, 1));
        let s0 = status.tier(PrecisionTier::S0).expect("S0 counters present");
        assert_eq!((s0.attempts, s0.served), (1, 1));
    }

    /// S1 is the lowest definition-capable tier: a resolver `Ok(None)` is
    /// Degraded and exhausts the chain (Unresolved), and the old-trait shape
    /// stays `Ok(None)`.
    #[tokio::test]
    async fn test_local_resolver_none_is_unresolved_and_maps_to_ok_none() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("main.rs");
        std::fs::write(&file, "fn main() { missing_symbol(); }\n").unwrap();
        let location = Location::new(file.to_string_lossy().to_string(), 1, 13);

        let provider = CompositeProvider::with_policy(
            tmp.path(),
            TierPolicy {
                lsp: false,
                ..TierPolicy::all()
            },
        );
        let outcome = provider.get_definition_tiered(&location).await;
        assert!(
            !outcome.is_served(),
            "the local resolver found no definition: {outcome:?}"
        );
        assert_eq!(outcome.diagnostics().len(), 1);
        assert_eq!(outcome.diagnostics()[0].attempted_tier, PrecisionTier::S1);
        assert_eq!(outcome.diagnostics()[0].outcome, ProviderOutcome::Degraded);

        // Counters are cumulative: snapshot them before the old-trait call,
        // which delegates to the same tiered op.
        let status = provider.status();
        let s1 = status.tier(PrecisionTier::S1).expect("S1 counters present");
        assert_eq!((s1.attempts, s1.served, s1.degraded), (1, 0, 1));

        assert!(provider.get_definition(&location).await.unwrap().is_none());
    }

    // ── W2: honest old-trait error semantics (RED: tasks 1.1–1.3) ────────

    /// W2 (task 1.1): a definition query for an unreadable file is an error,
    /// not a clean miss — the deepest provider's original variant survives.
    /// LSP off keeps the S1 resolver as the deepest attempted tier.
    #[tokio::test]
    async fn test_definition_missing_file_maps_to_file_not_found() {
        let provider = CompositeProvider::with_policy(
            std::path::Path::new("/nonexistent"),
            TierPolicy {
                lsp: false,
                ..TierPolicy::all()
            },
        );
        let location = Location::new("/nonexistent/missing.rs".to_string(), 1, 1);

        let error = provider
            .get_definition(&location)
            .await
            .expect_err("a missing file is an error, not a clean miss");
        assert!(
            matches!(error, CodeIntelligenceError::FileNotFound(_)),
            "the deepest provider's original variant must survive: {error}"
        );
    }

    /// W2 (task 1.2): an out-of-range location is the deepest provider's
    /// `InvalidLocation`, not a clean miss.
    #[tokio::test]
    async fn test_definition_out_of_range_location_maps_to_invalid_location() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("main.rs");
        std::fs::write(&file, "fn main() {}\n").unwrap();
        let location = Location::new(file.to_string_lossy().to_string(), 99, 1);

        let provider = CompositeProvider::with_policy(
            tmp.path(),
            TierPolicy {
                lsp: false,
                ..TierPolicy::all()
            },
        );
        let error = provider
            .get_definition(&location)
            .await
            .expect_err("an out-of-range position is an error, not a clean miss");
        assert!(
            matches!(error, CodeIntelligenceError::InvalidLocation(_)),
            "the deepest provider's original variant must survive: {error}"
        );
    }

    /// W2 (task 1.3): real clean misses keep the historical `Ok(None)` —
    /// an S1 resolver miss, a terminal S0 hover miss, and a gate-only
    /// failure with no lower tier attempted.
    #[tokio::test]
    async fn test_clean_miss_and_gate_only_failure_stay_ok_none() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("main.rs");
        std::fs::write(
            &file,
            "fn main() { missing_symbol(); }\n// no symbol here\n",
        )
        .unwrap();
        let provider = CompositeProvider::with_policy(
            tmp.path(),
            TierPolicy {
                lsp: false,
                ..TierPolicy::all()
            },
        );

        // S1 local resolver: the identifier exists but has no definition.
        let location = Location::new(file.to_string_lossy().to_string(), 1, 13);
        assert!(provider.get_definition(&location).await.unwrap().is_none());

        // S0 terminal hover: no identifier at the queried position.
        let location = Location::new(file.to_string_lossy().to_string(), 2, 1);
        assert!(provider.hover(&location).await.unwrap().is_none());

        // Gate-only failure: an unsupported language cannot even reach the
        // LSP tier, and no lower tier is enabled — still no error.
        let unknown = tmp.path().join("notes.unknownlang");
        std::fs::write(&unknown, "nothing\n").unwrap();
        let gate_only = CompositeProvider::with_policy(
            tmp.path(),
            TierPolicy {
                local_resolver: false,
                tree_sitter: false,
                ..TierPolicy::all()
            },
        );
        let location = Location::new(unknown.to_string_lossy().to_string(), 1, 1);
        assert!(gate_only.get_definition(&location).await.unwrap().is_none());
    }

    // ── W3: bounded fallback readiness (RED: tasks 3.1–3.2) ──────────────

    /// W3 (task 3.1): the S2 readiness fallback bound defaults to 2s.
    #[test]
    fn test_fallback_readiness_defaults_to_two_seconds() {
        assert_eq!(
            TierPolicy::all().fallback_readiness,
            Duration::from_secs(2),
            "the default S2 fallback readiness is 2s"
        );
    }

    /// W3 (task 3.1): an enabled lower tier bounds readiness at
    /// `min(wait, fallback_readiness)`; a policy-disabled lower tier keeps
    /// the full wait (nothing below can serve, so S2 gets the whole budget).
    #[test]
    fn test_readiness_budget_uses_the_fallback_bound_only_with_a_lower_tier() {
        let bounded = TierPolicy {
            fallback_readiness: Duration::from_millis(150),
            ..TierPolicy::all()
        };
        assert_eq!(
            bounded.readiness_budget(30, PrecisionTier::S0),
            Duration::from_millis(150)
        );
        assert_eq!(
            bounded.readiness_budget(30, PrecisionTier::S1),
            Duration::from_millis(150)
        );
        // The shorter full wait wins: the bound never extends it.
        assert_eq!(
            TierPolicy::all().readiness_budget(1, PrecisionTier::S0),
            Duration::from_secs(1)
        );

        let no_lower_tier = TierPolicy {
            lsp: true,
            local_resolver: false,
            tree_sitter: false,
            ..TierPolicy::all()
        };
        assert_eq!(
            no_lower_tier.readiness_budget(30, PrecisionTier::S0),
            Duration::from_secs(30)
        );
        assert_eq!(
            no_lower_tier.readiness_budget(30, PrecisionTier::S1),
            Duration::from_secs(30)
        );
    }

    /// W3 (task 3.2): the readiness wrapper cuts a hanging readiness at the
    /// bound — it never waits for the provider's own request timeout.
    #[tokio::test]
    async fn test_bounded_readiness_cuts_a_hanging_readiness() {
        let budget = Duration::from_millis(100);
        let started = std::time::Instant::now();
        let result = CompositeProvider::readiness_within(
            budget,
            std::future::pending::<Result<(), LspProcessError>>(),
        )
        .await;
        let elapsed = started.elapsed();

        assert!(result.is_err(), "the hanging readiness must expire");
        assert!(
            elapsed >= budget,
            "the bound must not cut early: {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "the wait must stay far below the 30s initialize timeout: {elapsed:?}"
        );
    }

    /// W3 (task 3.2): with a lower tier enabled, `get_hierarchy` falls
    /// through on the bound instead of paying the full S2 readiness wait
    /// (`LspProcess::initialize` has its own 30s request timeout). PATH-gated
    /// on the Rust server binary, like the e39 Java conformance branch:
    /// without a spawnable server the gate fails immediately and cannot time
    /// out.
    #[tokio::test]
    async fn test_hierarchy_falls_through_within_the_bounded_readiness() {
        if !binary_on_path("rust-analyzer") {
            println!(
                "skipped: rust-analyzer is not on PATH; the bounded fall-through \
                 needs a spawnable server"
            );
            return;
        }

        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("sample.rs");
        std::fs::write(&file, "fn helper() {}\n").unwrap();
        let location = Location::new(file.to_string_lossy().to_string(), 1, 4);

        let provider = CompositeProvider::with_policy(
            tmp.path(),
            TierPolicy {
                fallback_readiness: Duration::from_millis(50),
                ..TierPolicy::all()
            },
        );
        let started = std::time::Instant::now();
        let outcome = provider.get_hierarchy_tiered(&location).await;
        let elapsed = started.elapsed();

        assert!(
            !outcome.is_served(),
            "tree-sitter cannot serve hierarchy: {outcome:?}"
        );
        assert!(
            elapsed < Duration::from_secs(5),
            "the bound must cut the wait far below the 30s request timeout: {elapsed:?}"
        );
        let diagnostics = outcome.diagnostics();
        assert!(
            diagnostics.iter().any(|d| {
                d.attempted_tier == PrecisionTier::S2
                    && d.outcome == ProviderOutcome::Unavailable
                    && d.message.contains("bounded fallback")
            }),
            "S2 must record an Unavailable diagnostic naming the bounded fallback: {diagnostics:?}"
        );
        let status = provider.status();
        let s2 = status.tier(PrecisionTier::S2).expect("S2 counters present");
        assert_eq!((s2.attempts, s2.unavailable), (1, 1));
        let s0 = status.tier(PrecisionTier::S0).expect("S0 counters present");
        assert_eq!((s0.attempts, s0.errors), (1, 1));
    }

    /// PATH scan (no spawn) for the bounded-readiness test: mirrors the
    /// conformance harness probe.
    fn binary_on_path(binary: &str) -> bool {
        std::env::var_os("PATH")
            .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(binary).is_file()))
            .unwrap_or(false)
    }

    /// Historical old-trait shape (D2): an unresolved collection op is
    /// `Err(Internal)` carrying the exhausted-tier diagnostic summary.
    #[tokio::test]
    async fn test_symbols_unresolved_maps_to_internal_error() {
        let provider = CompositeProvider::with_policy(
            std::path::Path::new("/tmp"),
            TierPolicy {
                lsp: false,
                ..TierPolicy::all()
            },
        );
        let error = provider
            .get_symbols(std::path::Path::new("/nonexistent/missing.rs"))
            .await
            .expect_err("no tier can parse a missing file");
        let message = error.to_string();
        assert!(message.contains("all tiers exhausted"), "{message}");
        assert!(message.contains("tree-sitter@S0 error"), "{message}");
    }

    /// The policy-disabled S2 tier is skipped for every op without a
    /// diagnostic or a counter increment, while S0 keeps serving.
    #[tokio::test]
    async fn test_policy_gating_applies_to_every_op() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join("sample.ts");
        std::fs::write(&file, "export function helper() {}\n").unwrap();
        let location = Location::new(file.to_string_lossy().to_string(), 1, 17);

        let provider = CompositeProvider::with_policy(
            tmp.path(),
            TierPolicy {
                lsp: false,
                ..TierPolicy::all()
            },
        );

        let symbols = provider.get_symbols_tiered(&file).await;
        assert!(symbols.is_served());
        assert_eq!(symbols.tier(), Some(PrecisionTier::S0));

        let references = provider.find_references_tiered(&location, true).await;
        assert!(references.is_served());
        assert_eq!(references.tier(), Some(PrecisionTier::S0));

        let document_symbols = provider.get_document_symbols_tiered(&file).await;
        assert!(document_symbols.is_served());
        assert_eq!(document_symbols.tier(), Some(PrecisionTier::S0));

        let hover = provider.hover_tiered(&location).await;
        assert!(hover.is_served());
        assert_eq!(hover.tier(), Some(PrecisionTier::S0));

        let status = provider.status();
        assert_eq!(
            status
                .tier(PrecisionTier::S2)
                .expect("S2 counters")
                .attempts,
            0,
            "gated-off S2 must not be attempted by any op"
        );
        let s0 = status.tier(PrecisionTier::S0).expect("S0 counters present");
        assert_eq!((s0.attempts, s0.served), (4, 4));
    }
}
