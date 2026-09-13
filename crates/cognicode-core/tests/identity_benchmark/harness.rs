//! E38 M3 identity benchmark harness core (design D7).
//!
//! Runs the tiered continuity matcher (WU-3) over the ground-truth
//! `sandbox/fixtures/lsi-identity/<case>/` fixture cases and scores the
//! result against the DECLARED gates:
//!
//! - **Fixture path** (design Data Flow): per case, `before/` and `after/`
//!   sources are walked in sorted order (paths RELATIVE to the side dir, so
//!   identity strings are `src/...:name:line`), extracted with
//!   `extract_file` → `FactBatchBuilder` (e37 D3, untouched), committed
//!   into a fresh kernel store (bootstrap → commit → `facts_in_snapshot`),
//!   and the READ-BACK facts feed [`match_snapshots`] together with the
//!   fixture-DECLARED rename evidence (`evidence.json` — fixtures are NOT
//!   git repos; the git adapter itself is proven on throwaway repos by the
//!   WU-2 tests).
//! - **Expected mapping** (`expected-mapping.json`, design schema): the
//!   complete ground truth per case — every expected Matched pair (with
//!   optional tier), New, Terminated, and Ambiguous (with candidates) is
//!   verified against the matcher's [`ContinuityResult`]; any unexplained
//!   actual outcome fails the case.
//! - **Scoring**: precision/recall over the Matched-pair multiset, pooled
//!   across all cases; line-shift retention (recall of the `line-shift`
//!   case) and move retention (recall pooled over the move cases) are
//!   dedicated gates. `Ambiguous` outcomes count as NEITHER match nor miss
//!   and are reported separately; expected-Ambiguous returning anything
//!   else FAILS the case (inverted fail-closed check).
//! - **Convention pin** (spec "Separate pinned convention digest"): the
//!   matcher/fingerprint convention is pinned through
//!   [`PINNED_MATCHER_DIGEST`], hashed FNV-1a 64 over
//!   [`MATCHER_CONVENTION`] — a digest SEPARATE from e37's
//!   `PINNED_IDENTITY_DIGEST` (`fnv1a64:efccc22e912913fe`). Changing any
//!   tier rule, fingerprint element, pool order, or pinned constant
//!   changes the digest and fails the run until explicitly re-pinned.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use cognicode_core::application::fact_bridge::FactBatchBuilder;
use cognicode_core::application::ingest::extractor::extract_file;
use cognicode_core::domain::evidence_kernel::bootstrap::bootstrap_registry;
use cognicode_core::domain::evidence_kernel::continuity::{
    ContinuityOutcome, ContinuityResult, ContinuityStatus, MatchTier, MatcherThresholds,
    SnapshotEntityView, match_snapshots,
};
use cognicode_core::domain::evidence_kernel::fact::Fact;
use cognicode_core::domain::evidence_kernel::ids::SnapshotId;
use cognicode_core::domain::evidence_kernel::ports::{FactStore, FileRename};
use cognicode_core::domain::value_objects::{WalkFilter, WorkspaceId};
use cognicode_core::infrastructure::evidence_kernel::in_memory::{
    InMemoryFactStore, InMemorySchemaRegistry,
};
use cognicode_core::infrastructure::parser::LanguageConfig;
use ignore::WalkBuilder;
use serde::Deserialize;
use serde::Serialize;

// ============================================================================
// Declared gates (spec "Pinned scoring gates and fixture coverage") —
// declared BEFORE any scoring.
// ============================================================================

/// Minimum rename precision (matched pairs that are in the ground truth).
pub const PRECISION_THRESHOLD: f64 = 0.95;

/// Minimum rename recall (ground-truth pairs the matcher recovered).
pub const RECALL_THRESHOLD: f64 = 0.90;

/// Line-shift identity retention: the `line-shift` case must recover EVERY
/// expected mapping (100%).
pub const LINE_SHIFT_RETENTION_THRESHOLD: f64 = 1.00;

/// File-move identity retention: the move cases pooled must recover at
/// least 99% of their expected mappings.
pub const MOVE_RETENTION_THRESHOLD: f64 = 0.99;

