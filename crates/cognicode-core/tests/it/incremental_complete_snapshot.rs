//! CGH-026 (CAA-08): IncrementalComplete with verifiable logical
//! coverage — partial reruns plus proven reuse, judged honestly.
//!
//! Each test names the AC it pins:
//! #1 IncrementalComplete and FullRebuild agree, over the same frozen
//!    origin, on findings AND completeness — via the real planner;
//! #2 a parse error, a missing capability, an uncovered plugin or a
//!    truncated read-set NEVER receives Pass;
//! #3 identical inputs yield identical results and reason codes (the
//!    one owner both surfaces will call);
//! #4 adversarial capture: a change during capture and a corrupt
//!    cache never mix evidence of two snapshots.

use std::collections::BTreeMap;

use cognicode_core::application::change_tracking::complete_snapshot::{
    AnalysisMode, CaptureGap, CompleteSnapshotRequest, ReasonCode, Resolution, Verdict,
    build_complete_snapshot, load_reuse_index,
};
use cognicode_core::application::change_tracking::planner::{
    AffectedWorkPlan, ExecutionDependency, SchedulingReason, WorkDecision, WorkDisposition, WorkId,
    plan_affected_work,
};
use cognicode_core::domain::evidence_kernel::fact::{
    Fact, FactValue, ProducerKind, ProvenanceRecord,
};
use cognicode_core::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::evidence_kernel::semantic_diff::compute_fact_delta;
use cognicode_core::domain::findings::detector_reuse::{
    CachedFinding, CachedObservation, DetectorReuseIndex, DetectorRunIdentity, DetectorRunRecord,
    OutputLocation, TypedEvidenceRecord, semantic_fingerprint,
};
use cognicode_core::domain::findings::digest::DetectorDigest;
use cognicode_core::domain::findings::{
    AnalysisCapability, DetectorAuthority, DetectorDigests, DetectorId,
};
use cognicode_core::domain::kernel_ids::ExecutionId;
use cognicode_core::domain::naming::NamespacedName;
use cognicode_core::domain::readset::ReadSet;
use cognicode_core::domain::value_objects::Provenance;

fn snap(n: u64) -> SnapshotId {
    SnapshotId(n)
}

fn work(name: &str) -> WorkId {
    WorkId::new(NamespacedName::new(name).expect("valid namespaced name"))
}

fn dig(content: &str) -> DetectorDigest {
    DetectorDigest::from_content(content)
}

fn identity(key: &str) -> DetectorRunIdentity {
    DetectorRunIdentity {
        detector_id: DetectorId::new(key).expect("valid detector id"),
        version: "1".to_string(),
        authority: DetectorAuthority::Gated,
        digests: DetectorDigests {
            logic: dig("logic"),
            policy: dig("policy"),
            semantic: dig(&format!("sem-{key}")),
            instance: dig("instance"),
        },
        effective_capabilities: [AnalysisCapability::AstPattern].into_iter().collect(),
        parser: dig("parser"),
        sanitizer: dig("sanitizer"),
    }
}

fn finding(key: &str, subject: &str) -> CachedFinding {
    CachedFinding {
        semantic_fingerprint: semantic_fingerprint(&dig(&format!("sem-{key}")), subject, "warn"),
        output_locations: vec![OutputLocation {
            file: format!("{key}.src"),
            line: 3,
            column: 1,
        }],
        observation: CachedObservation::Advisory {
            snapshot_id: 0,
            evidence: vec![TypedEvidenceRecord {
                evidence: 7,
                snapshot: 0,
            }],
            message: format!("advisory from {key}"),
        },
    }
}

fn record(key: &str, produced: SnapshotId, truncated: bool) -> DetectorRunRecord {
    DetectorRunRecord {
        identity: identity(key),
        read_boundary: None,
        observed_facts: [FactId(50)].into_iter().collect(),
        observed_truncated: truncated,
        produced_snapshot: produced,
        findings: vec![finding(key, key)],
    }
}

fn provenance() -> ProvenanceRecord {
    ProvenanceRecord::new(
        Provenance::Extracted,
        ProducerKind::DeterministicAnalyzer,
        None,
    )
}

