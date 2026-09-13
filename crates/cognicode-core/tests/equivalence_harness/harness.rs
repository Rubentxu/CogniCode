//! E37 M2 equivalence harness core (design D7).
//!
//! Compares, per golden fixture, the LEGACY graph oracle against the
//! FACT-DERIVED projection:
//!
//! - **Legacy oracle**: `AnalysisService::new().build_project_graph(dir)` →
//!   `get_project_graph()` → `CallGraphProjection::from_call_graph`.
//! - **Fact path**: fixture walk → `extract_file` per source →
//!   `FactBatchBuilder` (canonical batch, design D3) →
//!   `bootstrap_registry` → `InMemoryFactStore::commit` →
//!   `facts_in_snapshot` → `CallGraphProjection::from_facts`.
//! - **Comparator**: Jaccard over normalized (stably sorted) node/edge
//!   multisets; per-fixture score compared against
//!   [`EQUIVALENCE_THRESHOLD`].
//!
//! Quarantine (spec "Quarantine of known-unstable surfaces"): fixtures in
//! [`KNOWN_UNSTABLE_SURFACES`] are MEASURED and REPORTED but excluded from
//! the pass/fail verdict — the legacy graph engine's directory-level node
//! set is walk-order unstable for same-named symbols across languages (e36
//! WU-1 finding). Any divergence OUTSIDE the quarantine fails the run.
//!
//! Deterministic fact identity (spec "Deterministic fact identity"): the
//! entity-identity convention is pinned through
//! [`PINNED_IDENTITY_DIGEST`]; a convention change fails the run until the
//! goldens are explicitly re-pinned as a new baseline.
//!
//! Known input gap (encoded honestly, not silently ignored): Rust `use`
//! statements currently emit NO import facts (tree-sitter field mismatch,
//! flagged in e37 batch 1). Legacy builds carry no import edges either, so
//! the Calls-only edge multiset used for scoring is unaffected; the gap is
//! surfaced here so a future extractor fix re-baselines consciously.
//!
//! DECLARED normalization (part of the comparison contract): the legacy
//! engine records symbol lines as the tree-sitter 0-based `start.row`
//! (`tree_sitter_parser::node_to_symbol_with_path`), while the extractor —
//! and therefore the fact-side FQN convention (design D3) — records
//! `start.row + 1`. The SAME source symbol thus yields legacy FQN
//! `f:name:N` and fact FQN `f:name:N+1`. The harness normalizes the LEGACY
//! side onto the fact-side convention (`normalize_legacy_fqn`) before
//! comparing; this is a mechanical, deterministic convention alignment, not
//! a tolerated divergence. Any structural difference still fails the run.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cognicode_core::application::fact_bridge::FactBatchBuilder;
use cognicode_core::application::ingest::extractor::extract_file;
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::domain::evidence_kernel::bootstrap::bootstrap_registry;
use cognicode_core::domain::evidence_kernel::fact::Fact;
use cognicode_core::domain::evidence_kernel::ids::SnapshotId;
use cognicode_core::domain::evidence_kernel::ports::FactStore;
use cognicode_core::domain::value_objects::{WalkFilter, WorkspaceId};
use cognicode_core::infrastructure::evidence_kernel::in_memory::{
    InMemoryFactStore, InMemorySchemaRegistry,
};
use cognicode_core::infrastructure::graph::CallGraphProjection;
use cognicode_core::infrastructure::parser::LanguageConfig;
use ignore::WalkBuilder;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

/// Declared BEFORE any comparison (spec "Declared equivalence contract"):
/// minimum structural equivalence for every non-quarantined fixture.
pub const EQUIVALENCE_THRESHOLD: f64 = 0.99;

/// Declared quarantine: surfaces whose legacy behavior is known-unstable
/// (walk-order nondeterminism on same-named symbols across languages).
/// Measured and reported, excluded from scoring.
pub const KNOWN_UNSTABLE_SURFACES: [&str; 1] = ["multi-lang-types"];

/// Fixtures whose equivalence is SCORED against the threshold.
pub const SCORED_FIXTURES: [&str; 2] = ["python-hello", "rust-hello"];

/// Quarantined fixtures (the intersection of the fixture set with
/// [`KNOWN_UNSTABLE_SURFACES`]).
pub const QUARANTINED_FIXTURES: [&str; 1] = ["multi-lang-types"];

/// Every fixture the harness visits.
pub const ALL_FIXTURES: [&str; 3] = ["python-hello", "rust-hello", "multi-lang-types"];