/// The case exercising the line-shift retention gate.
pub const LINE_SHIFT_CASE: &str = "line-shift";

/// The cases pooled into the move retention gate.
pub const MOVE_CASES: [&str; 2] = ["move", "move-edit"];

/// The case whose ambiguity is quarantined and reported separately.
pub const COLLIDING_CASE: &str = "colliding-names";

/// Every ground-truth case the harness visits (design D7: seven cases —
/// rename/move with and without edits, line shift, colliding names,
/// unchanged control).
pub const ALL_CASES: [&str; 7] = [
    "control",
    "colliding-names",
    "line-shift",
    "move",
    "move-edit",
    "pure-rename",
    "rename-edit",
];

/// The pinned workspace/snapshots every case commits into (per-case store).
pub const WORKSPACE: &str = "ws-e38-harness";
pub const SNAPSHOT_BEFORE: SnapshotId = SnapshotId::new(1);
pub const SNAPSHOT_AFTER: SnapshotId = SnapshotId::new(2);

// ============================================================================
// Convention pin (spec "Separate pinned convention digest")
// ============================================================================

/// The pinned matcher/fingerprint convention (design D6). Changing ANY part
/// of this text — tier order, fingerprint composition, pool order,
/// fail-closed rules, or a pinned constant — changes the digest and fails
/// the harness until [`PINNED_MATCHER_DIGEST`] is explicitly re-pinned
/// (re-pin duty).
pub const MATCHER_CONVENTION: &str = "e38 matcher convention v1: tiers run T0 exact FQN -> T1 path+name+kind -> \
T2 declared rename evidence (similarity >= rename_floor, name+kind equal) -> \
T3 tagged-multiset Jaccard >= jaccard_threshold with kind hard pre-filter and \
bodyless fail-closed; fingerprint elements 'name:<n>'/'call:<c>'/'ref:<r>' \
multisets from EntityFacts; ambiguity fail-closed: >1 candidate or best minus \
second <= epsilon yields terminal Ambiguous listing candidates with no stable id; \
stable ids: before pool 1..K in sorted-FQN order, matches inherit, New = K+1.. in \
sorted-FQN order, no history table (no resurrection); entities are core:defines \
subjects; identity grammar '{file}:{name}:{line}' with 1-based line; pinned \
constants jaccard_threshold=0.6 epsilon=0.05 rename_floor=0.5";

/// The digest of the convention the benchmark was pinned against.
pub const PINNED_MATCHER_DIGEST: &str = "fnv1a64:e79f623705344f98";

/// e37's pinned fact-identity digest — recorded here ONLY so the harness
/// can assert the two conventions stay separate digests (spec "Separate
/// pinned convention digest").
pub const E37_FACT_IDENTITY_DIGEST: &str = "fnv1a64:efccc22e912913fe";

/// FNV-1a 64-bit digest (deterministic, dependency-free; e37 D7 pattern).
fn fnv1a64(data: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// Digest of an arbitrary convention text (exposed for the re-pin scenario
/// test, which simulates a convention change).
pub fn matcher_convention_digest_for(convention: &str) -> String {
    fnv1a64(convention)
}

/// Digest of the CURRENT matcher convention.
pub fn matcher_convention_digest() -> String {
    matcher_convention_digest_for(MATCHER_CONVENTION)
}

/// Verifies `digest` against the pinned baseline. Err instructs an explicit
/// re-pin (spec scenario "Convention change requires explicit re-pin").
pub fn verify_matcher_pin(digest: &str) -> Result<(), String> {
    if digest == PINNED_MATCHER_DIGEST {
        Ok(())
    } else {
        Err(format!(
            "matcher convention changed (digest {digest} != pinned \
             {PINNED_MATCHER_DIGEST}): re-pin PINNED_MATCHER_DIGEST explicitly \
             after reviewing the convention change (explicit re-pin required)"
        ))
    }
}

// ============================================================================
// Expected ground truth (design `expected-mapping.json` schema)
// ============================================================================

/// The expected status of one ground-truth entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpectedStatus {
    /// The (before, after) pair must be Matched.
    Matched,
    /// The after-only entity must be New.
    New,
    /// The before-only entity must be Terminated.
    Terminated,
    /// The after entity must be Ambiguous with the expected candidates.
    Ambiguous,
}

