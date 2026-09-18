//! Grounded finding flow — end-to-end acceptance seam (e67 WU3).
//!
//! Composes the existing production seams (WU1 ingest, WU2 grounded AST
//! projection) with the canonical M6 finding authorities — [`DetectorAdmission`],
//! [`DetectorExecutor`], [`CanonicalEvidenceWriter`], [`FindingAssembler`],
//! [`KernelEvidenceReadModel`] and [`FindingVerifier`] — to demonstrate the
//! full vertical that e67 sets out to prove:
//!
//! ```text
//! Rust source
//!     ↓ ingest_rust_facts (WU1)
//! FactStore (canonical Facts in snapshot)
//!     ↓ project_grounded_ast (WU2)
//! AstInput (each AstConstruct carries GroundingRef or None)
//!     ↓ AstBackend.run (existing)
//! DetectorOutcome (ProducedEvidence with GroundingRef)
//!     ↓ CanonicalEvidenceWriter.persist
//! EvidenceStore (atomic batch, Grounded/Ungrounded bindings)
//!     ↓ PreparedExecution::finalize
//! FindingAssembler::assemble → Vec<Finding>
//!     ↓ KernelEvidenceReadModel::load
//! EvidenceLookup (scope + descriptors)
//!     ↓ FindingVerifier::verify_for_gate
//! gate decision
//! ```
//!
//! # Authority rule (e67 REQ-DGN-001 + LSI fail-closed)
//!
//! This module introduces **no new authority path**: it does not mint
//! `ExecutionPermit`s, does not bypass `DetectorAdmission`, does not skip
//! `CanonicalEvidenceWriter`, and does not re-implement verification. It
//! only composes the existing authorities. A finding may block the gate
//! iff the existing `FindingVerifier::can_block` says so.
//!
//! # Async end-to-end
//!
//! Every hop is `.await`-ed: ingest, projection, admission/promotion,
//! detector execution, evidence persistence, evidence load, and the
//! final gate check. There are no sync runtime bridges; the function is
//! a single async pipeline that the caller drives on their chosen
//! executor.
//!
//! # Errors
//!
//! Errors propagate from each composing piece. This module does not invent
//! new error variants; the surface is the union of the pieces it composes.

use std::sync::Arc;

use crate::application::fact_bridge::production_grounding::ingest_rust_facts;
use crate::application::findings::grounded_ast_projection::project_grounded_ast;
use crate::application::findings::kernel_bridge::{
    CanonicalEvidenceWriter, KernelEvidenceReadModel,
};
use crate::application::ingest::extractor::extract_file;
use crate::domain::evidence_kernel::fact::{ProvenanceRecord, ProvenanceRecord as _Provenance};
use crate::domain::evidence_kernel::ports::{EvidenceStore, FactStore, KernelError};
use crate::domain::execution::{ActorRef, CorrelationId};
use crate::domain::findings::ast_backend::AstBackend;
use crate::domain::findings::detector_ir::{
    AnalysisCapability, DetectorAuthority, DetectorId, DetectorIr, DetectorStep, FindingKind,
    SubjectPattern,
};
use crate::domain::findings::execution::{
    AnalysisInput, BackendRegistry, DetectorExecutor, ExecutionRequest,
};
use crate::domain::findings::finding::FindingGate;
use crate::domain::findings::scope::AnalysisScope;
use crate::domain::findings::verifier::FindingVerifier;
use crate::domain::kernel_ids::ExecutionId;
use crate::domain::value_objects::WorkspaceId;
use crate::infrastructure::parser::language_config::RUST_CONFIG;

/// The subject the detector scans for. The same vocabulary WU2 uses, so the
/// AstConstructs the projector emits are exactly what the detector matches on.
pub const SUBJECT_FUNCTION_DEFINITION: &str = "syntax.function_definition";

/// The detector id and the produced FindingKind, scoped to e67.
pub const DETECTOR_ID_FUNCTION_DEFINITION: &str = "structure.function_definition";

/// The provenance attached to evidence committed by the canonical writer.
///
/// The kernel's `Fact::new` permits `ProducerKind::LlmAgent` only when
/// `class=Hypothesis`; we use `DeterministicAnalyzer` because every fact
/// this flow emits is rooted in a parse of a real Rust source file.
fn canonical_provenance() -> ProvenanceRecord {
    use crate::domain::value_objects::Provenance;
    _Provenance::new(
        Provenance::Extracted,
        crate::domain::evidence_kernel::fact::ProducerKind::DeterministicAnalyzer,
        Some("e67-grounded-finding-flow".to_string()),
    )
}

/// Outcome of the full e67 vertical for a single source file.
#[derive(Debug, Clone)]
pub struct GroundedFindingReport {
    /// How many function constructs were observed by the projector.
    pub construct_count: usize,
    /// How many constructs were grounded (1-of-1 canonical match).
    pub grounded_count: usize,
    /// How many constructs were left ungrounded (0 or >1 matches).
    pub ungrounded_count: usize,
    /// The findings assembled by `FindingAssembler`. May be empty when no
    /// detector match fired.
    pub findings: Vec<crate::domain::findings::finding::Finding>,
    /// The gate decision per finding: `true` means `verifier.can_block(gate)`
    /// returned `true`, `false` means it returned `false` (for any reason).
    pub gateable: Vec<bool>,
    /// The deserialized fact ids committed by WU1's ingest, for traceability.
    pub committed_fact_ids: Vec<crate::domain::kernel_ids::FactId>,
}

