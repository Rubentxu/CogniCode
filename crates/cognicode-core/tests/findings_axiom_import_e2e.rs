//! Imported-detector end-to-end (M6, cycle e61, umbrella 7.7, U-A8).
//!
//! An imported (legacy-migrated) detector must traverse the **same** path as a
//! builtin one — `admission → planner → backend → outcome → evidence →
//! assembler → verifier → gate`. Axiom gets no special execution route.
//!
//! The import only produces a `DetectorIr`; authority still comes exclusively
//! from `DetectorAdmission` (+ an optional verified promotion).

use cognicode_core::application::findings::axiom_migration::{
    AxiomDetectorTranslator, AxiomImportResult, ImportedDetectorDefinition, LegacyDetection,
    LegacySeverity, NormalizedLegacyRule,
};
use cognicode_core::domain::execution::{ActorRef, CorrelationId};
use cognicode_core::domain::findings::{
    AdmissionSource, AnalysisInput, AnalysisScope, AstBackend, AstConstruct, AstInput, AstUnit,
    BackendRegistry, CausalStepKind, DetectorAdmission, DetectorAuthority, DetectorExecutor,
    EvidenceClass, ExecutionRequest, FindingGate, FindingVerifier, GroundingRef,
    PromotionAuthority, PromotionRequest, RiskLevel, SubjectPattern,
};
use cognicode_core::domain::kernel_ids::{ExecutionId, FactId};
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;

fn legacy_weak_hash_rule(severity: LegacySeverity) -> NormalizedLegacyRule {
    NormalizedLegacyRule {
        system: "axiom".to_string(),
        rule_id: "S4790".to_string(),
        name: "weak hash algorithm".to_string(),
        language: "rust".to_string(),
        category: "security".to_string(),
        severity,
        source_revision: "axiom-corpus-2026".to_string(),
        detection: LegacyDetection::AstPattern {
            subject: "security.md5_usage".to_string(),
        },
    }
}

fn translate(severity: LegacySeverity) -> ImportedDetectorDefinition {
    match AxiomDetectorTranslator::translate(&legacy_weak_hash_rule(severity)) {
        AxiomImportResult::Translated(def) => def,
        AxiomImportResult::Skipped(d) => panic!("expected translation, got {d:?}"),
    }
}

fn ast_input() -> AnalysisInput {
    AnalysisInput {
        scope: Some(scope()),
        ast: Some(AstInput {
            units: vec![AstUnit {
                path: "src/hash.rs".to_string(),
                constructs: vec![AstConstruct {
                    subject: SubjectPattern::new("security.md5_usage").unwrap(),
                    line: 12,
                    detail: "md5::Md5::new()".to_string(),
                    grounding: Some(GroundingRef::fact(FactId::new(7))),
                }],
            }],
        }),
        graph: None,
        dataflow: None,
    }
}

fn registry() -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    registry.register(Box::new(AstBackend));
    registry
}

#[test]
fn u_a8_imported_detector_uses_the_same_chain_as_a_builtin() {
    // A legacy BLOCKER: severity maps to policy, never to authority.
    let imported = translate(LegacySeverity::Blocker);
    assert_eq!(imported.legacy.legacy_severity, "BLOCKER");
    assert_eq!(imported.ir.policy.default_risk, RiskLevel::Critical);

    // The CALLER admits it — the importer has no API to grant authority.
    let permit =
        DetectorAdmission::admit(imported.ir.clone(), "1.0.0", AdmissionSource::Imported).unwrap();
    assert_eq!(permit.authority(), DetectorAuthority::Candidate);

    // Same seam as any other detector.
    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::with_scope(scope());
    let record = executor
        .execute(&permit, &ast_input(), &mut store, test_request(3))
        .expect("imported detector must execute");

    assert_eq!(record.backend, "ast");
    assert_eq!(record.findings.len(), 1);
    let finding = &record.findings[0];
    assert_eq!(finding.kind.as_str(), "security.s4790");
    // Policy from the legacy severity, not authority.
    assert_eq!(finding.risk, RiskLevel::Critical);
    assert_eq!(
        finding.detector.authority_at_execution,
        DetectorAuthority::Candidate
    );
    assert_eq!(finding.causal_chain[0].kind, CausalStepKind::Source);

    let verifier = FindingVerifier::new(&store);
    assert!(verifier.verify_for_gate(finding).is_ok());
    let gate = FindingGate::new(EvidenceClass::C, RiskLevel::Low);
    assert!(
        !verifier.can_block(finding, &gate),
        "an imported BLOCKER rule must never block without a verified promotion"
    );
}

/// A governance verifier that accepts the approval (test double for M9/M13).
struct GovernanceVerifier;

impl cognicode_core::domain::findings::ApprovalVerifier for GovernanceVerifier {
    fn verify(&self, request: &PromotionRequest) -> bool {
        !request.approver().trim().is_empty()
    }
}

#[test]
fn u_a8_imported_provenance_is_not_eligible_for_the_default_verifier() {
    let imported = translate(LegacySeverity::Blocker);
    let candidate =
        DetectorAdmission::admit(imported.ir, "1.0.0", AdmissionSource::Imported).unwrap();

    // The shipped EligibleSourceVerifier only trusts Builtin/HumanCurated
    // provenance, so an imported detector cannot be promoted with it.
    let request = PromotionRequest::for_permit(&candidate, "security-governance").unwrap();
    assert_eq!(request.target().source, AdmissionSource::Imported);
    assert!(matches!(
        PromotionAuthority::verify(
            &cognicode_core::domain::findings::EligibleSourceVerifier,
            request
        )
        .unwrap_err(),
        cognicode_core::domain::findings::AdmissionError::PromotionRejected { .. }
    ));
}

#[test]
fn u_a8_imported_detector_gates_only_through_a_governance_verifier() {
    let imported = translate(LegacySeverity::Blocker);
    let candidate =
        DetectorAdmission::admit(imported.ir, "1.0.0", AdmissionSource::Imported).unwrap();

    // An explicit governance verifier (M9/M13) can promote it; the standard
    // promotion path is used — nothing bespoke for imports.
    let request = PromotionRequest::for_permit(&candidate, "security-governance").unwrap();
    let verified = PromotionAuthority::verify(&GovernanceVerifier, request).unwrap();
    let gated = DetectorAdmission::promote(&candidate, verified).unwrap();
    assert!(gated.can_block());

    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::with_scope(scope());
    let record = executor
        .execute(&gated, &ast_input(), &mut store, test_request(4))
        .unwrap();
    let verifier = FindingVerifier::new(&store);
    assert!(verifier.can_block(
        &record.findings[0],
        &FindingGate::new(EvidenceClass::C, RiskLevel::Low)
    ));
}

/// The execution request every test run uses (M7.2): a deterministic actor,
/// a fixed correlation, and the scope the fixture views were projected from.
fn test_request(id: u64) -> ExecutionRequest {
    ExecutionRequest::new(
        ExecutionId::new(id),
        scope(),
        ActorRef::detector("test.detector"),
        CorrelationId::new("test-correlation").unwrap(),
    )
}

fn scope() -> AnalysisScope {
    AnalysisScope::new(
        cognicode_core::domain::value_objects::WorkspaceId::try_new("workspace").unwrap(),
        cognicode_core::domain::kernel_ids::SnapshotId::new(1),
    )
}
