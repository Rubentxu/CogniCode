//! CGH-025 (CAA-07): detector results are reused by layered identity
//! and never by luck.
//!
//! #1 a rerun with no invalidated input recovers equivalent findings;
//! #2 changing rule, capability, parser, sanitizer or a read fact
//!    invalidates exactly the applicable level;
//! #3 output location is not semantic identity — line moves and file
//!    renames keep findings, grounding changes do not;
//! #4 failures, advisory and inconclusive states travel as themselves
//!    and are never promoted to verified.

use std::collections::BTreeSet;

use cognicode_core::application::services::detector_reuse_store::DetectorReuseStore;
use cognicode_core::domain::evidence_kernel::fact::{
    Fact, FactValue, ProducerKind, ProvenanceRecord,
};
use cognicode_core::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::evidence_kernel::semantic_diff::compute_fact_delta;
use cognicode_core::domain::findings::detector_reuse::{
    CachedObservation, DetectorReuseIndex, DetectorRunIdentity, DetectorRunRecord,
    InvalidationLevel, OutputLocation, ReuseDecision, cache_observation, rebind_candidate,
    reuse_decision, semantic_fingerprint,
};
use cognicode_core::domain::findings::finding::CausalStepKind;
use cognicode_core::domain::findings::guard_transfer::GuardObservation;
use cognicode_core::domain::findings::projection::{
    CauseProjection, Gap, TypedEvidenceHandle, UnknownReason,
};
use cognicode_core::domain::findings::{
    AnalysisCapability, DetectorAuthority, DetectorDigest, DetectorDigests, DetectorExecutionRef,
    DetectorId, EvidenceId,
};

fn digests(tag: &str) -> DetectorDigests {
    DetectorDigests {
        logic: DetectorDigest::from_content(&format!("logic {tag}")),
        policy: DetectorDigest::from_content(&format!("policy {tag}")),
        semantic: DetectorDigest::from_content(&format!("semantic {tag}")),
        instance: DetectorDigest::from_content(&format!("instance {tag}")),
    }
}

fn identity(tag: &str) -> DetectorRunIdentity {
    DetectorRunIdentity {
        detector_id: DetectorId::new("security.sql_injection").expect("valid id"),
        version: "1.0.0".to_string(),
        authority: DetectorAuthority::Gated,
        digests: digests(tag),
        effective_capabilities: BTreeSet::from([
            AnalysisCapability::AstPattern,
            AnalysisCapability::Dataflow,
        ]),
        parser: DetectorDigest::from_content("parser v1"),
        sanitizer: DetectorDigest::from_content("sanitizer v1"),
    }
}

fn evidence_handle(evidence: u64, snapshot: u64) -> TypedEvidenceHandle {
    TypedEvidenceHandle {
        id: EvidenceId(evidence),
        snapshot: SnapshotId(snapshot),
    }
}

fn execution_ref() -> DetectorExecutionRef {
    DetectorExecutionRef::new(
        DetectorId::new("security.sql_injection").expect("valid id"),
        "1.0.0",
        DetectorAuthority::Gated,
        digests("v1"),
        None,
    )
    .expect("valid ref")
}

fn verifiable() -> GuardObservation {
    GuardObservation::Verifiable {
        snapshot_id: SnapshotId(1),
        detector: execution_ref(),
        evidence: vec![evidence_handle(7, 1)],
        causes: vec![CauseProjection::Grounded {
            kind: CausalStepKind::Source,
            subject: Some(EntityId(10)),
            fact: Some(FactId(100)),
            evidence: Some(EvidenceId(7)),
            detail: "request param".to_string(),
        }],
    }
}

fn record(observation: GuardObservation, observed: &[u64], truncated: bool) -> DetectorRunRecord {
    let fingerprint = semantic_fingerprint(
        &digests("v1").semantic,
        "app::handler::query",
        "security.sql_injection",
    );
    DetectorRunRecord {
        identity: identity("v1"),
        read_boundary: None,
        observed_facts: observed.iter().map(|f| FactId(*f)).collect(),
        observed_truncated: truncated,
        produced_snapshot: SnapshotId(1),
        findings: vec![
            cognicode_core::domain::findings::detector_reuse::CachedFinding {
                semantic_fingerprint: fingerprint,
                output_locations: vec![OutputLocation {
                    file: "src/api.rs".to_string(),
                    line: 42,
                    column: 9,
                }],
                observation: cache_observation(&observation),
            },
        ],
    }
}

fn provenance() -> ProvenanceRecord {
    ProvenanceRecord::new(
        Provenance::Extracted,
        ProducerKind::DeterministicAnalyzer,
        None,
    )
}

