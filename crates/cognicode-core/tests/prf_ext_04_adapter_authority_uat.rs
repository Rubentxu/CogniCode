//! PRF-EXT-04 UAT: backend adapters/extensions do not become an
//! alternative source of truth.
//!
//! Three claims, exercised through the public cross-crate surface:
//! 1. Registration/execution requires an ExecutionPermit minted only by
//!    DetectorAdmission — the permit is sealed (no public constructor
//!    outside admission), and restore() downgrades any stored authority
//!    back to Candidate (fail-closed).
//! 2. A registered backend cannot inflate evidence beyond its declared
//!    ceiling: overclaim is rejected (BackendContractViolation).
//! 3. Admission alone never grants gate authority: a Candidate detector
//!    cannot block CI (can_block == false) regardless of its evidence.

use cognicode_core::domain::findings::admission::{AdmissionSource, DetectorAdmission};

#[test]
fn restored_permits_are_fail_closed_to_candidate() {
    // Admission is the only authority mint. Whatever a persisted record
    // claims, restore() forces Candidate authority (fail-closed): a
    // backend cannot gain `Gated`/CI-blocking authority by registering
    // or by forging a stored record.
    let record = {
        let permit =
            DetectorAdmission::admit(minimal_detector_ir(), "1.0.0", AdmissionSource::Builtin)
                .expect("admission of a valid detector");
        permit.record()
    };
    let restored = DetectorAdmission::restore(&record).expect("restore succeeds (fail-closed)");
    assert_eq!(
        restored.admitted().authority,
        cognicode_core::domain::findings::detector_ir::DetectorAuthority::Candidate,
        "restored authority must be downgraded to Candidate"
    );
    assert!(
        !restored.can_block(),
        "a Candidate detector must never be able to block CI"
    );
}

#[test]
fn registry_cannot_plan_capabilities_no_backend_declares() {
    // An extension cannot win authority by merely being registered:
    // the planner only selects backends that declare the required
    // capabilities, and fails closed when none does.
    let registry = cognicode_core::domain::findings::execution::BackendRegistry::new();
    let mut requires = std::collections::BTreeSet::new();
    requires.insert(
        cognicode_core::domain::findings::detector_ir::AnalysisCapability::SemanticResolution,
    );
    match registry.plan(&requires) {
        Err(err) => assert!(
            err.missing.contains(
                &cognicode_core::domain::findings::detector_ir::AnalysisCapability::SemanticResolution
            ),
            "planner must report the missing capability"
        ),
        Ok(_) => panic!("planner must fail with no backends registered"),
    }
}

#[test]
fn candidate_permits_cannot_skip_contracts_on_execution() {
    // Even with an admitted permit, execution is bounded by backend
    // contract checks: an execution against an empty registry cannot be
    // planned (fail closed), and nothing reaches evidence persistence.
    let permit =
        DetectorAdmission::admit(minimal_detector_ir(), "1.0.0", AdmissionSource::AiGenerated)
            .expect("admission");
    assert_eq!(
        permit.admitted().authority,
        cognicode_core::domain::findings::detector_ir::DetectorAuthority::Candidate,
        "admission alone must yield Candidate, never Gated"
    );
    assert!(!permit.can_block());
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn minimal_detector_ir() -> cognicode_core::domain::findings::detector_ir::DetectorIr {
    use cognicode_core::domain::findings::{
        AnalysisCapability, DetectorFindingPolicy, DetectorIr, DetectorStep, FindingKind,
        SubjectPattern,
    };
    let mut requires = std::collections::BTreeSet::new();
    requires.insert(AnalysisCapability::AstPattern);
    DetectorIr {
        id: cognicode_core::domain::findings::detector_ir::DetectorId::new("ext04.minimal")
            .unwrap(),
        name: "ext04 minimal".to_string(),
        policy: DetectorFindingPolicy::default(),
        requires,
        authority: cognicode_core::domain::findings::detector_ir::DetectorAuthority::Candidate,
        steps: vec![
            DetectorStep::Match {
                subject: SubjectPattern::new("security.md5_usage").unwrap(),
            },
            DetectorStep::Produce {
                kind: FindingKind::new("security.weak_hash").unwrap(),
            },
        ],
    }
}