/// The pinned entity-identity convention (design D3). Changing ANY part of
/// this text changes the digest and fails the harness until the affected
/// goldens are explicitly re-pinned as a new baseline.
pub const IDENTITY_CONVENTION: &str = "e37 entity-identity convention v1: subjects are raw id strings; \
symbols use the legacy FQN '{file}:{name}:{line}'; EntityIdTable maps sorted \
unique subject strings onto EntityId(1..N) per snapshot; no hashing; \
cross-entity references stay FactValue::Text";

/// The digest of the convention the current goldens were pinned against.
pub const PINNED_IDENTITY_DIGEST: &str = "fnv1a64:efccc22e912913fe";

/// The pinned snapshot/workspace the harness commits into.
pub const SNAPSHOT: SnapshotId = SnapshotId::new(1);
pub const WORKSPACE: &str = "ws-e37-harness";

/// FNV-1a 64-bit digest (deterministic, dependency-free).
fn fnv1a64(data: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// Digest of an arbitrary convention text (exposed for the re-baseline
/// scenario test, which simulates a convention change).
pub fn identity_convention_digest_for(convention: &str) -> String {
    fnv1a64(convention)
}

/// Digest of the CURRENT entity-identity convention.
pub fn identity_convention_digest() -> String {
    identity_convention_digest_for(IDENTITY_CONVENTION)
}

/// Verifies `digest` against the pinned baseline. Err instructs an explicit
/// re-baseline (spec scenario "Convention change requires explicit
/// re-baseline").
pub fn verify_identity_pin(digest: &str) -> Result<(), String> {
    if digest == PINNED_IDENTITY_DIGEST {
        Ok(())
    } else {
        Err(format!(
            "entity-identity convention changed (digest {digest} != pinned \
             {PINNED_IDENTITY_DIGEST}): re-pin the affected goldens as a new \
             baseline (explicit re-baseline required)"
        ))
    }
}

/// One fixture's equivalence report.
#[derive(Debug, Clone)]
pub struct FixtureReport {
    /// Fixture (surface) name.
    pub name: String,
    /// Whether the surface is quarantined (excluded from scoring).
    pub quarantined: bool,
    /// Jaccard over the normalized node multisets.
    pub node_score: f64,
    /// Jaccard over the normalized edge multisets.
    pub edge_score: f64,
    /// Legacy node/edge multiset sizes.
    pub legacy_nodes: usize,
    pub legacy_edges: usize,
    /// Fact-path node/edge multiset sizes.
    pub fact_nodes: usize,
    pub fact_edges: usize,
    /// Call facts the fact path could not resolve (dropped AND counted).
    pub fact_unresolved: usize,
}

impl FixtureReport {
    /// The score compared against the threshold: the weaker multiset score.
    pub fn min_score(&self) -> f64 {
        self.node_score.min(self.edge_score)
    }

    /// True when a non-quarantined surface would fail the declared contract.
    pub fn below_threshold(&self) -> bool {
        !self.quarantined && self.min_score() < EQUIVALENCE_THRESHOLD
    }

    /// One-line human-readable report entry (includes name and scores).
    pub fn describe(&self) -> String {
        format!(
            "fixture '{}' [{}]: node_score={:.4} edge_score={:.4} \
             legacy(nodes={}, edges={}) fact(nodes={}, edges={}, unresolved={})",
            self.name,
            if self.quarantined {
                "QUARANTINED"
            } else {
                "scored"
            },
            self.node_score,
            self.edge_score,
            self.legacy_nodes,
            self.legacy_edges,
            self.fact_nodes,
            self.fact_edges,
            self.fact_unresolved,
        )
    }
}

/// The outcome of one full harness run.
#[derive(Debug, Clone)]
pub struct HarnessRun {
    /// Per-fixture reports (quarantined surfaces included).
    pub reports: Vec<FixtureReport>,
    /// Digest of the identity convention observed during this run.
    pub digest: String,
    /// Whether the observed convention matches the pinned baseline.
    pub digest_ok: bool,
    /// Final verdict (digest pin + every non-quarantined fixture at/above
    /// the threshold).
    pub passed: bool,
    /// Named failures (empty when passed).
    pub failures: Vec<String>,
}

/// Evaluates reports against the declared contract (pure function so the
/// failure scenarios are testable without real fixtures).
pub fn evaluate_reports(reports: &[FixtureReport], digest_ok: bool) -> (bool, Vec<String>) {
    let mut failures = Vec::new();
    if !digest_ok {
        failures.push(
            "entity-identity convention changed: explicit re-baseline of the \
             pinned goldens is required before the run can pass"
                .to_string(),
        );
    }
    for report in reports {
        if report.below_threshold() {
            failures.push(format!(
                "surface '{}' diverges below the declared threshold \
                 {EQUIVALENCE_THRESHOLD}: node_score={:.4} edge_score={:.4}",
                report.name, report.node_score, report.edge_score
            ));
        }
    }
    (failures.is_empty(), failures)
}

/// Runs the full harness over `fixtures_root` (the `sandbox/fixtures` dir).
pub fn run(fixtures_root: &Path) -> HarnessRun {
    let digest = identity_convention_digest();
    let digest_ok = verify_identity_pin(&digest).is_ok();

    let mut reports = Vec::new();
    for name in ALL_FIXTURES {
        let dir = fixtures_root.join(name);
        let quarantined = KNOWN_UNSTABLE_SURFACES.contains(&name);
        reports.push(compare_fixture(name, &dir, quarantined));
    }

    let (passed, failures) = evaluate_reports(&reports, digest_ok);
    HarnessRun {
        reports,
        digest,
        digest_ok,
        passed,
        failures,
    }
}

/// Builds both projections for one fixture and scores them.
fn compare_fixture(name: &str, dir: &Path, quarantined: bool) -> FixtureReport {
    let legacy = legacy_projection(dir);
    let fact = fact_projection(dir);

    let legacy_nodes = legacy_node_multiset(&legacy);
    let legacy_edges = legacy_edge_multiset(&legacy);
    let fact_nodes = node_multiset(&fact);
    let fact_edges = edge_multiset(&fact);

    FixtureReport {
        name: name.to_string(),
        quarantined,
        node_score: multiset_jaccard(&legacy_nodes, &fact_nodes),
        edge_score: multiset_jaccard(&legacy_edges, &fact_edges),
        legacy_nodes: legacy_nodes.len(),
        legacy_edges: legacy_edges.len(),
        fact_nodes: fact_nodes.len(),
        fact_edges: fact_edges.len(),
        fact_unresolved: fact.unresolved_edges(),
    }
}

/// The LEGACY oracle (design D7): AnalysisService build path →
/// `from_call_graph`.
pub fn legacy_projection(fixture_dir: &Path) -> CallGraphProjection {
    let service = AnalysisService::new();
    service
        .build_project_graph(fixture_dir)
        .expect("legacy build_project_graph");
    let graph = service.get_project_graph();
    CallGraphProjection::from_call_graph(&graph)
}

/// The FACT path (design D7): extract → batch → bootstrap → commit →
/// read back → `from_facts`.
pub fn fact_projection(fixture_dir: &Path) -> CallGraphProjection {
    let facts = fixture_facts(fixture_dir);
    fact_projection_from_committed(&facts)
}

/// Walks one fixture and extracts the canonical fact batch (design D3):
/// files in sorted order, `extract_file` per recognized source, producer
/// `DeterministicAnalyzer`.
pub fn fixture_facts(fixture_dir: &Path) -> Vec<Fact> {
    let mut builder = FactBatchBuilder::new(SNAPSHOT);
    for (path, config) in fixture_sources(fixture_dir) {
        let source = std::fs::read_to_string(&path).expect("read fixture source");
        let result = extract_file(config, &path, &source, "harness-fact-bridge");
        builder.add_extraction(&result);
    }
    builder.finish()
}

/// Commits `facts` into a fresh kernel store (bootstrap → commit →
/// `facts_in_snapshot`) and builds the fact-sourced projection from the
/// RE-READ facts — the R2 rebuild path.
pub fn fact_projection_from_committed(facts: &[Fact]) -> CallGraphProjection {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).expect("canonical bootstrap registers the core:* set");
    let store = Arc::new(InMemoryFactStore::new(Arc::new(registry)));
    let workspace = WorkspaceId::try_new(WORKSPACE).expect("valid workspace");

    let committed = tokio_test::block_on(store.commit(&workspace, &SNAPSHOT, facts.to_vec()))
        .expect("canonical facts commit");
    assert_eq!(
        committed.len(),
        facts.len(),
        "every committed fact returns an id"
    );

    let read_back = tokio_test::block_on(store.facts_in_snapshot(&workspace, &SNAPSHOT))
        .expect("pinned snapshot read");
    assert_eq!(
        read_back.len(),
        facts.len(),
        "facts_in_snapshot must return every committed fact"
    );

    CallGraphProjection::from_facts(&read_back)
}