/// One ground-truth entity of a case (design schema: per-entity
/// before/after identity + expected status, optional tier, candidates iff
/// Ambiguous).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedEntity {
    /// The before identity string (`null` for New/after-only).
    pub before: Option<String>,
    /// The after identity string (`null` for Terminated/before-only).
    pub after: Option<String>,
    /// The expected continuity status.
    pub status: ExpectedStatus,
    /// Optional expected tier name (`ExactIdentity` | `PathNameKind` |
    /// `RenameEvidence` | `Fingerprint`); checked when present.
    #[serde(default)]
    pub tier: Option<String>,
    /// The expected candidate set (required iff Ambiguous).
    #[serde(default)]
    pub candidates: Vec<String>,
}

/// The complete ground truth of one fixture case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpectedMapping {
    /// Schema version (currently 1).
    pub schema_version: u32,
    /// The case name.
    pub case: String,
    /// Every expected entity of the case.
    pub entities: Vec<ExpectedEntity>,
}

/// Loads and parses a case's `expected-mapping.json`.
pub fn load_expected_mapping(case_dir: &Path) -> ExpectedMapping {
    let raw = std::fs::read_to_string(case_dir.join("expected-mapping.json"))
        .expect("read expected-mapping.json");
    serde_json::from_str(&raw).expect("parse expected-mapping.json")
}

/// Loads a case's declared rename evidence (`evidence.json`), or an EMPTY
/// evidence set when the file is absent (no-move cases fall through the T2
/// tier exactly like a fail-closed adapter returning no evidence).
pub fn load_evidence(case_dir: &Path) -> Vec<FileRename> {
    let path = case_dir.join("evidence.json");
    if !path.exists() {
        return Vec::new();
    }
    let raw = std::fs::read_to_string(&path).expect("read evidence.json");
    serde_json::from_str(&raw).expect("parse evidence.json")
}

// ============================================================================
// Fixture loading (design Data Flow: extract → batch → commit → read back)
// ============================================================================

/// Extracts one side of a case into a canonical fact batch: files in sorted
/// order (same walk semantics as the e37 oracle: `ignore` walker, hidden +
/// git-ignore respected, `WalkFilter` blocklist), each extracted under its
/// path RELATIVE to the side dir so identity strings are `src/...:name:line`.
pub fn case_facts(case_dir: &Path, side: &str, snapshot: SnapshotId) -> Vec<Fact> {
    let root = case_dir.join(side);
    let mut builder = FactBatchBuilder::new(snapshot);
    for (rel, config) in sources_under(&root) {
        let source = std::fs::read_to_string(root.join(&rel)).expect("read fixture source");
        let result = extract_file(config, Path::new(&rel), &source, "e38-identity-harness");
        builder.add_extraction(&result);
    }
    builder.finish()
}

/// Recognized sources under `root` as `(relative path, language config)`
/// pairs, sorted for determinism.
fn sources_under(root: &Path) -> Vec<(String, &'static LanguageConfig)> {
    let walk_filter = WalkFilter::default();
    let mut sources: Vec<(String, &'static LanguageConfig)> = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| {
            let absolute = entry.path();
            if walk_filter.matches_any_component(absolute) {
                return None;
            }
            let rel = absolute
                .strip_prefix(root)
                .ok()?
                .to_string_lossy()
                .into_owned();
            let ext = absolute.extension()?.to_str()?;
            let config = LanguageConfig::from_extension(ext)?;
            Some((rel, config))
        })
        .collect();
    sources.sort_by(|a, b| a.0.cmp(&b.0));
    sources
}