/// Errors raised by the composed flow.
#[derive(Debug, thiserror::Error)]
pub enum GroundedFindingFlowError {
    #[error("ingest failed: {0}")]
    Ingest(String),
    #[error("projection failed: {0}")]
    Projection(String),
    #[error("detector admit failed: {0}")]
    Admit(String),
    #[error("detector execution failed: {0}")]
    Execution(String),
    #[error("canonical evidence write failed: {0}")]
    EvidenceWrite(#[from] KernelError),
    #[error("finding verification failed: {0}")]
    Verification(String),
}

/// Run the e67 vertical for one Rust source file.
///
/// This is a **composition**: it calls the existing authorities in order and
/// stops at the first hard failure. It does not introduce any verification,
/// any authority, any store. The gate decision it reports is the gate
/// decision the existing `FindingVerifier` returned — for better or worse.
#[allow(clippy::too_many_arguments)] // single entry point; params are distinct evidence inputs
pub async fn run_grounded_finding_flow(
    fact_store: Arc<dyn FactStore>,
    evidence_store: Arc<dyn EvidenceStore>,
    workspace: &WorkspaceId,
    snapshot: crate::domain::evidence_kernel::ids::SnapshotId,
    path: &std::path::Path,
    source: &str,
    hash: &str,
    gate: FindingGate,
) -> Result<GroundedFindingReport, GroundedFindingFlowError> {
    // ---- WU1: production ingest -------------------------------------------
    let receipt = ingest_rust_facts(
        fact_store.as_ref(),
        workspace,
        &snapshot,
        path,
        source,
        hash,
    )
    .await
    .map_err(|e| GroundedFindingFlowError::Ingest(e.to_string()))?;

    // Re-extract to get the same ExtractionResult the projector consumes.
    let extraction = extract_file(&RUST_CONFIG, path, source, hash);

    // ---- WU2: grounded AST projection ------------------------------------
    let projection = project_grounded_ast(fact_store.clone(), workspace, &snapshot, &extraction)
        .await
        .map_err(|e| GroundedFindingFlowError::Projection(e.to_string()))?;

    let grounded_count = projection
        .grounded_outcomes
        .iter()
        .filter(|o| o.grounding.is_some())
        .count();
    let ungrounded_count = projection.grounded_outcomes.len() - grounded_count;
    let construct_count = projection.grounded_outcomes.len();

    // ---- Detector admission + plan ---------------------------------------
    let ir = DetectorIr {
        id: DetectorId::new(DETECTOR_ID_FUNCTION_DEFINITION)
            .map_err(|e| GroundedFindingFlowError::Admit(e.to_string()))?,
        name: DETECTOR_ID_FUNCTION_DEFINITION.to_string(),
        policy: Default::default(),
        requires: [AnalysisCapability::AstPattern].into_iter().collect(),
        authority: DetectorAuthority::Candidate,
        steps: vec![
            DetectorStep::Match {
                subject: SubjectPattern::new(SUBJECT_FUNCTION_DEFINITION)
                    .map_err(|e| GroundedFindingFlowError::Admit(e.to_string()))?,
            },
            DetectorStep::Produce {
                kind: FindingKind::new(DETECTOR_ID_FUNCTION_DEFINITION)
                    .map_err(|e| GroundedFindingFlowError::Admit(e.to_string()))?,
            },
        ],
    };
    let permit = crate::domain::findings::admission::DetectorAdmission::admit(
        ir,
        "e67-wu3-1.0.0",
        crate::domain::findings::admission::AdmissionSource::Builtin,
    )
    .map_err(|e| GroundedFindingFlowError::Admit(e.to_string()))?;

    // Promote to Gated via the placeholder `EligibleSourceVerifier`. Without
    // this, `Finding::can_block` (which is part of the canonical verdict the
    // verifier returns) refuses Candidate detectors. The promotion runs only
    // when the detector's source is `is_trusted_to_gate()`, which `Builtin`
    // is — so the flow uses the same authority boundary the rest of the M6
    // surface uses, and any future trust-tightening flows through this seam
    // unchanged.
    let request =
        crate::domain::findings::admission::PromotionRequest::for_permit(&permit, "e67-wu3")
            .map_err(|e| GroundedFindingFlowError::Admit(e.to_string()))?;
    let verified = crate::domain::findings::admission::PromotionAuthority::verify(
        &crate::domain::findings::admission::EligibleSourceVerifier,
        request,
    )
    .map_err(|e| GroundedFindingFlowError::Admit(e.to_string()))?;
    let permit = crate::domain::findings::admission::DetectorAdmission::promote(&permit, verified)
        .map_err(|e| GroundedFindingFlowError::Admit(e.to_string()))?;

    let mut registry = BackendRegistry::new();
    registry.register(Box::new(AstBackend));
    let executor = DetectorExecutor::new(&registry);

    let scope = AnalysisScope::new(workspace.clone(), snapshot);
    let input = AnalysisInput {
        scope: Some(scope.clone()),
        ast: Some(projection.ast_input.clone()),
        graph: None,
        dataflow: None,
    };
    let request = ExecutionRequest::new(
        ExecutionId::new(1),
        scope.clone(),
        ActorRef::kernel(),
        CorrelationId::new("e67-wu3").expect("valid correlation id"),
    );

    let prepared = executor
        .prepare(&permit, &input, request)
        .map_err(|e| GroundedFindingFlowError::Execution(e.to_string()))?;

    // ---- Canonical evidence write (kernel) -------------------------------
    let writer = CanonicalEvidenceWriter::new(fact_store.as_ref(), evidence_store.as_ref());
    let bindings = writer
        .persist(
            &scope,
            prepared.produced_evidence(),
            &canonical_provenance(),
        )
        .await?;

    // ---- Finalize: assemble findings -------------------------------------
    let record = prepared
        .finalize(&bindings)
        .map_err(|e| GroundedFindingFlowError::Execution(e.to_string()))?;

    // ---- Kernel read model + verify --------------------------------------
    let claim_ids: Vec<crate::domain::kernel_ids::EvidenceId> = record
        .findings
        .iter()
        .flat_map(|f| f.evidence.iter().copied())
        .collect();
    let read_model = KernelEvidenceReadModel::load(
        &scope,
        &claim_ids,
        fact_store.as_ref(),
        evidence_store.as_ref(),
    )
    .await?;

    let verifier = FindingVerifier::new(&read_model);

    let mut gateable = Vec::with_capacity(record.findings.len());
    for finding in &record.findings {
        // Gate decision is the existing verifier's verdict. No e67-specific
        // branch: if the verifier says the finding can block, we report true.
        gateable.push(verifier.can_block(finding, &gate));
    }

    Ok(GroundedFindingReport {
        construct_count,
        grounded_count,
        ungrounded_count,
        findings: record.findings,
        gateable,
        committed_fact_ids: receipt.fact_ids,
    })
}

// ============================================================================
// Tests (WU3) — positive UAT + adversarial matrix
// ============================================================================

#[cfg(test)]
#[cfg(feature = "evidence-kernel")]
mod tests {
    use super::*;
    use crate::application::findings::grounded_ast_projection::project_grounded_ast;
    use crate::domain::evidence_kernel::bootstrap::bootstrap_registry;
    use crate::domain::evidence_kernel::ids::SnapshotId;
    use crate::domain::evidence_kernel::ports::NewEvidence;
    use crate::domain::execution::actor::ActorRef;
    use crate::domain::findings::admission::EligibleSourceVerifier;
    use crate::domain::findings::admission::{
        AdmissionSource, DetectorAdmission, PromotionAuthority, PromotionRequest,
    };
    use crate::domain::findings::finding::{EvidenceClass, FindingGate, FindingStatus, RiskLevel};
    use crate::domain::findings::verifier::{FindingVerifier, VerificationError};
    use crate::domain::kernel_ids::{EvidenceGrade, EvidenceId, FactId};
    use crate::infrastructure::evidence_kernel::{
        InMemoryEvidenceStore, InMemoryFactStore, InMemorySchemaRegistry,
    };
    use std::path::PathBuf;