fn fact(id: u64, subject: u64, object: FactValue, snapshot: SnapshotId) -> Fact {
    Fact::new(
        FactId(id),
        EntityId(subject),
        RelationKind::try_new("core:calls").expect("valid predicate"),
        object,
        snapshot,
        provenance(),
    )
    .expect("valid fact")
}

/// The one transition the whole file speaks about: the callee of
/// `detector.alpha` moved (fact 10 → 11 at the same semantic key), so
/// the real planner names alpha `Affected` and leaves beta and gamma
/// `Unaffected`.
fn delta() -> cognicode_core::domain::evidence_kernel::semantic_diff::FactDelta {
    compute_fact_delta(
        snap(1),
        vec![
            fact(10, 100, FactValue::Ref(EntityId(200)), snap(1)),
            fact(50, 300, FactValue::Text("stable".into()), snap(1)),
        ],
        snap(2),
        vec![
            fact(11, 100, FactValue::Ref(EntityId(300)), snap(2)),
            fact(50, 300, FactValue::Text("stable".into()), snap(2)),
        ],
    )
    .expect("valid delta")
}

fn base_plan() -> AffectedWorkPlan {
    plan_affected_work(
        &delta(),
        &[
            ExecutionDependency {
                work: work("detector.alpha"),
                execution: ExecutionId(1),
                read_set: [FactId(10)].into_iter().collect::<ReadSet>(),
                read_scope: None,
            },
            ExecutionDependency {
                work: work("detector.beta"),
                execution: ExecutionId(2),
                read_set: [FactId(50)].into_iter().collect::<ReadSet>(),
                read_scope: None,
            },
            ExecutionDependency {
                work: work("detector.gamma"),
                execution: ExecutionId(3),
                read_set: ReadSet::from_iter([]),
                read_scope: None,
            },
        ],
    )
}

/// Cache of the base snapshot for beta and gamma, the identities a
/// rerun would use now (unchanged), and the fresh record of alpha.
fn parts() -> (
    DetectorReuseIndex,
    BTreeMap<String, DetectorRunIdentity>,
    BTreeMap<String, DetectorRunRecord>,
) {
    let mut cache = DetectorReuseIndex::empty();
    cache.remember(record("detector.beta", snap(1), false));
    cache.remember(record("detector.gamma", snap(1), false));
    let mut ids = BTreeMap::new();
    for key in ["detector.beta", "detector.gamma", "detector.alpha"] {
        ids.insert(key.to_string(), identity(key));
    }
    let mut reruns = BTreeMap::new();
    reruns.insert(
        "detector.alpha".to_string(),
        record("detector.alpha", snap(2), false),
    );
    (cache, ids, reruns)
}

fn build(
    mode: AnalysisMode,
    cache: &DetectorReuseIndex,
    ids: &BTreeMap<String, DetectorRunIdentity>,
    reruns: &BTreeMap<String, DetectorRunRecord>,
    gaps: &[CaptureGap],
) -> cognicode_core::application::change_tracking::complete_snapshot::CompleteSnapshot {
    let plan = base_plan();
    let d = delta();
    build_complete_snapshot(&CompleteSnapshotRequest {
        mode,
        plan: &plan,
        candidate_snapshot: snap(2),
        delta: &d,
        cache,
        current_identities: ids,
        reruns,
        gaps,
    })
}

fn gap(code: ReasonCode) -> CaptureGap {
    CaptureGap {
        code,
        detail: "observed during capture".to_string(),
    }
}

// ---------------------------------------------------------------- AC #1

#[test]
fn incremental_complete_matches_full_rebuild_over_the_same_frozen_origin() {
    let (cache, ids, mut reruns) = parts();
    let incremental = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &[],
    );
    assert_eq!(incremental.verdict(), Verdict::Pass);

    // Full rebuild: every work executed fresh against the candidate.
    for key in ["detector.beta", "detector.gamma"] {
        reruns.insert(key.to_string(), record(key, snap(2), false));
    }
    let rebuilt = build(
        AnalysisMode::FullRebuild,
        &DetectorReuseIndex::empty(),
        &ids,
        &reruns,
        &[],
    );
    assert_eq!(rebuilt.verdict(), Verdict::Pass);

    assert_eq!(incremental.semantic_coverage(), rebuilt.semantic_coverage());
    assert_eq!(incremental.findings.len(), 3);

    // The incremental answer got there honestly: alpha was rerun
    // because the REAL planner named it Affected, beta and gamma were
    // reused because the cached records survived every layer.
    let resolution_of = |name: &str| {
        incremental
            .outcomes
            .iter()
            .find(|o| o.work == work(name))
            .map(|o| matches!(o.resolution, Resolution::Reran { .. }))
            .expect("work present")
    };
    assert!(resolution_of("detector.alpha"));
    assert!(!resolution_of("detector.beta"));
    assert!(!resolution_of("detector.gamma"));
}