fn fact(id: u64, subject: u64, object: &str, snapshot: SnapshotId) -> Fact {
    Fact::new(
        FactId(id),
        EntityId(subject),
        RelationKind::try_new("core:defines").expect("valid predicate"),
        FactValue::Text(object.to_string()),
        snapshot,
        provenance(),
    )
    .expect("valid fact")
}

use cognicode_core::domain::value_objects::Provenance;

fn delta(
    from_facts: Vec<Fact>,
    to_facts: Vec<Fact>,
) -> cognicode_core::domain::evidence_kernel::semantic_diff::FactDelta {
    compute_fact_delta(SnapshotId(1), from_facts, SnapshotId(2), to_facts).expect("valid pair")
}

// ---- AC #1 ----

#[test]
fn a_rerun_without_invalidated_inputs_recovers_equivalent_findings() {
    let base = record(verifiable(), &[100, 101], false);
    let decision = reuse_decision(&base, &identity("v1"), None);
    assert_eq!(decision, ReuseDecision::Reusable);

    let rebound = rebind_candidate(&base, SnapshotId(2));
    assert_eq!(rebound.len(), 1);
    let finding = &rebound[0];
    // Equivalence: same semantic fingerprint, same evidence, same
    // causal chain; only the candidate reference moves.
    assert_eq!(finding.produced_in, SnapshotId(1));
    assert_eq!(finding.reported_in, SnapshotId(2));
    match &finding.observation {
        CachedObservation::Verifiable {
            snapshot_id,
            evidence,
            causes,
        } => {
            assert_eq!(*snapshot_id, 2);
            assert_eq!(evidence.len(), 1);
            assert_eq!(evidence[0].evidence, 7);
            // The evidence's own snapshot provenance is NOT rewritten:
            // it was produced under snapshot 1 and still says so.
            assert_eq!(evidence[0].snapshot, 1);
            assert_eq!(causes.len(), 1);
            assert_eq!(causes[0].detail, "request param");
        }
        other => panic!("must stay verifiable: {other:?}"),
    }
}

#[test]
fn the_index_survives_a_round_trip_through_the_store() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let store = DetectorReuseStore::new(dir.path());
    let mut index = DetectorReuseIndex::empty();
    index.remember(record(verifiable(), &[100], false));
    store.save(&index).expect("save");

    let loaded = store.load().expect("load");
    assert_eq!(loaded, index);
    let rec = loaded.latest("security.sql_injection").expect("one record");
    assert!(reuse_decision(rec, &identity("v1"), None).is_reusable());
}

// ---- AC #2 ----

#[test]
fn each_layer_invalidates_only_itself() {
    let base = record(verifiable(), &[100], false);

    let mut rule_only = identity("v2");
    rule_only.parser = base.identity.parser.clone();
    rule_only.sanitizer = base.identity.sanitizer.clone();
    rule_only.effective_capabilities = base.identity.effective_capabilities.clone();
    assert_eq!(
        reuse_decision(&base, &rule_only, None),
        ReuseDecision::Invalidated {
            levels: vec![InvalidationLevel::Rule]
        },
        "a rule edit must not re-run the parser or trust new facts"
    );

    let mut capability_only = identity("v1");
    capability_only.effective_capabilities = BTreeSet::from([AnalysisCapability::AstPattern]);
    assert_eq!(
        reuse_decision(&base, &capability_only, None),
        ReuseDecision::Invalidated {
            levels: vec![InvalidationLevel::Capability]
        }
    );

    let mut parser_only = identity("v1");
    parser_only.parser = DetectorDigest::from_content("parser v2");
    assert_eq!(
        reuse_decision(&base, &parser_only, None),
        ReuseDecision::Invalidated {
            levels: vec![InvalidationLevel::Parser]
        }
    );

    let mut sanitizer_only = identity("v1");
    sanitizer_only.sanitizer = DetectorDigest::from_content("sanitizer v2");
    assert_eq!(
        reuse_decision(&base, &sanitizer_only, None),
        ReuseDecision::Invalidated {
            levels: vec![InvalidationLevel::Sanitizer]
        }
    );

    // Two layers at once report both, deterministically ordered.
    let mut both = identity("v2");
    both.parser = DetectorDigest::from_content("parser v2");
    let ReuseDecision::Invalidated { levels } = reuse_decision(&base, &both, None) else {
        panic!("rule+parser must invalidate");
    };
    assert_eq!(
        levels,
        vec![InvalidationLevel::Rule, InvalidationLevel::Parser]
    );
}

// ---- AC #3: semantic layer ----