/// Recognized sources under `root`: same walk semantics as the legacy
/// oracle (`ignore` walker, hidden + git-ignore respected, `WalkFilter`
/// blocklist) plus `LanguageConfig::from_extension`. Sorted for
/// determinism; the batch builder erases any residual order difference.
fn fixture_sources(root: &Path) -> Vec<(PathBuf, &'static LanguageConfig)> {
    let walk_filter = WalkFilter::default();
    let mut sources: Vec<(PathBuf, &'static LanguageConfig)> = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| {
            let path = entry.path().to_path_buf();
            if walk_filter.matches_any_component(&path) {
                return None;
            }
            let ext = path.extension()?.to_str()?;
            let config = LanguageConfig::from_extension(ext)?;
            Some((path, config))
        })
        .collect();
    sources.sort_by(|a, b| a.0.cmp(&b.0));
    sources
}

/// Normalized node multiset: sorted `SymbolId` strings.
pub fn node_multiset(projection: &CallGraphProjection) -> Vec<String> {
    let mut nodes: Vec<String> = projection
        .graph()
        .node_indices()
        .map(|ni| projection.graph()[ni].as_str().to_string())
        .collect();
    nodes.sort();
    nodes
}

/// Normalized edge multiset: sorted
/// `(source, target, dependency-type, confidence-bits)` items.
pub fn edge_multiset(projection: &CallGraphProjection) -> Vec<(String, String, String, u64)> {
    let mut edges: Vec<(String, String, String, u64)> = projection
        .graph()
        .edge_references()
        .map(|edge| {
            let (dep, confidence) = *edge.weight();
            (
                projection.graph()[edge.source()].as_str().to_string(),
                projection.graph()[edge.target()].as_str().to_string(),
                dep.to_string(),
                confidence.to_bits(),
            )
        })
        .collect();
    edges.sort();
    edges
}