#[test]
fn scoped_modes_never_claim_completeness_even_when_fully_resolved() {
    let (cache, ids, mut reruns) = parts();
    reruns.insert(
        "detector.beta".to_string(),
        record("detector.beta", snap(2), false),
    );
    reruns.insert(
        "detector.gamma".to_string(),
        record("detector.gamma", snap(2), false),
    );
    let focused = build(AnalysisMode::Focused, &cache, &ids, &reruns, &[]);
    assert_eq!(focused.verdict(), Verdict::Scoped);
    // Scoped still says nothing blocking: the question itself is the
    // limit, not the evidence.
    assert!(focused.blocking.is_empty());
}

// ---------------------------------------------------------------- AC #2

#[test]
fn capture_gaps_never_receive_pass_whatever_else_is_proven() {
    let (cache, ids, reruns) = parts();
    for code in [
        ReasonCode::ParseError,
        ReasonCode::CapabilityMissing,
        ReasonCode::PluginUncovered,
    ] {
        let snapshot = build(
            AnalysisMode::IncrementalComplete,
            &cache,
            &ids,
            &reruns,
            &[gap(code)],
        );
        assert_eq!(
            snapshot.verdict(),
            Verdict::NotPass,
            "gap {code} must block"
        );
        assert!(snapshot.reason_codes().contains(&code.as_str()));
        // Honesty does not erase what WAS proven — the findings of
        // resolved work stay visible, the gap says what is missing.
        assert_eq!(snapshot.findings.len(), 3);
    }
}

#[test]
fn a_truncated_fresh_read_set_never_passes() {
    let (cache, ids, mut reruns) = parts();
    reruns.insert(
        "detector.alpha".to_string(),
        record("detector.alpha", snap(2), true),
    );
    let snapshot = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &[],
    );
    assert_eq!(snapshot.verdict(), Verdict::NotPass);
    assert!(snapshot.reason_codes().contains(&"readset_truncated"));
    let alpha = snapshot
        .outcomes
        .iter()
        .find(|o| o.work == work("detector.alpha"))
        .expect("alpha present");
    assert!(matches!(alpha.resolution, Resolution::Unresolved { .. }));
    // Its unproven findings are NOT merged; the reused ones stand.
    assert_eq!(snapshot.findings.len(), 2);
}

#[test]
fn a_truncated_cached_read_set_never_passes() {
    let (mut cache, ids, reruns) = parts();
    cache.remember(record("detector.gamma", snap(1), true));
    let snapshot = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &[],
    );
    assert_eq!(snapshot.verdict(), Verdict::NotPass);
    assert!(snapshot.reason_codes().contains(&"readset_truncated"));
    // Lineage was incomplete at the ORIGINAL run: reusing "it did not
    // change" from it would collapse Unknown into Unaffected.
    let gamma = snapshot
        .outcomes
        .iter()
        .find(|o| o.work == work("detector.gamma"))
        .expect("gamma present");
    assert!(matches!(gamma.resolution, Resolution::Unresolved { .. }));
}

// ---------------------------------------------------------------- AC #3

#[test]
fn identical_inputs_produce_identical_results_and_reason_codes() {
    let (cache, ids, reruns) = parts();
    let gaps = [gap(ReasonCode::ParseError)];
    let first = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &gaps,
    );
    let second = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &gaps,
    );
    assert_eq!(first, second);
    assert_eq!(first.reason_codes(), second.reason_codes());
    // The passing case is equally reproducible for any surface.
    let a = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &[],
    );
    let b = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &[],
    );
    assert_eq!(a.reason_codes(), b.reason_codes());
    assert_eq!(a.semantic_coverage(), b.semantic_coverage());
}