/// Commits both sides of a case into a fresh kernel store (bootstrap →
/// commit → `facts_in_snapshot`) and returns the READ-BACK facts — the
/// design Data Flow path (the matcher consumes store reads, not raw
/// extraction output).
pub fn committed_case_facts(case_dir: &Path) -> (Vec<Fact>, Vec<Fact>) {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).expect("canonical bootstrap registers the core:* set");
    let store = InMemoryFactStore::new(Arc::new(registry));
    let workspace = WorkspaceId::try_new(WORKSPACE).expect("valid workspace");

    let before = read_back(
        &store,
        &workspace,
        SNAPSHOT_BEFORE,
        case_facts(case_dir, "before", SNAPSHOT_BEFORE),
    );
    let after = read_back(
        &store,
        &workspace,
        SNAPSHOT_AFTER,
        case_facts(case_dir, "after", SNAPSHOT_AFTER),
    );
    (before, after)
}

/// Commits `facts` and reads the snapshot back through the store.
fn read_back(
    store: &InMemoryFactStore,
    workspace: &WorkspaceId,
    snapshot: SnapshotId,
    facts: Vec<Fact>,
) -> Vec<Fact> {
    let expected_len = facts.len();
    let committed = tokio_test::block_on(store.commit(workspace, &snapshot, facts))
        .expect("canonical facts commit");
    assert_eq!(
        committed.len(),
        expected_len,
        "every committed fact returns an id"
    );
    tokio_test::block_on(store.facts_in_snapshot(workspace, &snapshot)).expect("pinned read")
}

// ============================================================================
// Case verification (pure — testable without fixtures)
// ============================================================================

/// One verified expected-Ambiguous finding, reported SEPARATELY from the
/// scored precision/recall multisets (quarantine pattern).
#[derive(Debug, Clone)]
pub struct AmbiguousFinding {
    /// The case the finding belongs to.
    pub case: String,
    /// The after identity string that was expected to be Ambiguous.
    pub after: String,
    /// The expected candidate set from the ground truth.
    pub expected_candidates: Vec<String>,
    /// The actual candidate set (empty when the status differed).
    pub actual_candidates: Vec<String>,
    /// True when the actual status was Ambiguous with EXACTLY the expected
    /// candidate set.
    pub matched: bool,
}

/// One case's verification report.
#[derive(Debug, Clone)]
pub struct CaseReport {
    /// The case name.
    pub case: String,
    /// True when every expectation verified and no actual outcome was
    /// unexplained.
    pub passed: bool,
    /// The actual Matched pairs `(before_fqn, after_fqn)` declared by the
    /// ground truth (the scoring multiset — ambiguous pairs NEVER enter).
    pub actual_matched: Vec<(String, String)>,
    /// The expected Matched pairs (the ground-truth scoring multiset).
    pub expected_matched: Vec<(String, String)>,
    /// The separately-reported ambiguous findings.
    pub ambiguous: Vec<AmbiguousFinding>,
    /// Named failures (empty when passed).
    pub failures: Vec<String>,
}

impl CaseReport {
    /// One-line human-readable report entry.
    pub fn describe(&self) -> String {
        format!(
            "case '{}' [{}]: matched={}/expected={} ambiguous={} failures={}",
            self.case,
            if self.passed { "PASS" } else { "FAIL" },
            self.actual_matched.len(),
            self.expected_matched.len(),
            self.ambiguous.len(),
            self.failures.join(" | "),
        )
    }
}

/// The before-pool identity strings in the matcher's pool order
/// (`StableEntityId(k)` ↔ `pool[k-1]`; the sorted-FQN pool order is part of
/// the pinned [`MATCHER_CONVENTION`]).
pub fn before_pool_fqns(before: &[Fact]) -> Vec<String> {
    let view = SnapshotEntityView::from_facts(before, SNAPSHOT_BEFORE);
    let mut entities: Vec<&cognicode_core::domain::evidence_kernel::continuity::EntityFacts> =
        view.entities.values().collect();
    entities.sort_by(|a, b| a.fqn.cmp(&b.fqn).then_with(|| a.entity.cmp(&b.entity)));
    entities.into_iter().map(|e| e.fqn.clone()).collect()
}

/// The serde-variant name of a [`MatchTier`] (the `tier` strings of the
/// expected-mapping schema).
fn tier_name(tier: &MatchTier) -> &'static str {
    match tier {
        MatchTier::ExactIdentity => "ExactIdentity",
        MatchTier::PathNameKind => "PathNameKind",
        MatchTier::RenameEvidence { .. } => "RenameEvidence",
        MatchTier::Fingerprint { .. } => "Fingerprint",
    }
}

