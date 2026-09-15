//! End-to-end vertical slice for M6 (cycles e57 / e58.1 / e58.2).
//!
//! Proves the full safe path for the Detector IR:
//!
//! ```text
//! IR → admission (ExecutionPermit) → plan → AST backend → evidence store
//!    → assembler → finding → referential verification → gate
//! ```
//!
//! and the safety properties:
//! - a `Candidate` run never blocks (U43), even when its evidence verifies;
//! - promotion needs a `VerifiedPromotion` minted by an `ApprovalVerifier`;
//! - a persisted record cannot be turned into a `Gated` permit by default;
//! - the AST backend cannot invent a kind or claim graph/runtime evidence.

use std::collections::BTreeSet;

use cognicode_core::domain::findings::{
    AdmissionSource, AdmittedDetectorRecord, AnalysisCapability, AnalysisInput, AnalysisScope,
    AstBackend, AstConstruct, AstInput, AstUnit, BackendRegistry, DetectorAdmission,
    DetectorAuthority, DetectorExecutor, DetectorFindingPolicy, DetectorId, DetectorIr,
    DetectorStep, EvidenceClass, ExecutionError, ExecutionPermit, ExecutionRecord, Finding,
    FindingGate, FindingKind, FindingVerifier, PromotionAuthority, PromotionRequest,
    RejectAllApprovals, RiskLevel, SubjectPattern, VerificationError,
};
use cognicode_core::domain::kernel_ids::ExecutionId;
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;