// ---------------------------------------------------------------- AC #4

#[test]
fn a_cache_record_from_another_snapshot_is_refused_not_merged() {
    let (mut cache, ids, reruns) = parts();
    // Mid-capture change: the only evidence cached for beta names a
    // foreign snapshot the plan never diffed from.
    cache
        .records
        .retain(|r| r.identity.detector_id.as_str() != "detector.beta");
    cache.remember(record("detector.beta", snap(3), false));
    let snapshot = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &[],
    );
    assert_eq!(snapshot.verdict(), Verdict::NotPass);
    assert!(snapshot.reason_codes().contains(&"snapshot_mismatch"));
    assert!(
        !snapshot.findings.iter().any(|f| f.produced_in == snap(3)),
        "evidence of two snapshots must never be mixed"
    );
}

#[test]
fn the_record_of_the_planned_base_is_used_beside_records_of_other_bases() {
    let (mut cache, ids, reruns) = parts();
    cache.remember(record("detector.beta", snap(3), false));
    let snapshot = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &[],
    );
    assert!(
        !snapshot.reason_codes().contains(&"snapshot_mismatch"),
        "a record kept for another base must not shadow the planned base's"
    );
    assert!(
        !snapshot.findings.iter().any(|f| f.produced_in == snap(3)),
        "evidence of two snapshots must never be mixed"
    );
}

#[test]
fn a_fresh_record_for_the_wrong_candidate_is_refused() {
    let (cache, ids, mut reruns) = parts();
    reruns.insert(
        "detector.alpha".to_string(),
        record("detector.alpha", snap(9), false),
    );
    let snapshot = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &[],
    );
    assert_eq!(snapshot.verdict(), Verdict::NotPass);
    assert!(snapshot.reason_codes().contains(&"snapshot_mismatch"));
    assert_eq!(snapshot.findings.len(), 2);
}

#[test]
fn a_corrupt_cache_is_named_not_guessed() {
    assert_eq!(
        load_reuse_index("not json at all").err().map(|g| g.code),
        Some(ReasonCode::CacheCorrupt)
    );
    let foreign = DetectorReuseIndex {
        schema: "cognicode.detector-reuse/v999".to_string(),
        records: Default::default(),
    };
    let text = foreign.to_json().expect("serializable");
    assert_eq!(
        load_reuse_index(&text).err().map(|g| g.code),
        Some(ReasonCode::CacheCorrupt)
    );
    // And naming it honestly still cannot Pass.
    let (_, ids, reruns) = parts();
    let snapshot = build(
        AnalysisMode::IncrementalComplete,
        &DetectorReuseIndex::empty(),
        &ids,
        &reruns,
        &[gap(ReasonCode::CacheCorrupt)],
    );
    assert_eq!(snapshot.verdict(), Verdict::NotPass);
    assert!(snapshot.reason_codes().contains(&"cache_corrupt"));
}

#[test]
fn reused_findings_name_both_the_producing_and_the_candidate_snapshot() {
    let (cache, ids, reruns) = parts();
    let snapshot = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &[],
    );
    let reused = snapshot
        .findings
        .iter()
        .find(|f| f.produced_in == snap(1))
        .expect("beta or gamma reused");
    assert_eq!(reused.reported_in, snap(2));
    // The observation carries the candidate as its rendering
    // snapshot — the producing snapshot lives in provenance, never
    // overwritten.
    match &reused.observation {
        CachedObservation::Advisory { snapshot_id, .. } => assert_eq!(*snapshot_id, 2),
        other => panic!("expected advisory, got {other:?}"),
    }
}