/// Verifies one case's actual [`ContinuityResult`] against the complete
/// ground truth (pure function so the inverted fail-closed scenarios are
/// testable without fixtures).
pub fn verify_mapping(
    case: &str,
    expected: &ExpectedMapping,
    result: &ContinuityResult,
    before_pool: &[String],
) -> CaseReport {
    let mut failures = Vec::new();
    let mut actual_matched = Vec::new();
    let mut expected_matched = Vec::new();
    let mut ambiguous = Vec::new();

    let after_outcomes: BTreeMap<&str, &ContinuityOutcome> = result
        .outcomes
        .iter()
        .filter(|o| o.snapshot == SNAPSHOT_AFTER)
        .map(|o| (o.fqn.as_str(), o))
        .collect();
    let before_outcomes: BTreeMap<&str, &ContinuityOutcome> = result
        .outcomes
        .iter()
        .filter(|o| o.snapshot == SNAPSHOT_BEFORE)
        .map(|o| (o.fqn.as_str(), o))
        .collect();

    for entity in &expected.entities {
        match &entity.status {
            ExpectedStatus::Matched => {
                let (Some(before), Some(after)) = (&entity.before, &entity.after) else {
                    failures.push(format!(
                        "case {case}: Matched expectation needs BOTH identities"
                    ));
                    continue;
                };
                expected_matched.push((before.clone(), after.clone()));
                let Some(outcome) = after_outcomes.get(after.as_str()) else {
                    failures.push(format!("case {case}: no outcome for {after}"));
                    continue;
                };
                let ContinuityStatus::Matched { tier, .. } = &outcome.status else {
                    failures.push(format!(
                        "case {case}: {after} expected Matched (to {before}), got {:?}",
                        outcome.status
                    ));
                    continue;
                };
                // Resolve the claimed before side through the pool order
                // (the stable id IS the pool position).
                let claimed = outcome
                    .stable_id
                    .and_then(|id| before_pool.get((id.get() as usize).checked_sub(1)?));
                if claimed != Some(before) {
                    failures.push(format!(
                        "case {case}: {after} matched {:?}, expected {before}",
                        claimed
                    ));
                    continue;
                }
                actual_matched.push((before.clone(), after.clone()));
                if let Some(want) = &entity.tier
                    && tier_name(tier) != want.as_str()
                {
                    failures.push(format!(
                        "case {case}: {after} matched at tier {:?}, expected {want}",
                        tier_name(tier)
                    ));
                }
            }
            ExpectedStatus::New => {
                let Some(after) = &entity.after else {
                    failures.push(format!(
                        "case {case}: New expectation needs the after identity"
                    ));
                    continue;
                };
                match after_outcomes.get(after.as_str()).map(|o| &o.status) {
                    Some(ContinuityStatus::New) => {}
                    other => {
                        failures.push(format!("case {case}: {after} expected New, got {other:?}"))
                    }
                }
            }
            ExpectedStatus::Terminated => {
                let Some(before) = &entity.before else {
                    failures.push(format!(
                        "case {case}: Terminated expectation needs the before identity"
                    ));
                    continue;
                };
                match before_outcomes.get(before.as_str()).map(|o| &o.status) {
                    Some(ContinuityStatus::Terminated) => {}
                    other => failures.push(format!(
                        "case {case}: {before} expected Terminated, got {other:?}"
                    )),
                }
            }
            ExpectedStatus::Ambiguous => {
                let Some(after) = &entity.after else {
                    failures.push(format!(
                        "case {case}: Ambiguous expectation needs the after identity"
                    ));
                    continue;
                };
                let mut expected_candidates = entity.candidates.clone();
                expected_candidates.sort();
                let (actual_candidates, matched_finding) =
                    match after_outcomes.get(after.as_str()).map(|o| &o.status) {
                        Some(ContinuityStatus::Ambiguous { candidates }) => {
                            let mut actual = candidates.clone();
                            actual.sort();
                            let equal = actual == expected_candidates;
                            if !equal {
                                failures.push(format!(
                                    "case {case}: {after} Ambiguous candidates {actual:?} \
                                     != expected {expected_candidates:?}"
                                ));
                            }
                            (actual, equal)
                        }
                        // Inverted fail-closed: expected-Ambiguous returning
                        // anything else FAILS the case.
                        other => {
                            failures.push(format!(
                                "case {case}: {after} expected Ambiguous (inverted fail-closed), \
                                 got {other:?}"
                            ));
                            (Vec::new(), false)
                        }
                    };
                ambiguous.push(AmbiguousFinding {
                    case: case.to_string(),
                    after: after.clone(),
                    expected_candidates,
                    actual_candidates,
                    matched: matched_finding,
                });
            }
        }
    }

    // Completeness: the expected mapping is the FULL ground truth — every
    // actual outcome must be explained by an expectation.
    let expected_after: BTreeSet<&str> = expected
        .entities
        .iter()
        .filter_map(|e| e.after.as_deref())
        .collect();
    let expected_before: BTreeSet<&str> = expected
        .entities
        .iter()
        .filter_map(|e| e.before.as_deref())
        .collect();
    for (fqn, outcome) in &after_outcomes {
        if !expected_after.contains(fqn) {
            failures.push(format!(
                "case {case}: unexplained after outcome {fqn} ({:?})",
                outcome.status
            ));
        }
    }
    for (fqn, outcome) in &before_outcomes {
        if !expected_before.contains(fqn) {
            failures.push(format!(
                "case {case}: unexplained before outcome {fqn} ({:?})",
                outcome.status
            ));
        }
    }

    CaseReport {
        case: case.to_string(),
        passed: failures.is_empty(),
        actual_matched,
        expected_matched,
        ambiguous,
        failures,
    }
}

