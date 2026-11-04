//! E39 M4 provider conformance harness (design D6/D8).
//!
//! Loads the per-language fixture manifests
//! (`sandbox/fixtures/lsi-providers/<language>/expected.json`), which
//! declare — BEFORE the run — the expected serving tier and outcome of
//! every query. The runner observes each query through the composite tier
//! pipeline and verifies the observation against its declaration; a
//! contradiction fails the run naming the query, the declared tier, and
//! the observed tier (spec `provider-pipeline-conformance`).
//!
//! Availability gating (spec "Availability-gated language coverage"):
//! Rust/TS run with the LSP tier policy-disabled — no server is spawned —
//! so their declared non-LSP targets verify unconditionally. Java probes
//! the `jdtls` binary NAME on `PATH` (a scan, never a spawn): present → the
//! full S2-declared run; absent → the declared-unavailable path asserts no
//! result declares a tier above what is available and that an `Unavailable`
//! S2 diagnostic was recorded, and does NOT fail.

use std::fs;
use std::path::{Path, PathBuf};

use cognicode_core::domain::traits::code_intelligence::{
    PrecisionTier, ProviderDiagnostic, ProviderOutcome, TieredCodeIntelligenceProvider,
    TieredOutcome,
};
use cognicode_core::domain::value_objects::Location;
use serde::Deserialize;

/// Fixture languages declared by design D6.
pub const RUST_LANGUAGE: &str = "rust";
pub const TS_LANGUAGE: &str = "ts";
pub const JAVA_LANGUAGE: &str = "java";

/// Java's LSP server binary name (`Language::lsp_server_binary`), probed via
/// a `PATH` scan — never spawned by the probe.
pub const JAVA_SERVER_BINARY: &str = "jdtls";

/// The tier gated by language-server availability.
pub const LSP_TIER: PrecisionTier = PrecisionTier::S2;

/// The highest tier reachable without a language server: S1 local resolver
/// (S0 tree-sitter lies below it).
pub const SERVERLESS_MAX_TIER: PrecisionTier = PrecisionTier::S1;

/// One fixture manifest (`expected.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedManifest {
    /// Manifest schema version (currently `1`).
    pub schema_version: u32,
    /// Fixture language key (`rust`, `ts`, `java`).
    pub language: String,
    /// The declared queries, verified in order.
    pub queries: Vec<ExpectedQuery>,
}

/// One declared query.
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedQuery {
    /// Composite/Tiered op name (`get_symbols`, `hover`, …).
    pub op: String,
    /// Fixture-relative source file.
    pub file: String,
    /// 1-based line, for location-driven ops.
    #[serde(default)]
    pub line: Option<u32>,
    /// 1-based column, for location-driven ops.
    #[serde(default)]
    pub column: Option<u32>,
    /// The declaration verified against the observation.
    pub expected: ExpectedObservation,
}

/// The declared observation for one query.
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedObservation {
    /// Declared serving tier (`"S0"` … `"S4"`).
    pub tier: String,
    /// Declared outcome.
    pub outcome: ExpectedOutcome,
    /// Optional declared diagnostics, each `"<tier>:<outcome>"`
    /// (e.g. `"S2:Unavailable"`). Every listed diagnostic must be recorded.
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

/// Declared outcome of a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ExpectedOutcome {
    /// Some tier served a value.
    Served,
    /// Every eligible tier was exhausted.
    Unresolved,
}

impl std::fmt::Display for ExpectedOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ExpectedOutcome::Served => "Served",
            ExpectedOutcome::Unresolved => "Unresolved",
        })
    }
}

/// What the pipeline actually observed for one query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// Human-readable query label (`"<op> <file>[:<line>]"`).
    pub query: String,
    /// The serving tier when served.
    pub tier: Option<PrecisionTier>,
    /// Observed outcome.
    pub outcome: ExpectedOutcome,
    /// Diagnostics attached by the pipeline (fall-through trail).
    pub diagnostics: Vec<ProviderDiagnostic>,
}