#[test]
fn invalidated_reuse_without_a_rerun_blocks_and_with_one_falls_back() {
    let (cache, mut ids, reruns) = parts();
    // The rule layer of beta died (digests edited) while the planner
    // still said Unaffected: reuse alone cannot cover it.
    let mut broken = identity("detector.beta");
    broken.digests.logic = dig("logic-edited");
    ids.insert("detector.beta".to_string(), broken);
    let snapshot = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &reruns,
        &[],
    );
    assert_eq!(snapshot.verdict(), Verdict::NotPass);
    assert!(snapshot.reason_codes().contains(&"reuse_invalidated"));

    // Supplying the rerun resolves it honestly as Reran.
    let mut with_rerun = reruns.clone();
    with_rerun.insert(
        "detector.beta".to_string(),
        record("detector.beta", snap(2), false),
    );
    let healed = build(
        AnalysisMode::IncrementalComplete,
        &cache,
        &ids,
        &with_rerun,
        &[],
    );
    assert_eq!(healed.verdict(), Verdict::Pass);
    let beta = healed
        .outcomes
        .iter()
        .find(|o| o.work == work("detector.beta"))
        .expect("beta present");
    assert!(matches!(beta.resolution, Resolution::Reran { .. }));
}

#[test]
fn work_the_planner_named_unknown_without_a_rerun_is_unresolved() {
    // An unknown disposition demands fresh evidence; absence of a
    // rerun record is coverage failure, never a silent skip.
    let plan = AffectedWorkPlan {
        from_snapshot: Some(snap(1)),
        to_snapshot: Some(snap(2)),
        decisions: vec![WorkDecision {
            work: work("detector.delta"),
            disposition: WorkDisposition::Unknown,
            reasons: vec![SchedulingReason::UnknownDependency],
        }],
    };
    let delta = delta();
    let cache = DetectorReuseIndex::empty();
    let ids = BTreeMap::new();
    let reruns = BTreeMap::new();
    let snapshot = build_complete_snapshot(&CompleteSnapshotRequest {
        mode: AnalysisMode::IncrementalComplete,
        plan: &plan,
        candidate_snapshot: snap(2),
        delta: &delta,
        cache: &cache,
        current_identities: &ids,
        reruns: &reruns,
        gaps: &[],
    });
    assert_eq!(snapshot.verdict(), Verdict::NotPass);
    assert!(snapshot.reason_codes().contains(&"work_not_rerun"));
}

#[test]
fn a_plan_for_another_transition_is_not_this_candidates_coverage() {
    // Mid-capture change in the QUESTION itself: the plan answers
    // 1->2 while the caller claims candidate 9. Nothing may be
    // presented as coverage of 9.
    let (cache, ids, reruns) = parts();
    let plan = base_plan();
    let d = delta();
    let snapshot = build_complete_snapshot(&CompleteSnapshotRequest {
        mode: AnalysisMode::IncrementalComplete,
        plan: &plan,
        candidate_snapshot: snap(9),
        delta: &d,
        cache: &cache,
        current_identities: &ids,
        reruns: &reruns,
        gaps: &[],
    });
    assert_eq!(snapshot.verdict(), Verdict::NotPass);
    assert!(snapshot.reason_codes().contains(&"snapshot_mismatch"));
    assert!(
        snapshot.findings.is_empty(),
        "a snapshot of candidate 9 must not publish findings of transition 1->2"
    );
}

#[test]
fn completeness_over_an_empty_plan_is_a_false_green_not_a_pass() {
    let empty = AffectedWorkPlan {
        from_snapshot: Some(snap(1)),
        to_snapshot: Some(snap(2)),
        decisions: vec![],
    };
    let d = delta();
    let ids = BTreeMap::new();
    let reruns = BTreeMap::new();
    let cache = DetectorReuseIndex::empty();
    for mode in [AnalysisMode::IncrementalComplete, AnalysisMode::FullRebuild] {
        let snapshot = build_complete_snapshot(&CompleteSnapshotRequest {
            mode,
            plan: &empty,
            candidate_snapshot: snap(2),
            delta: &d,
            cache: &cache,
            current_identities: &ids,
            reruns: &reruns,
            gaps: &[],
        });
        assert_eq!(snapshot.verdict(), Verdict::NotPass);
        assert!(snapshot.reason_codes().contains(&"empty_work_plan"));
    }
    // A scoped mode over no work is honest about being scoped.
    let focused = build_complete_snapshot(&CompleteSnapshotRequest {
        mode: AnalysisMode::Focused,
        plan: &empty,
        candidate_snapshot: snap(2),
        delta: &d,
        cache: &cache,
        current_identities: &ids,
        reruns: &reruns,
        gaps: &[],
    });
    assert_eq!(focused.verdict(), Verdict::Scoped);
}