// ============================================================================
// Gates (spec "Pinned scoring gates and fixture coverage")
// ============================================================================

/// The scored gate report.
#[derive(Debug, Clone)]
pub struct GateReport {
    /// Pooled precision over the matched-pair multisets (1.0 when the
    /// matcher claimed no pairs).
    pub precision: f64,
    /// Pooled recall over the matched-pair multisets (1.0 when the ground
    /// truth expects no pairs).
    pub recall: f64,
    /// Retention (recall) of the `line-shift` case — must be 100%.
    pub line_shift_retention: f64,
    /// Retention (recall) pooled over the move cases.
    pub move_retention: f64,
    /// Whether every gate is met.
    pub passed: bool,
    /// Named gate failures (gate + measured value).
    pub failures: Vec<String>,
}

/// Multiset intersection size (count-aware, e37 `multiset_jaccard`
/// discipline).
pub fn multiset_intersection_count<T: Ord + Clone>(a: &[T], b: &[T]) -> usize {
    let mut a = a.to_vec();
    let mut b = b.to_vec();
    a.sort();
    b.sort();
    let (mut i, mut j) = (0usize, 0usize);
    let mut common = 0usize;
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                common += 1;
                i += 1;
                j += 1;
            }
        }
    }
    common
}

/// Recall-style retention of one report: recovered expected pairs over
/// expected pairs (1.0 when the case expects no pairs; a MISSING report is
/// 0.0 — fail-closed).
fn retention(report: Option<&CaseReport>) -> f64 {
    match report {
        None => 0.0,
        Some(report) if report.expected_matched.is_empty() => 1.0,
        Some(report) => {
            multiset_intersection_count(&report.actual_matched, &report.expected_matched) as f64
                / report.expected_matched.len() as f64
        }
    }
}