impl Observation {
    /// The observed tier as a display string (`"none"` when unresolved).
    pub fn tier_text(&self) -> String {
        self.tier
            .map(|tier| tier.to_string())
            .unwrap_or_else(|| "none".to_string())
    }
}

/// The fixture root for `language` (`sandbox/fixtures/lsi-providers/<lang>`).
pub fn fixture_root(language: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures/lsi-providers")
        .join(language)
        .canonicalize()
        .unwrap_or_else(|error| panic!("fixture root for {language} resolves: {error}"))
}

/// Parses a declared tier string (`"S0"` … `"S4"`), failing loudly on an
/// unknown declaration.
pub fn parse_tier(text: &str) -> PrecisionTier {
    text.parse()
        .unwrap_or_else(|_| panic!("manifest declares the canonical tier form, got '{text}'"))
}

/// Parses a declared diagnostic (`"<tier>:<outcome>"`).
pub fn parse_diagnostic(text: &str) -> (PrecisionTier, ProviderOutcome) {
    let (tier, outcome) = text
        .split_once(':')
        .unwrap_or_else(|| panic!("declared diagnostic is '<tier>:<outcome>', got '{text}'"));
    let outcome = match outcome {
        "Unavailable" => ProviderOutcome::Unavailable,
        "Error" => ProviderOutcome::Error,
        "Degraded" => ProviderOutcome::Degraded,
        other => panic!("unknown declared diagnostic outcome '{other}'"),
    };
    (parse_tier(tier), outcome)
}

/// Loads the fixture manifest for `language`, validating its declared
/// schema version and language key (a stale or mis-filed manifest fails
/// loudly instead of verifying the wrong cohort).
pub fn load_manifest(language: &str) -> ExpectedManifest {
    let path = fixture_root(language).join("expected.json");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("manifest {} is readable: {error}", path.display()));
    let manifest: ExpectedManifest = serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("manifest {} parses: {error}", path.display()));
    assert_eq!(
        manifest.schema_version,
        1,
        "manifest {} declares the supported schema_version",
        path.display()
    );
    assert_eq!(
        manifest.language,
        language,
        "manifest {} declares its own language",
        path.display()
    );
    manifest
}

