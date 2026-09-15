//! End-to-end vertical slice for M6 (cycles e57 / e58.1).
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
//! - the same detector promoted to `Gated` does block, with the same semantic
//!   digest but a different instance digest (U57 promotion identity);
//! - a hand-written JSON record cannot mint gate authority;
//! - the AST backend cannot claim graph/runtime evidence (ceiling contract).

use std::collections::BTreeSet;

use cognicode_core::domain::findings::{
    AdmissionSource, AdmittedDetectorRecord, AnalysisCapability, AnalysisInput, AstBackend,
    AstConstruct, AstInput, AstUnit, BackendRegistry, DetectorAdmission, DetectorAuthority,
    DetectorExecutor, DetectorFindingPolicy, DetectorId, DetectorIr, DetectorStep, EvidenceClass,
    ExecutionError, ExecutionPermit, ExecutionRecord, Finding, FindingGate, FindingKind,
    FindingVerifier, PromotionApproval, RiskLevel, SubjectPattern, VerificationError,
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
        ast: Some(AstInput {
            units: vec![AstUnit {
                path: "src/hash.rs".to_string(),
                constructs: vec![AstConstruct {
                    subject: SubjectPattern::new("security.md5_usage").unwrap(),
                    line: 12,
                    detail: "md5::Md5::new()".to_string(),
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

fn run(permit: &ExecutionPermit) -> (InMemoryEvidenceStore, ExecutionRecord) {
    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::new();
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
    // A raw definition claiming `Gated` from an AI source is forced to
    // Candidate by admission.
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

    let verifier = FindingVerifier::new(&store);
    assert!(
        verifier.verify_for_gate(finding).is_ok(),
        "evidence resolves and the finding is explainable"
    );
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
    let gated =
        DetectorAdmission::promote(&candidate, PromotionApproval::new("security-team").unwrap())
            .unwrap();
    assert!(gated.can_block());

    let (candidate_store, candidate_record) = run(&candidate);
    let (gated_store, gated_record) = run(&gated);

    let candidate_verifier = FindingVerifier::new(&candidate_store);
    let gated_verifier = FindingVerifier::new(&gated_store);
    assert!(!candidate_verifier.can_block(&candidate_record.findings[0], &blocker_gate()));
    assert!(gated_verifier.can_block(&gated_record.findings[0], &blocker_gate()));

    // Same algorithm, different authority.
    assert_eq!(
        candidate_record.findings[0].detector.semantic_digest,
        gated_record.findings[0].detector.semantic_digest,
        "promotion does not change the semantic identity"
    );
    assert_ne!(
        candidate_record.findings[0].detector.instance_digest,
        gated_record.findings[0].detector.instance_digest,
        "promotion changes the instance digest"
    );
}

#[test]
fn unresolved_evidence_blocks_even_a_gated_finding() {
    let gated = DetectorAdmission::promote(
        &DetectorAdmission::admit(
            weak_hash_ir(DetectorAuthority::Candidate),
            "1.0.0",
            AdmissionSource::HumanCurated,
        )
        .unwrap(),
        PromotionApproval::new("team").unwrap(),
    )
    .unwrap();

    let (_store, record) = run(&gated);
    // Verify against an EMPTY store: the evidence does not resolve.
    let empty_store = InMemoryEvidenceStore::new();
    let verifier = FindingVerifier::new(&empty_store);
    assert!(matches!(
        verifier.verify_for_gate(&record.findings[0]),
        Err(VerificationError::UnresolvedEvidence(_))
    ));
    assert!(!verifier.can_block(&record.findings[0], &blocker_gate()));
}

#[test]
fn u47_planning_fails_loud_when_no_backend_covers_capabilities() {
    let mut ir = weak_hash_ir(DetectorAuthority::Candidate);
    ir.requires = [AnalysisCapability::Dataflow].into_iter().collect();
    // The AST backend does not provide Dataflow.
    let permit = DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::Builtin).unwrap();

    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::new();
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
fn a_json_record_cannot_mint_gate_authority() {
    // Persist a Candidate, then hand-edit the record to claim Gated with no
    // approval: restore must downgrade it.
    let permit = DetectorAdmission::admit(
        weak_hash_ir(DetectorAuthority::Candidate),
        "1.0.0",
        AdmissionSource::AiGenerated,
    )
    .unwrap();
    let mut record = permit.record();
    record.authority = DetectorAuthority::Gated;

    let json = serde_json::to_string(&record).unwrap();
    let parsed: AdmittedDetectorRecord = serde_json::from_str(&json).unwrap();
    let restored = DetectorAdmission::restore(&parsed).unwrap();
    assert_eq!(restored.authority(), DetectorAuthority::Candidate);
    assert!(!restored.can_block());
}

#[test]
fn registry_and_capability_sets_are_stable() {
    let registry = registry();
    let requires: BTreeSet<AnalysisCapability> = BTreeSet::new();
    // An empty requirement set is satisfied by any backend (PRODUCE-only
    // aggregators are the only legal detectors with no requirement).
    assert_eq!(registry.plan(&requires).unwrap().name(), "ast");
}