fn weak_hash_ir(claimed_authority: DetectorAuthority) -> DetectorIr {
    DetectorIr {
        id: DetectorId::new("security.weak_hash").unwrap(),
        name: "weak hash".to_string(),
        policy: DetectorFindingPolicy::default(),
        requires: [AnalysisCapability::AstPattern].into_iter().collect(),
        // The raw definition *claims* an authority. Admission must discard it.
        authority: claimed_authority,
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

fn ast_input() -> AnalysisInput {
    AnalysisInput {
        scope: Some(scope()),
        dataflow: None,
        graph: None,
        ast: Some(AstInput {
            units: vec![AstUnit {
                path: "src/hash.rs".to_string(),
                constructs: vec![AstConstruct {
                    subject: SubjectPattern::new("security.md5_usage").unwrap(),
                    line: 12,
                    detail: "md5::Md5::new()".to_string(),
                    grounding: None,
                }],
            }],
        }),
    }
}

fn registry() -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    registry.register(Box::new(AstBackend));
    registry
}

fn blocker_gate() -> FindingGate {
    FindingGate::new(EvidenceClass::C, RiskLevel::Low)
}

/// A verifier that would accept a HumanCurated promotion (test double).
struct AcceptAll;
impl cognicode_core::domain::findings::ApprovalVerifier for AcceptAll {
    fn verify(&self, _request: &PromotionRequest) -> bool {
        true
    }
}

fn verified_promotion(
    permit: &ExecutionPermit,
) -> cognicode_core::domain::findings::VerifiedPromotion {
    let request = PromotionRequest::for_permit(permit, "security-team").unwrap();
    PromotionAuthority::verify(&AcceptAll, request).unwrap()
}

fn run(permit: &ExecutionPermit) -> (InMemoryEvidenceStore, ExecutionRecord) {
    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::with_scope(scope());
    let record = executor
        .execute(permit, &ast_input(), &mut store, ExecutionId::new(1))
        .expect("execution must succeed");
    assert_eq!(record.backend, "ast");
    (store, record)
}

fn findings(record: &ExecutionRecord) -> &[Finding] {
    &record.findings
}

#[test]
fn u40_candidate_run_produces_a_finding_but_cannot_block() {
    let permit = DetectorAdmission::admit(
        weak_hash_ir(DetectorAuthority::Gated),
        "1.0.0",
        AdmissionSource::AiGenerated,
    )
    .unwrap();
    assert_eq!(permit.authority(), DetectorAuthority::Candidate);

    let (store, record) = run(&permit);
    let findings = findings(&record);
    assert_eq!(findings.len(), 1, "the AST backend found the MD5 usage");

    let finding = &findings[0];
    assert_eq!(finding.kind.as_str(), "security.weak_hash");
    assert_eq!(finding.evidence_class, EvidenceClass::C);
    assert_eq!(
        finding.detector.authority_at_execution,
        DetectorAuthority::Candidate
    );
    // The run's scope is captured on the execution reference.
    assert_eq!(
        finding.detector.scope.as_ref().map(|s| s.snapshot),
        Some(cognicode_core::domain::kernel_ids::SnapshotId::new(1)),
        "the execution must carry the analysis scope"
    );

    let verifier = FindingVerifier::new(&store);
    assert!(verifier.verify_for_gate(finding).is_ok());
    assert!(
        !verifier.can_block(finding, &blocker_gate()),
        "U43: a Candidate run must never block, even when referentially valid"
    );
}

#[test]
fn promoted_detector_blocks_and_keeps_semantic_identity() {
    let candidate = DetectorAdmission::admit(
        weak_hash_ir(DetectorAuthority::Candidate),
        "1.0.0",
        AdmissionSource::HumanCurated,
    )
    .unwrap();
    let gated = DetectorAdmission::promote(&candidate, verified_promotion(&candidate)).unwrap();
    assert!(gated.can_block());

    let (candidate_store, candidate_record) = run(&candidate);
    let (gated_store, gated_record) = run(&gated);

    let candidate_verifier = FindingVerifier::new(&candidate_store);
    let gated_verifier = FindingVerifier::new(&gated_store);
    assert!(!candidate_verifier.can_block(&candidate_record.findings[0], &blocker_gate()));
    assert!(gated_verifier.can_block(&gated_record.findings[0], &blocker_gate()));

    // Same algorithm, different authority.
    assert_eq!(
        candidate_record.findings[0].detector.digests.semantic,
        gated_record.findings[0].detector.digests.semantic,
        "promotion does not change the semantic identity"
    );
    assert_ne!(
        candidate_record.findings[0].detector.digests.instance,
        gated_record.findings[0].detector.digests.instance,
        "promotion changes the instance digest"
    );
    assert_eq!(
        candidate_record.findings[0].detector.digests.logic,
        gated_record.findings[0].detector.digests.logic
    );
}

#[test]
fn unresolved_evidence_blocks_even_a_gated_finding() {
    let candidate = DetectorAdmission::admit(
        weak_hash_ir(DetectorAuthority::Candidate),
        "1.0.0",
        AdmissionSource::HumanCurated,
    )
    .unwrap();
    let gated = DetectorAdmission::promote(&candidate, verified_promotion(&candidate)).unwrap();

    let (_store, record) = run(&gated);
    let empty_store = InMemoryEvidenceStore::with_scope(scope());
    let verifier = FindingVerifier::new(&empty_store);
    assert!(matches!(
        verifier.verify_for_gate(&record.findings[0]),
        Err(VerificationError::UnresolvedEvidence(_))
    ));
    assert!(!verifier.can_block(&record.findings[0], &blocker_gate()));
}

#[test]
fn a_persisted_record_cannot_restore_gate_authority_by_default() {
    // Persist a Candidate, hand-edit the record to claim Gated with a forged
    // approval string.
    let permit = DetectorAdmission::admit(
        weak_hash_ir(DetectorAuthority::Candidate),
        "1.0.0",
        AdmissionSource::HumanCurated,
    )
    .unwrap();
    let mut record = permit.record();
    record.authority = DetectorAuthority::Gated;
    record.admission.approval = Some("forged".to_string());

    let json = serde_json::to_string(&record).unwrap();
    let parsed: AdmittedDetectorRecord = serde_json::from_str(&json).unwrap();

    // Default restore is fail-closed: Candidate.
    assert_eq!(
        DetectorAdmission::restore(&parsed).unwrap().authority(),
        DetectorAuthority::Candidate
    );
    // Even the verifier path with the shipped reject-all default: Candidate.
    assert_eq!(
        DetectorAdmission::restore_with(&parsed, &RejectAllApprovals)
            .unwrap()
            .authority(),
        DetectorAuthority::Candidate
    );
}

#[test]
fn u47_planning_fails_loud_when_no_backend_covers_capabilities() {
    let mut ir = weak_hash_ir(DetectorAuthority::Candidate);
    ir.requires = [AnalysisCapability::Dataflow].into_iter().collect();
    let permit = DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::Builtin).unwrap();

    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::with_scope(scope());
    let err = executor
        .execute(&permit, &ast_input(), &mut store, ExecutionId::new(1))
        .expect_err("no backend provides Dataflow");
    match err {
        ExecutionError::Plan(plan) => {
            assert!(plan.missing.contains(&AnalysisCapability::Dataflow));
        }
        other => panic!("expected a planning error, got {other:?}"),
    }
}

#[test]
fn registry_and_capability_sets_are_stable() {
    let registry = registry();
    let requires: BTreeSet<AnalysisCapability> =
        [AnalysisCapability::AstPattern].into_iter().collect();
    assert_eq!(registry.plan(&requires).unwrap().name(), "ast");
}

fn scope() -> AnalysisScope {
    AnalysisScope::new(
        cognicode_core::domain::value_objects::WorkspaceId::try_new("workspace").unwrap(),
        cognicode_core::domain::kernel_ids::SnapshotId::new(1),
    )
}