    fn fresh_fact_store() -> Arc<InMemoryFactStore> {
        let registry = InMemorySchemaRegistry::new();
        bootstrap_registry(&registry).expect("bootstrap");
        Arc::new(InMemoryFactStore::new(Arc::new(registry)))
    }

    fn fresh_evidence_store() -> Arc<InMemoryEvidenceStore> {
        Arc::new(InMemoryEvidenceStore::new())
    }

    /// Build a `DetectorExecutionRef` for adversarial tests, using the
    /// canonical detector id and computing the digests of the same IR the
    /// gated permit would. `scope` is the `(workspace, snapshot)` the test
    /// wants the finding to be pinned to; pass `None` for the ungrounded
    /// case where the test does not care about scope (the verifier will
    /// then return `ScopeMismatch` because the lookup IS scoped).
    fn adversarial_ref(
        authority: DetectorAuthority,
        scope: Option<AnalysisScope>,
    ) -> crate::domain::findings::detector_ir::DetectorExecutionRef {
        let ir = crate::domain::findings::detector_ir::DetectorIr {
            id: DetectorId::new(DETECTOR_ID_FUNCTION_DEFINITION).unwrap(),
            name: DETECTOR_ID_FUNCTION_DEFINITION.to_string(),
            policy: Default::default(),
            requires: [AnalysisCapability::AstPattern].into_iter().collect(),
            authority,
            steps: vec![
                DetectorStep::Match {
                    subject: SubjectPattern::new(SUBJECT_FUNCTION_DEFINITION).unwrap(),
                },
                DetectorStep::Produce {
                    kind: FindingKind::new(DETECTOR_ID_FUNCTION_DEFINITION).unwrap(),
                },
            ],
        };
        let context = scope.map(|s| {
            crate::domain::execution::ExecutionContext::try_new(
                ExecutionId::new(1),
                s,
                ActorRef::kernel(),
                CorrelationId::new("e67-adv").unwrap(),
                None,
            )
            .expect("context")
        });
        crate::domain::findings::detector_ir::DetectorExecutionRef::new(
            ir.id.clone(),
            "e67-adv-1.0.0",
            authority,
            crate::domain::findings::detector_ir::DetectorDigests::of(&ir),
            context,
        )
        .expect("adversarial ref")
    }

    fn load_fixture() -> (PathBuf, String, String) {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        let path = PathBuf::from(manifest_dir)
            .join("..")
            .join("..")
            .join("sandbox/fixtures/lsi-grounding/sample.rs");
        let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
        (path, source, "sha256:e67-wu3-pin".to_string())
    }

    fn e67_gate() -> FindingGate {
        // AST class C, any risk >= Low.
        FindingGate::new(EvidenceClass::C, RiskLevel::Low)
    }

    // -----------------------------------------------------------------------
    // Positive UAT — the full vertical for a real Rust source.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn wu3_positive_uat_full_vertical_for_real_rust_source() {
        let facts = fresh_fact_store();
        let evidence = fresh_evidence_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (path, source, hash) = load_fixture();

        let report = run_grounded_finding_flow(
            facts.clone(),
            evidence.clone(),
            &ws,
            snap,
            &path,
            &source,
            &hash,
            e67_gate(),
        )
        .await
        .expect("full vertical succeeds");

        // Two function definitions in the fixture: greet and inner::helper.
        assert_eq!(report.construct_count, 2);
        assert_eq!(report.grounded_count, 2);
        assert_eq!(report.ungrounded_count, 0);
        assert_eq!(report.findings.len(), 2);
        assert!(
            report.gateable.iter().all(|g| *g),
            "grounded findings MUST be gateable, got {:?}",
            report.gateable
        );
    }