/// Normalizes a LEGACY-engine FQN onto the fact-side line convention: the
/// legacy parser stores the 0-based `start.row` in the trailing FQN
/// segment, the extractor stores `start.row + 1` (design D3). Non-parsing
/// identity strings pass through unchanged.
pub fn normalize_legacy_fqn(fqn: &str) -> String {
    match fqn.rsplit_once(':') {
        Some((prefix, line)) => match line.parse::<u32>() {
            Ok(row) => format!("{prefix}:{}", row + 1),
            Err(_) => fqn.to_string(),
        },
        None => fqn.to_string(),
    }
}

/// Normalized LEGACY node multiset: sorted FQNs re-based onto the
/// extractor's 1-based line convention (declared normalization).
pub fn legacy_node_multiset(projection: &CallGraphProjection) -> Vec<String> {
    let mut nodes: Vec<String> = node_multiset(projection)
        .into_iter()
        .map(|fqn| normalize_legacy_fqn(&fqn))
        .collect();
    nodes.sort();
    nodes
}

/// Normalized LEGACY edge multiset: both endpoints re-based onto the
/// extractor's 1-based line convention, then sorted.
pub fn legacy_edge_multiset(
    projection: &CallGraphProjection,
) -> Vec<(String, String, String, u64)> {
    let mut edges: Vec<(String, String, String, u64)> = edge_multiset(projection)
        .into_iter()
        .map(|(source, target, dep, confidence)| {
            (
                normalize_legacy_fqn(&source),
                normalize_legacy_fqn(&target),
                dep,
                confidence,
            )
        })
        .collect();
    edges.sort();
    edges
}

/// Jaccard over MULTISETS (count-aware): `sum(min) / sum(max)`; two empty
/// multisets are identical (score 1.0).
pub fn multiset_jaccard<T: Ord + Clone>(a: &[T], b: &[T]) -> f64 {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort();
    b.sort();

    let (mut i, mut j) = (0usize, 0usize);
    let (mut intersection, mut union) = (0usize, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            Ordering::Less => {
                union += 1;
                i += 1;
            }
            Ordering::Greater => {
                union += 1;
                j += 1;
            }
            Ordering::Equal => {
                intersection += 1;
                union += 1;
                i += 1;
                j += 1;
            }
        }
    }
    union += a.len() - i + (b.len() - j);

    if union == 0 {
        1.0
    } else {
        intersection as f64 / union as f64
    }
}