/// Probes whether `binary` is available on `PATH` — a directory scan, no
/// spawn (design D6; proposal risk 2: no CI flakiness from process probing).
pub fn binary_on_path(binary: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(binary);
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

/// Runs one declared query through `provider` and captures the observation.
pub async fn observe(
    provider: &dyn TieredCodeIntelligenceProvider,
    root: &Path,
    query: &ExpectedQuery,
) -> Observation {
    let path = root.join(&query.file);
    let location = Location::new(
        path.to_string_lossy().to_string(),
        query.line.unwrap_or(1),
        query.column.unwrap_or(1),
    );
    let label = format!(
        "{} {}{}",
        query.op,
        query.file,
        query
            .line
            .map(|line| format!(":{line}"))
            .unwrap_or_default()
    );

    match query.op.as_str() {
        "get_symbols" => outcome_to_observation(label, provider.get_symbols_tiered(&path).await),
        "get_document_symbols" => {
            outcome_to_observation(label, provider.get_document_symbols_tiered(&path).await)
        }
        "hover" => outcome_to_observation(label, provider.hover_tiered(&location).await),
        "find_references" => outcome_to_observation(
            label,
            provider.find_references_tiered(&location, false).await,
        ),
        "get_definition" => {
            outcome_to_observation(label, provider.get_definition_tiered(&location).await)
        }
        "get_hierarchy" => {
            outcome_to_observation(label, provider.get_hierarchy_tiered(&location).await)
        }
        other => panic!("manifest declares an implemented op, got '{other}'"),
    }
}

/// Runs every declared query in order.
pub async fn observe_all(
    provider: &dyn TieredCodeIntelligenceProvider,
    root: &Path,
    manifest: &ExpectedManifest,
) -> Vec<Observation> {
    let mut observations = Vec::new();
    for query in &manifest.queries {
        observations.push(observe(provider, root, query).await);
    }
    observations
}

fn outcome_to_observation<T>(query: String, outcome: TieredOutcome<T>) -> Observation {
    match outcome {
        TieredOutcome::Served(tiered) => Observation {
            query,
            tier: Some(tiered.tier),
            outcome: ExpectedOutcome::Served,
            diagnostics: tiered.diagnostics,
        },
        TieredOutcome::Unresolved(diagnostics) => Observation {
            query,
            tier: None,
            outcome: ExpectedOutcome::Unresolved,
            diagnostics,
        },
    }
}

/// Verifies every observation against its declaration.
///
/// Returns one failure message per contradiction, each naming the query,
/// the declared tier, and the observed tier/outcome. An empty result means
/// every declaration matched — no numeric thresholds are involved.
pub fn verify(manifest: &ExpectedManifest, observations: &[Observation]) -> Vec<String> {
    let mut failures = Vec::new();
    if observations.len() != manifest.queries.len() {
        failures.push(format!(
            "run observed {} queries, the manifest declares {}",
            observations.len(),
            manifest.queries.len()
        ));
    }

    for (query, observation) in manifest.queries.iter().zip(observations) {
        let declared_tier = parse_tier(&query.expected.tier);
        match (query.expected.outcome, observation.outcome) {
            (ExpectedOutcome::Served, ExpectedOutcome::Served) => {
                if observation.tier != Some(declared_tier) {
                    failures.push(format!(
                        "query {}: declared tier {declared_tier}, observed tier {}",
                        observation.query,
                        observation.tier_text()
                    ));
                }
            }
            (ExpectedOutcome::Served, ExpectedOutcome::Unresolved) => failures.push(format!(
                "query {}: declared tier {declared_tier} Served, observed Unresolved{}",
                observation.query,
                diagnostics_suffix(observation)
            )),
            (ExpectedOutcome::Unresolved, ExpectedOutcome::Served) => failures.push(format!(
                "query {}: declared tier {declared_tier} Unresolved, observed {} served at tier {}{}",
                observation.query,
                observation.outcome,
                observation.tier_text(),
                diagnostics_suffix(observation)
            )),
            (ExpectedOutcome::Unresolved, ExpectedOutcome::Unresolved) => {}
        }

        // Every declared diagnostic must be recorded on the observation.
        for declared in &query.expected.diagnostics {
            let (tier, outcome) = parse_diagnostic(declared);
            let recorded = observation.diagnostics.iter().any(|diagnostic| {
                diagnostic.attempted_tier == tier && diagnostic.outcome == outcome
            });
            if !recorded {
                failures.push(format!(
                    "query {}: declared diagnostic {declared} was not recorded{}",
                    observation.query,
                    diagnostics_suffix(observation)
                ));
            }
        }
    }

    failures
}

/// The recorded diagnostics, formatted for a failure message (empty string
/// when none were recorded).
fn diagnostics_suffix(observation: &Observation) -> String {
    if observation.diagnostics.is_empty() {
        return String::new();
    }
    format!(
        " (diagnostics: {})",
        observation
            .diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" | ")
    )
}

/// The declared-unavailable path (spec scenario "Java without its server
/// degrades by declaration"): every observation must have recorded an
/// `Unavailable` diagnostic for [`LSP_TIER`], and NO result may declare a
/// tier above [`SERVERLESS_MAX_TIER`].
pub fn verify_unavailable_degradation(observations: &[Observation]) -> Vec<String> {
    let mut failures = Vec::new();
    for observation in observations {
        let lsp_unavailable = observation.diagnostics.iter().any(|diagnostic| {
            diagnostic.attempted_tier == LSP_TIER
                && diagnostic.outcome == ProviderOutcome::Unavailable
        });
        if !lsp_unavailable {
            failures.push(format!(
                "query {}: the unavailable LSP tier must record an {LSP_TIER}:Unavailable \
                 diagnostic; recorded {:?}",
                observation.query, observation.diagnostics
            ));
        }
        if let Some(tier) = observation.tier
            && tier > SERVERLESS_MAX_TIER
        {
            failures.push(format!(
                "query {}: no result may declare a tier above the available \
                 {SERVERLESS_MAX_TIER}, observed {tier}",
                observation.query
            ));
        }
    }
    failures
}