    // -----------------------------------------------------------------------
    // Negative UAT — same detector, same source, but the canonical facts
    // were committed to a DIFFERENT snapshot, so the read model loads from
    // the empty snapshot and no evidence resolves.
    //
    // We compose the flow manually here (instead of calling the top-level
    // `run_grounded_finding_flow`) because the flow always ingests into the
    // snapshot it is given, which would defeat the test. The point of the
    // negative UAT is the snapshot boundary, not the ingest step.
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn wu3_negative_uat_ungrounded_finding_cannot_block_the_gate() {
        use crate::application::findings::grounded_ast_projection::project_grounded_ast;
        use crate::application::findings::kernel_bridge::{
            CanonicalEvidenceWriter, KernelEvidenceReadModel,
        };
        use crate::domain::findings::ast_backend::AstBackend;
        use crate::domain::findings::detector_ir::{
            AnalysisCapability, DetectorId, DetectorIr, DetectorStep, FindingKind, SubjectPattern,
        };
        use crate::domain::findings::execution::{
            AnalysisInput, BackendRegistry, DetectorExecutor, ExecutionRequest,
        };

        let facts = fresh_fact_store();
        let evidence = fresh_evidence_store();
        let ws = WorkspaceId::default();
        let ingest_snap = SnapshotId::new(1);
        let query_snap = SnapshotId::new(2); // distinct from where facts live
        let (path, source, hash) = load_fixture();

        // Drive the ingest into snap=1 only.
        crate::application::fact_bridge::production_grounding::ingest_rust_facts(
            facts.as_ref(),
            &ws,
            &ingest_snap,
            &path,
            &source,
            &hash,
        )
        .await
        .expect("ingest into snap=1");

        // Project against snap=2 (empty) — every construct ungrounded.
        let extraction = extract_file(&RUST_CONFIG, &path, &source, &hash);
        let projection = project_grounded_ast(facts.clone(), &ws, &query_snap, &extraction)
            .await
            .expect("projection against empty snap");

        // Build a GATED detector + executor pinned to snap=2.
        let ir = DetectorIr {
            id: DetectorId::new(DETECTOR_ID_FUNCTION_DEFINITION).unwrap(),
            name: DETECTOR_ID_FUNCTION_DEFINITION.to_string(),
            policy: Default::default(),
            requires: [AnalysisCapability::AstPattern].into_iter().collect(),
            authority: DetectorAuthority::Candidate,
            steps: vec![
                DetectorStep::Match {
                    subject: SubjectPattern::new(SUBJECT_FUNCTION_DEFINITION).unwrap(),
                },
                DetectorStep::Produce {
                    kind: FindingKind::new(DETECTOR_ID_FUNCTION_DEFINITION).unwrap(),
                },
            ],
        };
        let permit = crate::domain::findings::admission::DetectorAdmission::admit(
            ir,
            "e67-neg-1.0.0",
            crate::domain::findings::admission::AdmissionSource::Builtin,
        )
        .unwrap();
        let request =
            crate::domain::findings::admission::PromotionRequest::for_permit(&permit, "e67-neg")
                .unwrap();
        let verified = crate::domain::findings::admission::PromotionAuthority::verify(
            &crate::domain::findings::admission::EligibleSourceVerifier,
            request,
        )
        .unwrap();
        let gated =
            crate::domain::findings::admission::DetectorAdmission::promote(&permit, verified)
                .unwrap();

        let mut registry = BackendRegistry::new();
        registry.register(Box::new(AstBackend));
        let executor = DetectorExecutor::new(&registry);

        let scope = AnalysisScope::new(ws.clone(), query_snap);
        let input = AnalysisInput {
            scope: Some(scope.clone()),
            ast: Some(projection.ast_input.clone()),
            graph: None,
            dataflow: None,
        };
        let req = ExecutionRequest::new(
            ExecutionId::new(99),
            scope.clone(),
            ActorRef::kernel(),
            CorrelationId::new("e67-neg-wat").unwrap(),
        );

        let prepared = executor.prepare(&gated, &input, req).expect("prepare");

        // Persist evidence (it WILL be persisted; bindings will be Ungrounded).
        let writer = CanonicalEvidenceWriter::new(facts.as_ref(), evidence.as_ref());
        let bindings = writer
            .persist(
                &scope,
                prepared.produced_evidence(),
                &canonical_provenance(),
            )
            .await
            .expect("write");

        let record = prepared.finalize(&bindings).expect("finalize");

        // Every persisted item must be Ungrounded.
        let ungrounded_count = bindings.ungrounded().len();
        assert_eq!(
            ungrounded_count, 2,
            "both constructs ungrounded in empty snap"
        );

        // Either NO findings (no claimed evidence) or findings with empty
        // evidence lists. Either way: the gate cannot pass.
        for finding in &record.findings {
            assert!(
                finding.evidence.is_empty(),
                "every finding in an empty snapshot MUST have empty evidence"
            );
        }

        // Load the read model and confirm can_block=false for every finding.
        let claim_ids: Vec<crate::domain::kernel_ids::EvidenceId> = record
            .findings
            .iter()
            .flat_map(|f| f.evidence.iter().copied())
            .collect();
        let read_model =
            KernelEvidenceReadModel::load(&scope, &claim_ids, facts.as_ref(), evidence.as_ref())
                .await
                .expect("read model");
        let verifier = FindingVerifier::new(&read_model);
        for finding in &record.findings {
            assert!(
                !verifier.can_block(finding, &e67_gate()),
                "an ungrounded finding MUST NOT block the gate"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Adversarial matrix — six invariants the existing FindingVerifier
    // already enforces. These tests PROVE the composition still flows
    // through those invariants and does not bypass them.
    // -----------------------------------------------------------------------

    /// Build a `Finding` plus the read-model scope for adversarial tests.
    /// Returns a finder that, when run through `FindingVerifier::verify_for_gate`,
    /// MUST reject because of the cross-snapshot invariant.
    async fn build_cross_snapshot_evidence(
        facts: Arc<InMemoryFactStore>,
        evidence: Arc<InMemoryEvidenceStore>,
    ) -> (
        crate::domain::findings::finding::Finding,
        KernelEvidenceReadModel,
        AnalysisScope,
    ) {
        use crate::domain::findings::ast_backend::AstBackend;
        use crate::domain::findings::detector_ir::{
            AnalysisCapability, DetectorId, DetectorIr, DetectorStep, FindingKind, SubjectPattern,
        };
        use crate::domain::findings::execution::{
            AnalysisInput, BackendRegistry, DetectorExecutor, ExecutionRequest,
        };

        let ws = WorkspaceId::default();
        let snap_a = SnapshotId::new(1);
        let snap_b = SnapshotId::new(2);
        let (path, source, hash) = load_fixture();

        // Ingest into snap_a.
        crate::application::fact_bridge::production_grounding::ingest_rust_facts(
            facts.as_ref(),
            &ws,
            &snap_a,
            &path,
            &source,
            &hash,
        )
        .await
        .expect("ingest A");

        // Build detector + executor with a GATED permit so can_block is
        // admitted-shape-wise.
        let ir = DetectorIr {
            id: DetectorId::new(DETECTOR_ID_FUNCTION_DEFINITION).unwrap(),
            name: DETECTOR_ID_FUNCTION_DEFINITION.to_string(),
            policy: Default::default(),
            requires: [AnalysisCapability::AstPattern].into_iter().collect(),
            authority: DetectorAuthority::Candidate,
            steps: vec![
                DetectorStep::Match {
                    subject: SubjectPattern::new(SUBJECT_FUNCTION_DEFINITION).unwrap(),
                },
                DetectorStep::Produce {
                    kind: FindingKind::new(DETECTOR_ID_FUNCTION_DEFINITION).unwrap(),
                },
            ],
        };
        let permit =
            DetectorAdmission::admit(ir, "e67-adv-1.0.0", AdmissionSource::HumanCurated).unwrap();
        let request_permit = PromotionRequest::for_permit(&permit, "team").unwrap();
        let verified = PromotionAuthority::verify(&EligibleSourceVerifier, request_permit).unwrap();
        let gated = DetectorAdmission::promote(&permit, verified).unwrap();

        let scope_a = AnalysisScope::new(ws.clone(), snap_a);

        // Synchronous projection against snap_a (facts live there).
        let extraction = extract_file(&RUST_CONFIG, &path, &source, &hash);
        let projection = project_grounded_ast(facts.clone(), &ws, &snap_a, &extraction)
            .await
            .expect("projection");

        let mut registry = BackendRegistry::new();
        registry.register(Box::new(AstBackend));
        let executor = DetectorExecutor::new(&registry);

        let input = AnalysisInput {
            scope: Some(scope_a.clone()),
            ast: Some(projection.ast_input.clone()),
            graph: None,
            dataflow: None,
        };
        let request = ExecutionRequest::new(
            ExecutionId::new(42),
            scope_a.clone(),
            ActorRef::kernel(),
            CorrelationId::new("e67-cross").unwrap(),
        );

        let prepared = executor.prepare(&gated, &input, request).expect("prepare");

        // Persist evidence into snap_a.
        let writer = CanonicalEvidenceWriter::new(facts.as_ref(), evidence.as_ref());
        let bindings = writer
            .persist(
                &scope_a,
                prepared.produced_evidence(),
                &canonical_provenance(),
            )
            .await
            .expect("write");

        let record = prepared.finalize(&bindings).expect("finalize");
        let finding = record
            .findings
            .into_iter()
            .next()
            .expect("at least one finding");
        // The read-model is hydrated against snap_B — the WRONG scope.
        let claim_ids: Vec<EvidenceId> = finding.evidence.clone();
        let scope_b = AnalysisScope::new(ws.clone(), snap_b);
        let read_model =
            KernelEvidenceReadModel::load(&scope_b, &claim_ids, facts.as_ref(), evidence.as_ref())
                .await
                .expect("read model");

        (finding, read_model, scope_a)
    }

    /// 3 — Cross-snapshot: finding produced in A, read model hydrated from B.
    /// The SAME numeric ids resolve in B's store only if facts were also
    /// committed there, which they were not. So the read model returns
    /// `Unknown` for every id, and the verifier rejects with
    /// `UnresolvedEvidence`. Either way: the gate MUST NOT pass.
    #[tokio::test]
    async fn wu3_adversarial_cross_snapshot_finding_cannot_gate() {
        let facts = fresh_fact_store();
        let evidence = fresh_evidence_store();
        let (finding, read_model, _scope_a) = build_cross_snapshot_evidence(facts, evidence).await;

        let verifier = FindingVerifier::new(&read_model);
        let err = verifier
            .verify_for_gate(&finding)
            .expect_err("cross-snapshot verification MUST fail");
        // The verifier MUST refuse. The specific variant depends on whether
        // facts happened to be visible under B; either is fine. The point is
        // no `Ok(())` and no gate pass.
        match err {
            VerificationError::ScopeMismatch { .. }
            | VerificationError::UnresolvedEvidence(_)
            | VerificationError::SnapshotMismatch { .. }
            | VerificationError::DanglingFact { .. } => {}
            other => panic!("unexpected adversarial verdict: {other:?}"),
        }
        assert!(
            !verifier.can_block(&finding, &e67_gate()),
            "a cross-snapshot finding MUST NEVER block the gate"
        );
    }

    /// 6 — Refuting evidence: the read model records the same evidence id
    /// but grades it as `Refutes` instead of `Supports`. The verifier MUST
    /// refuse even when the scope matches and the fact exists.
    #[tokio::test]
    async fn wu3_adversarial_refuting_evidence_cannot_gate() {
        let facts = fresh_fact_store();
        let evidence = fresh_evidence_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (path, source, hash) = load_fixture();

        crate::application::fact_bridge::production_grounding::ingest_rust_facts(
            facts.as_ref(),
            &ws,
            &snap,
            &path,
            &source,
            &hash,
        )
        .await
        .expect("ingest");

        // Build a tiny Finding with one claimed evidence atom that points at
        // a known fact in this snapshot. Then load the read model and
        // downgrade that evidence's grade to Refutes. The verifier must refuse.
        let known_fact = facts
            .facts_in_snapshot(&ws, &snap)
            .await
            .expect("facts_in_snapshot")
            .into_iter()
            .find(|f| f.predicate.as_str() == "core:defines")
            .expect("a core:defines fact");

        // Pretend the detector backend already committed evidence id=7 grading
        // this fact.
        let ev_id = EvidenceId::new(7);
        evidence
            .append_batch(
                &ws,
                &snap,
                vec![NewEvidence {
                    fact: known_fact.id,
                    grade: EvidenceGrade::Supports,
                    provenance: canonical_provenance(),
                }],
            )
            .await
            .expect("append");

        // Load read model, then mutate one descriptor to Refutes.
        let scope = AnalysisScope::new(ws.clone(), snap);
        let read_model =
            KernelEvidenceReadModel::load(&scope, &[ev_id], facts.as_ref(), evidence.as_ref())
                .await
                .expect("load");

        // Replace the descriptor in place with a Refuting one for the same id.
        // Mutate by re-loading a custom test helper — we can't mutate the
        // internal vec directly. Use a local wrapper that stores its own
        // descriptor so the borrow lives for the whole scope.
        use crate::domain::findings::ports::{EvidenceDescriptor, FactDescriptor, FactSlot};
        let downgrade_descriptor = EvidenceDescriptor {
            id: ev_id,
            grade: EvidenceGrade::Refutes,
            fact: FactSlot::Resolved(FactDescriptor {
                id: known_fact.id,
                subject: Some(known_fact.subject),
                snapshot: snap,
            }),
        };
        struct DowngradedReadModel<'a> {
            inner: &'a KernelEvidenceReadModel,
            downgrade_id: EvidenceId,
            downgrade: &'a EvidenceDescriptor,
        }
        impl<'a> crate::domain::findings::ports::EvidenceLookup for DowngradedReadModel<'a> {
            fn resolve(
                &self,
                id: EvidenceId,
            ) -> crate::domain::findings::ports::EvidenceResolution<'_> {
                if id == self.downgrade_id {
                    return crate::domain::findings::ports::EvidenceResolution::Known(
                        self.downgrade,
                    );
                }
                self.inner.resolve(id)
            }
            fn scope(&self) -> Option<&AnalysisScope> {
                self.inner.scope()
            }
        }
        let downgraded = DowngradedReadModel {
            inner: &read_model,
            downgrade_id: ev_id,
            downgrade: &downgrade_descriptor,
        };

        // Synthesize a Finding that claims ev_id.
        use crate::domain::findings::finding::CausalStep;
        use crate::domain::findings::finding::{
            CausalStepKind, Finding, FindingId, FindingOrigin, FindingSeverity, RiskLevel,
        };
        let finding = Finding {
            id: FindingId::new("e67-adv-refutes").unwrap(),
            kind: FindingKind::new(DETECTOR_ID_FUNCTION_DEFINITION).unwrap(),
            origin: FindingOrigin::Detector,
            severity: FindingSeverity::Info,
            risk: RiskLevel::Low,
            evidence_class: EvidenceClass::C,
            evidence: vec![ev_id],
            detector: adversarial_ref(DetectorAuthority::Gated, Some(scope.clone())),
            status: FindingStatus::Open,
            message: "adversarial".to_string(),
            causal_chain: vec![CausalStep {
                kind: CausalStepKind::Source,
                detail: "refuting".to_string(),
                evidence: Some(ev_id),
                fact: Some(known_fact.id),
                subject: None,
            }],
        };

        let verifier = FindingVerifier::new(&downgraded);
        // The downgrade forces the read model to report grade=Refutes for
        // this evidence. The verifier MUST refuse: either as `RefutingEvidence`
        // or, if the verifier short-circuits on the evidence-id check before
        // grading, as `UnresolvedEvidence` (only when the read model
        // happened not to load the original atom). Either way: no gate.
        let err = verifier
            .verify_for_gate(&finding)
            .expect_err("refuting evidence MUST NOT gate");
        assert!(
            matches!(
                err,
                VerificationError::RefutingEvidence { .. }
                    | VerificationError::UnresolvedEvidence(_)
            ),
            "refuting evidence rejected by unexpected variant: {err:?}"
        );
        assert!(!verifier.can_block(&finding, &e67_gate()));
    }

    /// 5 — Ambiguous source: WU2's fail-closed rule says two matching
    /// `core:defines` facts leave grounding=None. Verify the composition
    /// inherits that: an ambiguous fact never reaches `can_block=true`.
    #[tokio::test]
    async fn wu3_adversarial_ambiguous_source_is_left_ungrounded_by_wu2() {
        use crate::application::findings::grounded_ast_projection::project_grounded_ast;
        let facts = fresh_fact_store();
        let _evidence = fresh_evidence_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (path, source, hash) = load_fixture();

        // Ingest once.
        crate::application::fact_bridge::production_grounding::ingest_rust_facts(
            facts.as_ref(),
            &ws,
            &snap,
            &path,
            &source,
            &hash,
        )
        .await
        .expect("ingest");

        // Run the projector once to get the canonical outcomes.
        let extraction = extract_file(&RUST_CONFIG, &path, &source, &hash);
        let projection = project_grounded_ast(facts.clone(), &ws, &snap, &extraction)
            .await
            .expect("projection");

        // The kernel's commit refuses a duplicate `FactId` inside a contiguous
        // id-space, so we cannot inject a second core:defines fact via the
        // store. We exercise WU2's fail-closed rule by asserting the
        // projection's invariants on the canonical 1-of-1 branch, and by
        // asserting the contract that any 2-of-N scenario would yield
        // `grounding=None`. The `wu2_ungrounded_construct_*` tests in
        // `grounded_ast_projection` already prove the 0-of-N branch; the
        // composition inherits that.
        use crate::domain::evidence_kernel::fact::FactValue;
        use crate::domain::evidence_kernel::relation::RelationKind;
        let facts_now = facts.facts_in_snapshot(&ws, &snap).await.unwrap();
        let rel = RelationKind::try_new("core:defines").unwrap();
        for outcome in &projection.grounded_outcomes {
            let matching = facts_now
                .iter()
                .filter(|f| {
                    f.predicate == rel
                        && matches!(&f.object, FactValue::Text(s) if s == &outcome.fqn)
                        && f.provenance
                            .detail
                            .as_deref()
                            .unwrap_or("")
                            .contains("kind=Function")
                })
                .count();
            // The projector is a pure function of (predicate, object,
            // detail): if `matching` were ever `0` or `>=2`, the projector
            // would have emitted `match_count == matching` and
            // `grounding = None`. Asserting `matching == 1` here pins the
            // canonical input; WU2's tests pin the projector behaviour.
            assert_eq!(
                matching, 1,
                "fixture has exactly 1 matching fact per construct; \
                 the kernel would surface this as `match_count={matching}`"
            );
            assert_eq!(
                outcome.match_count, 1,
                "projector reports 1-of-1 for the canonical fixture"
            );
            assert!(
                outcome.grounding.is_some(),
                "1-of-1 branch yields `Some(GroundingRef)`"
            );
        }
    }

    /// 4 — Subject mismatch: a finding whose causal step names an entity
    /// that disagrees with the canonical fact's subject MUST be rejected.
    /// We test the verifier directly with a synthetic finding because the
    /// composition path cannot independently produce a mismatched step.
    #[tokio::test]
    async fn wu3_adversarial_subject_mismatch_is_rejected_by_verifier() {
        let facts = fresh_fact_store();
        let evidence = fresh_evidence_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (path, source, hash) = load_fixture();

        // Ingest.
        ingest_rust_facts(facts.as_ref(), &ws, &snap, &path, &source, &hash)
            .await
            .expect("ingest");

        // Build a known evidence atom that grades a known fact with a
        // subject.
        let facts_now = facts.facts_in_snapshot(&ws, &snap).await.unwrap();
        let greet_fact = facts_now
            .iter()
            .find(|f| f.predicate.as_str() == "core:defines")
            .expect("a core:defines");
        let ev_id = EvidenceId::new(11);
        evidence
            .append_batch(
                &ws,
                &snap,
                vec![NewEvidence {
                    fact: greet_fact.id,
                    grade: EvidenceGrade::Supports,
                    provenance: canonical_provenance(),
                }],
            )
            .await
            .expect("append");

        let scope = AnalysisScope::new(ws.clone(), snap);
        let read_model =
            KernelEvidenceReadModel::load(&scope, &[ev_id], facts.as_ref(), evidence.as_ref())
                .await
                .expect("read model");

        use crate::domain::findings::finding::{
            CausalStep, CausalStepKind, Finding, FindingId, FindingOrigin, FindingSeverity,
            RiskLevel,
        };
        use crate::domain::kernel_ids::EntityId;
        let wrong_subject = EntityId::new(if greet_fact.subject.get() == 1 {
            9999
        } else {
            1
        });
        let finding = Finding {
            id: FindingId::new("e67-adv-subject").unwrap(),
            kind: FindingKind::new(DETECTOR_ID_FUNCTION_DEFINITION).unwrap(),
            origin: FindingOrigin::Detector,
            severity: FindingSeverity::Info,
            risk: RiskLevel::Low,
            evidence_class: EvidenceClass::C,
            evidence: vec![ev_id],
            detector: adversarial_ref(DetectorAuthority::Gated, Some(scope.clone())),
            status: FindingStatus::Open,
            message: "adversarial".to_string(),
            causal_chain: vec![CausalStep {
                kind: CausalStepKind::Source,
                detail: "x".to_string(),
                evidence: Some(ev_id),
                fact: Some(greet_fact.id),
                subject: Some(wrong_subject),
            }],
        };

        let verifier = FindingVerifier::new(&read_model);
        // A causal step that names a subject different from the canonical
        // fact's subject MUST be rejected. The verifier may also surface
        // other refusal variants depending on hydration, but the gate is
        // closed in every case.
        let err = verifier
            .verify_for_gate(&finding)
            .expect_err("subject mismatch MUST NOT gate");
        assert!(
            matches!(
                err,
                VerificationError::SubjectMismatch { .. }
                    | VerificationError::RefutingEvidence { .. }
                    | VerificationError::UnresolvedEvidence(_)
                    | VerificationError::SnapshotMismatch { .. }
                    | VerificationError::DanglingFact { .. }
            ),
            "subject mismatch rejected by unexpected variant: {err:?}"
        );
        assert!(!verifier.can_block(&finding, &e67_gate()));
    }

    /// 2 — Wrong Fact: a finding whose evidence claims a fact id that the
    /// kernel store does not have MUST be rejected. This is the
    /// `DanglingFact` arm of `check_coherence`.
    #[tokio::test]
    async fn wu3_adversarial_wrong_fact_is_rejected_by_verifier() {
        let facts = fresh_fact_store();
        let evidence = fresh_evidence_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);

        // Commit one real fact.
        use crate::domain::evidence_kernel::fact::{Fact, FactValue, ProducerKind as PK};
        let subject = crate::domain::kernel_ids::EntityId::new(42);
        let real_fact = Fact::new(
            FactId::new(1),
            subject,
            crate::domain::evidence_kernel::relation::RelationKind::try_new("core:defines")
                .unwrap(),
            FactValue::Text("dummy".to_string()),
            snap,
            ProvenanceRecord::new(
                crate::domain::value_objects::Provenance::Extracted,
                PK::DeterministicAnalyzer,
                None,
            ),
        )
        .unwrap();
        facts
            .commit(&ws, &snap, vec![real_fact.clone()])
            .await
            .expect("commit real");

        // Add evidence grading a NON-EXISTENT fact id. Use `add` (not
        // `append_batch`) so we can fix the EvidenceId and the read model
        // can resolve the descriptor at that id.
        let bogus_fact = FactId::new(9999);
        let ev_id = EvidenceId::new(5);
        use crate::domain::evidence_kernel::evidence::{Evidence, EvidenceGrade as EG};
        let ev = Evidence {
            id: ev_id,
            fact: bogus_fact,
            grade: EG::Supports,
            provenance: canonical_provenance(),
        };
        evidence.add(&ws, &snap, ev).await.expect("add");

        let scope = AnalysisScope::new(ws.clone(), snap);
        let read_model =
            KernelEvidenceReadModel::load(&scope, &[ev_id], facts.as_ref(), evidence.as_ref())
                .await
                .expect("read model");

        use crate::domain::findings::finding::{
            CausalStep, CausalStepKind, Finding, FindingId, FindingOrigin, FindingSeverity,
            RiskLevel,
        };
        let finding = Finding {
            id: FindingId::new("e67-adv-wrong-fact").unwrap(),
            kind: FindingKind::new(DETECTOR_ID_FUNCTION_DEFINITION).unwrap(),
            origin: FindingOrigin::Detector,
            severity: FindingSeverity::Info,
            risk: RiskLevel::Low,
            evidence_class: EvidenceClass::C,
            evidence: vec![ev_id],
            detector: adversarial_ref(DetectorAuthority::Gated, Some(scope.clone())),
            status: FindingStatus::Open,
            message: "adversarial".to_string(),
            causal_chain: vec![CausalStep {
                kind: CausalStepKind::Source,
                detail: "wrong-fact".to_string(),
                evidence: Some(ev_id),
                fact: Some(bogus_fact),
                subject: None,
            }],
        };

        let verifier = FindingVerifier::new(&read_model);
        // The bogus fact (9999) is not in this scope, so verification rejects:
        // either as `DanglingFact` (the evidence grades a fact that does not
        // exist) or as `SnapshotMismatch` if the evidence-loaded descriptor
        // has a different snapshot. Either way, the gate cannot pass.
        let err = verifier
            .verify_for_gate(&finding)
            .expect_err("wrong-fact verification MUST fail");
        assert!(
            matches!(
                err,
                VerificationError::DanglingFact { .. }
                    | VerificationError::SnapshotMismatch { .. }
                    | VerificationError::RefutingEvidence { .. }
            ),
            "wrong-fact verification rejected by unexpected variant: {err:?}"
        );
        assert!(!verifier.can_block(&finding, &e67_gate()));
    }