#[test]
fn only_read_facts_that_changed_invalidate_the_semantic_layer() {
    let base = record(verifiable(), &[100], false);

    // A change touching a fact the run read.
    let read_changed = delta(
        vec![
            fact(100, 10, "old", SnapshotId(1)),
            fact(101, 20, "kept", SnapshotId(1)),
        ],
        vec![
            fact(200, 10, "new", SnapshotId(2)),
            fact(101, 20, "kept", SnapshotId(2)),
        ],
    );
    let decision = reuse_decision(&base, &identity("v1"), Some(&read_changed));
    assert_eq!(
        decision,
        ReuseDecision::Invalidated {
            levels: vec![InvalidationLevel::SemanticDependency]
        }
    );

    // A change the run never read.
    let unrelated = delta(
        vec![
            fact(100, 10, "kept", SnapshotId(1)),
            fact(101, 20, "gone", SnapshotId(1)),
        ],
        vec![fact(200, 10, "kept", SnapshotId(2))],
    );
    // 101 removed and never read; (10, defines) only exists on both
    // sides — a renumbering, not a change.
    assert!(reuse_decision(&base, &identity("v1"), Some(&unrelated)).is_reusable());
}

#[test]
fn aggregate_reads_are_never_proven_unaffected_by_new_facts() {
    let base = record(verifiable(), &[100], false);
    // A purely new (subject, predicate): the run could have read it
    // through an aggregate query.
    let grown = delta(
        vec![fact(100, 10, "kept", SnapshotId(1))],
        vec![
            fact(200, 10, "kept", SnapshotId(2)),
            fact(201, 40, "brand new", SnapshotId(2)),
        ],
    );
    assert_eq!(
        reuse_decision(&base, &identity("v1"), Some(&grown)),
        ReuseDecision::Invalidated {
            levels: vec![InvalidationLevel::SemanticDependency]
        }
    );
}

#[test]
fn truncated_lineage_always_invalidates_semantics() {
    let base = record(verifiable(), &[100], true);
    let unchanged = delta(
        vec![fact(100, 10, "kept", SnapshotId(1))],
        vec![fact(200, 10, "kept", SnapshotId(2))],
    );
    assert_eq!(
        reuse_decision(&base, &identity("v1"), Some(&unchanged)),
        ReuseDecision::Invalidated {
            levels: vec![InvalidationLevel::SemanticDependency]
        }
    );
    assert_eq!(
        reuse_decision(&base, &identity("v1"), None),
        ReuseDecision::Reusable,
        "without a delta there is nothing to have gone stale"
    );
}

// ---- AC #3: location vs fingerprint ----

#[test]
fn line_moves_and_file_renames_keep_the_fingerprint_and_reuse() {
    let detector_semantic = digests("v1").semantic.clone();
    let before = semantic_fingerprint(&detector_semantic, "app::handler::query", "kind");
    // Same code, shifted lines, file moved by a rename: locations
    // differ, semantic identity does not.
    let after = semantic_fingerprint(&detector_semantic, "app::handler::query", "kind");
    assert_eq!(before, after);

    // Re-grounding onto a different subject is a different finding.
    let renamed_symbol = semantic_fingerprint(&detector_semantic, "app::handler::lookup", "kind");
    assert_ne!(before, renamed_symbol);

    // And locations genuinely do not feed the decision: two records
    // identical but for their output coordinates decide the same.
    let mut moved = record(verifiable(), &[100], false);
    moved.findings[0].output_locations = vec![OutputLocation {
        file: "src/lib/api.rs".to_string(),
        line: 7,
        column: 12,
    }];
    assert_eq!(
        reuse_decision(&moved, &identity("v1"), None),
        reuse_decision(&record(verifiable(), &[100], false), &identity("v1"), None)
    );
}

// ---- AC #4 ----