/// Evaluates the declared gates over the case reports (pure function so the
/// "missed gate fails the run" scenarios are testable without fixtures).
pub fn evaluate_gates(reports: &[CaseReport]) -> GateReport {
    let actual: Vec<(String, String)> = reports
        .iter()
        .flat_map(|r| r.actual_matched.iter().cloned())
        .collect();
    let expected: Vec<(String, String)> = reports
        .iter()
        .flat_map(|r| r.expected_matched.iter().cloned())
        .collect();
    let true_positives = multiset_intersection_count(&actual, &expected);
    let precision = if actual.is_empty() {
        1.0
    } else {
        true_positives as f64 / actual.len() as f64
    };
    let recall = if expected.is_empty() {
        1.0
    } else {
        true_positives as f64 / expected.len() as f64
    };

    let find = |name: &str| reports.iter().find(|r| r.case == name);
    let line_shift_retention = retention(find(LINE_SHIFT_CASE));
    let move_actual: Vec<(String, String)> = MOVE_CASES
        .iter()
        .filter_map(|name| find(name))
        .flat_map(|r| r.actual_matched.iter().cloned())
        .collect();
    let move_expected: Vec<(String, String)> = MOVE_CASES
        .iter()
        .filter_map(|name| find(name))
        .flat_map(|r| r.expected_matched.iter().cloned())
        .collect();
    let move_retention = if move_expected.is_empty() {
        1.0
    } else {
        multiset_intersection_count(&move_actual, &move_expected) as f64
            / move_expected.len() as f64
    };

    let mut failures = Vec::new();
    if precision < PRECISION_THRESHOLD {
        failures.push(format!(
            "precision gate missed: measured {precision:.4} < required {PRECISION_THRESHOLD}"
        ));
    }
    if recall < RECALL_THRESHOLD {
        failures.push(format!(
            "recall gate missed: measured {recall:.4} < required {RECALL_THRESHOLD}"
        ));
    }
    if line_shift_retention < LINE_SHIFT_RETENTION_THRESHOLD {
        failures.push(format!(
            "line-shift retention gate missed: measured {line_shift_retention:.4} < \
             required {LINE_SHIFT_RETENTION_THRESHOLD}"
        ));
    }
    if move_retention < MOVE_RETENTION_THRESHOLD {
        failures.push(format!(
            "move retention gate missed: measured {move_retention:.4} < required \
             {MOVE_RETENTION_THRESHOLD}"
        ));
    }
    GateReport {
        precision,
        recall,
        line_shift_retention,
        move_retention,
        passed: failures.is_empty(),
        failures,
    }
}

// ============================================================================
// Run
// ============================================================================

/// The outcome of one full harness run.
#[derive(Debug, Clone)]
pub struct HarnessRun {
    /// Per-case reports (quarantined ambiguous included).
    pub reports: Vec<CaseReport>,
    /// Digest of the matcher convention observed during this run.
    pub digest: String,
    /// Whether the observed convention matches the pinned digest.
    pub digest_ok: bool,
    /// The scored gates.
    pub gate: GateReport,
    /// Final verdict (pin + every case + every gate).
    pub passed: bool,
    /// Named failures (empty when passed).
    pub failures: Vec<String>,
}

/// Runs one fixture case end to end: load → commit → match → verify.
pub fn run_case(name: &str, case_dir: &Path) -> CaseReport {
    let (before, after) = committed_case_facts(case_dir);
    let evidence = load_evidence(case_dir);
    let expected = load_expected_mapping(case_dir);
    let result = match_snapshots(&before, &after, &evidence, &MatcherThresholds::default());
    let pool = before_pool_fqns(&before);
    verify_mapping(name, &expected, &result, &pool)
}

/// Runs the full harness over the `lsi-identity` fixture root.
pub fn run(fixtures_root: &Path) -> HarnessRun {
    let digest = matcher_convention_digest();
    let digest_ok = verify_matcher_pin(&digest).is_ok();

    let mut reports = Vec::new();
    for name in ALL_CASES {
        reports.push(run_case(name, &fixtures_root.join(name)));
    }

    let gate = evaluate_gates(&reports);
    let mut failures = gate.failures.clone();
    for report in &reports {
        failures.extend(report.failures.iter().cloned());
    }
    if !digest_ok {
        failures.push(
            "matcher convention changed: explicit re-pin of PINNED_MATCHER_DIGEST \
             is required before the run can pass"
                .to_string(),
        );
    }
    let passed = digest_ok && gate.passed && reports.iter().all(|r| r.passed);
    HarnessRun {
        reports,
        digest,
        digest_ok,
        gate,
        passed,
        failures,
    }
}