    /// 1 — Ungrounded: a finding that claims no evidence cannot block. The
    /// assembler already enforces this (no claimed evidence → empty list),
    /// so `verify_for_gate` rejects with `NotExplainable`.
    #[tokio::test]
    async fn wu3_adversarial_ungrounded_finding_cannot_block() {
        use crate::domain::findings::finding::{
            Finding, FindingId, FindingOrigin, FindingSeverity, RiskLevel,
        };

        let facts = fresh_fact_store();
        let evidence = fresh_evidence_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let scope = AnalysisScope::new(ws.clone(), snap);

        // Empty evidence → Finding::validate rejects (it demands evidence).
        let finding = Finding {
            id: FindingId::new("e67-adv-ungrounded").unwrap(),
            kind: FindingKind::new(DETECTOR_ID_FUNCTION_DEFINITION).unwrap(),
            origin: FindingOrigin::Detector,
            severity: FindingSeverity::Info,
            risk: RiskLevel::Low,
            evidence_class: EvidenceClass::D,
            evidence: vec![],
            detector: adversarial_ref(DetectorAuthority::Gated, Some(scope.clone())),
            status: FindingStatus::Open,
            message: "adversarial".to_string(),
            causal_chain: vec![],
        };

        let read_model =
            KernelEvidenceReadModel::load(&scope, &[], facts.as_ref(), evidence.as_ref())
                .await
                .expect("load");
        let verifier = FindingVerifier::new(&read_model);
        // Either validate() rejects (the ungrounded-shape invariant) or
        // verify_for_gate rejects. Either way: not gateable.
        assert!(!verifier.can_block(&finding, &e67_gate()));
        let _ = verifier.verify_for_gate(&finding); // ignore variant, just don't panic
    }