#[test]
fn no_state_is_promoted_by_caching_or_rebinding() {
    let observations = vec![
        verifiable(),
        GuardObservation::Advisory {
            snapshot_id: SnapshotId(1),
            detector: execution_ref(),
            evidence: vec![evidence_handle(8, 1)],
            message: "heads up".to_string(),
        },
        GuardObservation::Inconclusive {
            snapshot_id: SnapshotId(1),
            detector: execution_ref(),
            gaps: vec![Gap::NoEvidenceHandles],
        },
        GuardObservation::Unsupported {
            reason: UnknownReason::SnapshotMismatch {
                observed: SnapshotId(1),
                requested: SnapshotId(2),
            },
        },
        GuardObservation::Ungrounded {
            gaps: vec![Gap::UngroundedCausalStep],
        },
        GuardObservation::Failed {
            message: "structural".to_string(),
        },
    ];
    for observation in observations {
        let base = record(observation.clone(), &[100], false);
        let rebound = rebind_candidate(&base, SnapshotId(2));
        match (&observation, &rebound[0].observation) {
            (GuardObservation::Verifiable { .. }, CachedObservation::Verifiable { .. })
            | (GuardObservation::Advisory { .. }, CachedObservation::Advisory { .. })
            | (GuardObservation::Inconclusive { .. }, CachedObservation::Inconclusive { .. })
            | (GuardObservation::Unsupported { .. }, CachedObservation::Unsupported { .. })
            | (GuardObservation::Ungrounded { .. }, CachedObservation::Ungrounded { .. })
            | (GuardObservation::Failed { .. }, CachedObservation::Failed { .. }) => {}
            (original, rebounded) => {
                panic!("state changed shape from {original:?} to {rebounded:?}")
            }
        }
        // The three snapshot-less states stay snapshot-less: caching
        // them could not have invented a snapshot to verify against.
        assert!(
            !matches!(rebound[0].observation, CachedObservation::Verifiable { .. })
                || matches!(observation, GuardObservation::Verifiable { .. })
        );
    }
}

fn record_over(snapshot: u64) -> DetectorRunRecord {
    DetectorRunRecord {
        produced_snapshot: SnapshotId(snapshot),
        ..record(verifiable(), &[100], false)
    }
}

#[test]
fn a_detector_keeps_one_record_per_base() {
    let mut index = DetectorReuseIndex::empty();
    index.remember(record_over(1));
    index.remember(record_over(2));
    index.remember(record_over(1));

    assert_eq!(index.records.len(), 2, "re-remembering a base replaces it");
    let id = "security.sql_injection";
    assert!(index.record_for(id, SnapshotId(1)).is_some());
    assert!(index.record_for(id, SnapshotId(2)).is_some());
    assert!(index.record_for(id, SnapshotId(3)).is_none());
    assert_eq!(
        index.latest(id).map(|r| r.produced_snapshot),
        Some(SnapshotId(1)),
        "the base remembered last is the latest"
    );
}

#[test]
fn retention_is_bounded_per_detector_and_evicts_the_least_recent() {
    use cognicode_core::domain::findings::detector_reuse::MAX_RECORDS_PER_DETECTOR;
    let bound = MAX_RECORDS_PER_DETECTOR as u64;
    let mut index = DetectorReuseIndex::empty();
    let mut other = record_over(1);
    other.identity = identity("v1");
    other.identity.detector_id = DetectorId::new("other.detector").expect("id");
    index.remember(other);
    for snapshot in 1..=bound {
        index.remember(record_over(snapshot));
    }
    // Using the oldest base again makes it the most recent.
    index.remember(record_over(1));
    index.remember(record_over(bound + 1));

    let id = "security.sql_injection";
    let kept = index
        .records
        .iter()
        .filter(|r| r.identity.detector_id.as_str() == id)
        .count();
    assert_eq!(kept, MAX_RECORDS_PER_DETECTOR);
    assert!(index.record_for(id, SnapshotId(1)).is_some(), "refreshed");
    assert!(index.record_for(id, SnapshotId(2)).is_none(), "evicted");
    assert!(index.record_for(id, SnapshotId(bound + 1)).is_some());
    assert!(
        index.record_for("other.detector", SnapshotId(1)).is_some(),
        "another detector's records are never evicted by this one's"
    );
}

#[test]
fn an_index_in_the_previous_schema_loads_as_nothing() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let store = DetectorReuseStore::new(dir.path());
    std::fs::write(
        store_dir_file(dir.path()),
        br#"{"schema":"cognicode.detector-reuse/v1","records":{}}"#,
    )
    .expect("write");
    assert_eq!(store.load(), None, "costs a re-run, never a wrong reuse");
}

#[test]
fn a_foreign_schema_never_decodes_into_the_index() {
    let mut index = DetectorReuseIndex::empty();
    index.remember(record(verifiable(), &[100], false));
    let json = index.to_json().expect("serialize");
    let foreign = json.replace(
        "cognicode.detector-reuse/v2",
        "cognicode.detector-reuse/v99",
    );
    assert!(DetectorReuseIndex::from_json(&foreign).is_err());

    let dir = tempfile::TempDir::new().expect("temp dir");
    let store = DetectorReuseStore::new(dir.path());
    store.save(&index).expect("save");
    std::fs::write(store_dir_file(dir.path()), b"{ not json").expect("corrupt");
    assert_eq!(
        store.load(),
        None,
        "a corrupt cache costs work, never correctness"
    );
}

fn store_dir_file(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("detector-reuse.json")
}