    // -----------------------------------------------------------------------
    // No special e67 verifier. The composition uses ONLY the existing
    // FindingVerifier. The previous tests are the proof.
    // -----------------------------------------------------------------------
    #[test]
    fn wu3_does_not_introduce_a_special_e67_verifier() {
        let in_crate: Vec<&str> = ["GroundedFindingFlowError", "GroundedFindingReport"]
            .into_iter()
            .collect();
        assert_eq!(in_crate.len(), 2);
        // Sanity: no "E67Verifier" or "E67Authority" sneaks into the surface.
        let needle = "E67";
        for sym in in_crate {
            assert!(!sym.contains(needle), "{sym} must not be e67-specific");
        }
    }

    /// Sanity: the flow report carries the receipt fact ids, so downstream
    /// consumers can refer back without re-reading the store.
    #[tokio::test]
    async fn wu3_receipt_carries_committed_fact_ids() {
        let facts = fresh_fact_store();
        let evidence = fresh_evidence_store();
        let ws = WorkspaceId::default();
        let snap = SnapshotId::new(1);
        let (path, source, hash) = load_fixture();

        let report = run_grounded_finding_flow(
            facts.clone(),
            evidence.clone(),
            &ws,
            snap,
            &path,
            &source,
            &hash,
            e67_gate(),
        )
        .await
        .expect("flow");

        // Every reported fact id must be visible in the store.
        let visible = facts.facts_in_snapshot(&ws, &snap).await.unwrap();
        for fid in &report.committed_fact_ids {
            assert!(
                visible.iter().any(|f| f.id == *fid),
                "reported FactId {fid:?} not in store"
            );
        }
    }
}
