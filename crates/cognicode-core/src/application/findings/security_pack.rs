//! The security pack runner: classification, admission, execution and the
//! blocking decision, composed over the unchanged findings seam.
//!
//! ```text
//! observed source ─ classify ─▶ DataflowInput | AstInput ─▶ DetectorExecutor
//!   (catalog adapters)            (subjects)                 (planner + backend)
//!        ─▶ Finding ─▶ FindingVerifier::can_block(family gate)
//! ```
//!
//! The runner owns no analysis logic and no evidence store: reachability is the
//! M5 backend's, matching is the AST backend's, and persistence is whatever
//! store the caller passes in. What it owns is the pack's contract: which
//! authority a run has, which gate a family declares, and which verdict a
//! result may carry.

use crate::application::findings::M5DataflowBackend;
use crate::application::program_analysis::ProgramAnalysisService;
use crate::domain::execution::{ActorRef, CorrelationId};
use crate::domain::findings::semantic_model::{
    AnalysisReach, CapabilityMatrix, Classification, FindingNature, ObservedFunction, ObservedUnit,
    PACK_VERSION, PackVerdict, SastVerdict, SecurityFamily, SemanticModelCatalog,
    StructuralClassification, StructuralFamily, Weakness, assess, classify_structural,
    unresolved_checks,
};
use crate::domain::findings::{
    AdmissionError, AdmissionSource, AnalysisInput, AnalysisScope, AstBackend, BackendRegistry,
    DetectorAdmission, DetectorDiagnostic, DetectorExecutor, DetectorIr, EligibleSourceVerifier,
    EvidenceLookup, EvidenceSink, ExecutionError, ExecutionPermit, ExecutionRequest, Finding,
    FindingVerifier, GraphBackend, PromotionAuthority, PromotionRequest,
};
use crate::domain::kernel_ids::ExecutionId;

/// The authority a detector holds for a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunAuthority {
    /// Admitted as a candidate: it finds and explains, but can never block.
    Candidate,
    /// Promoted by a named approver: its findings may block when they clear the
    /// family's gate.
    Promoted {
        /// Who approved the promotion.
        approver: String,
    },
}

/// Where and as whom a run executes.
#[derive(Debug, Clone)]
pub struct RunContext {
    /// The snapshot the run is scoped to.
    pub scope: AnalysisScope,
    /// Identity of this execution.
    pub execution_id: ExecutionId,
    /// Correlation across the surrounding operation.
    pub correlation: CorrelationId,
    /// The authority of the detector.
    pub authority: RunAuthority,
}

/// Why a run could not be carried out (as opposed to finding nothing).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PackError {
    /// The detector could not be admitted or promoted.
    #[error("detector admission failed: {0:?}")]
    Admission(AdmissionError),
    /// The executor refused or failed the run.
    #[error("detector execution failed: {0:?}")]
    Execution(ExecutionError),
}

/// The result of one family over one function or unit.
#[derive(Debug, Clone)]
pub struct PackRun {
    /// The family that ran.
    pub family: SecurityFamily,
    /// What the result may claim.
    pub verdict: PackVerdict,
    /// The findings, with their evidence class and causal chain.
    pub findings: Vec<Finding>,
    /// How many findings clear the family's declared blocking level. Always
    /// zero for a candidate detector, ungrounded evidence, a weaker evidence
    /// class than declared, or a family that never blocks.
    pub blocking: usize,
    /// Notes qualifying the verdict.
    pub diagnostics: Vec<DetectorDiagnostic>,
    /// The backend that produced the evidence; `None` when nothing ran.
    pub backend: Option<String>,
}

impl PackRun {
    fn not_run(
        family: SecurityFamily,
        verdict: PackVerdict,
        diag: Vec<DetectorDiagnostic>,
    ) -> Self {
        Self {
            family,
            verdict,
            findings: Vec::new(),
            blocking: 0,
            diagnostics: diag,
            backend: None,
        }
    }
}

/// What the executor produced, before the pack decides the verdict.
struct Executed {
    findings: Vec<Finding>,
    blocking: usize,
    backend: String,
}

impl Executed {
    fn into_run(
        self,
        family: SecurityFamily,
        verdict: PackVerdict,
        diagnostics: Vec<DetectorDiagnostic>,
    ) -> PackRun {
        PackRun {
            family,
            verdict,
            findings: self.findings,
            blocking: self.blocking,
            diagnostics,
            backend: Some(self.backend),
        }
    }
}

/// The security pack over a catalog and a backend registry.
pub struct SecurityPack {
    catalog: SemanticModelCatalog,
    registry: BackendRegistry,
}

impl SecurityPack {
    /// A pack over explicit parts, for composing with other backends.
    pub fn new(catalog: SemanticModelCatalog, registry: BackendRegistry) -> Self {
        Self { catalog, registry }
    }

    /// The shipped pack: the built-in catalog with the AST, graph and M5
    /// dataflow backends.
    pub fn standard() -> Self {
        let mut registry = BackendRegistry::new();
        registry.register(Box::new(AstBackend));
        registry.register(Box::new(GraphBackend));
        registry.register(Box::new(M5DataflowBackend::new(
            ProgramAnalysisService::new(),
        )));
        Self::new(SemanticModelCatalog::builtin(), registry)
    }

    /// The capability matrix of this pack, derived from its catalog and the
    /// backends it can plan.
    pub fn capability_matrix(&self) -> CapabilityMatrix {
        CapabilityMatrix::derive(&self.catalog, &self.registry)
    }

    /// Run a taint family over one observed function.
    pub fn analyse_taint<S>(
        &self,
        weakness: Weakness,
        function: &ObservedFunction,
        context: &RunContext,
        store: &mut S,
    ) -> Result<PackRun, PackError>
    where
        S: EvidenceSink + EvidenceLookup,
    {
        let family = SecurityFamily::of_weakness(weakness);
        let classification = self.catalog.classify(weakness, function);
        let Classification::Modeled(modeled) = &classification else {
            let a = assess(&classification, 0);
            return Ok(PackRun::not_run(
                family,
                verdict_of(a.verdict, family),
                a.diagnostics,
            ));
        };

        let permit = permit_for(family.detector(AnalysisReach::Intraprocedural), context)?;
        let input = AnalysisInput {
            scope: Some(context.scope.clone()),
            ast: None,
            graph: None,
            dataflow: Some(modeled.dataflow_input()),
        };
        let executed = self.execute(family, &permit, &input, context, store)?;
        let assessment = assess(&classification, executed.findings.len());
        Ok(executed.into_run(
            family,
            verdict_of(assessment.verdict, family),
            assessment.diagnostics,
        ))
    }

    /// Run a structural family over one observed unit.
    pub fn analyse_structural<S>(
        &self,
        family: StructuralFamily,
        unit: &ObservedUnit,
        context: &RunContext,
        store: &mut S,
    ) -> Result<PackRun, PackError>
    where
        S: EvidenceSink + EvidenceLookup,
    {
        let pack_family = SecurityFamily::of_structural(family);
        let ast = match classify_structural(family, unit) {
            StructuralClassification::Modeled(ast) => ast,
            StructuralClassification::Unsupported { language, family } => {
                let reason = format!(
                    "{} has no model for {}",
                    family.finding_kind(),
                    language.name()
                );
                return Ok(PackRun::not_run(
                    pack_family,
                    PackVerdict::Unsupported {
                        reason: reason.clone(),
                    },
                    vec![DetectorDiagnostic {
                        code: "sast.family_unsupported".to_string(),
                        message: reason,
                    }],
                ));
            }
        };

        let permit = permit_for(family.detector(), context)?;
        let input = AnalysisInput {
            scope: Some(context.scope.clone()),
            ast: Some(ast),
            graph: None,
            dataflow: None,
        };
        let executed = self.execute(pack_family, &permit, &input, context, store)?;
        let unresolved = unresolved_checks(family, unit);
        let mut diagnostics = Vec::new();
        let verdict = if executed.findings.is_empty() {
            if unresolved.is_empty() {
                PackVerdict::NotObserved {
                    within: "constructs of the family in the analysed unit".to_string(),
                }
            } else {
                // Nothing was verified, but checks that could not decide
                // remain: the unit is not shown clean.
                PackVerdict::Unknown { unresolved }
            }
        } else {
            if !unresolved.is_empty() {
                diagnostics.push(DetectorDiagnostic {
                    code: "sast.unresolved_checks".to_string(),
                    message: format!(
                        "{} further check(s) could not decide: {}",
                        unresolved.len(),
                        unresolved.join(" | ")
                    ),
                });
            }
            PackVerdict::Found {
                nature: pack_family.nature(),
                findings: executed.findings.len(),
            }
        };
        Ok(executed.into_run(pack_family, verdict, diagnostics))
    }

    fn execute<S>(
        &self,
        family: SecurityFamily,
        permit: &ExecutionPermit,
        input: &AnalysisInput,
        context: &RunContext,
        store: &mut S,
    ) -> Result<Executed, PackError>
    where
        S: EvidenceSink + EvidenceLookup,
    {
        let request = ExecutionRequest::new(
            context.execution_id,
            context.scope.clone(),
            ActorRef::detector("security.pack"),
            context.correlation.clone(),
        );
        let record = DetectorExecutor::new(&self.registry)
            .execute(permit, input, store, request)
            .map_err(PackError::Execution)?;

        let verifier = FindingVerifier::new(&*store);
        let blocking = match family.blocking().gate() {
            Some(gate) => record
                .findings
                .iter()
                .filter(|f| verifier.can_block(f, &gate))
                .count(),
            None => 0,
        };
        Ok(Executed {
            findings: record.findings,
            blocking,
            backend: record.backend,
        })
    }
}

fn permit_for(detector: DetectorIr, context: &RunContext) -> Result<ExecutionPermit, PackError> {
    let candidate = DetectorAdmission::admit(detector, PACK_VERSION, AdmissionSource::HumanCurated)
        .map_err(PackError::Admission)?;
    match &context.authority {
        RunAuthority::Candidate => Ok(candidate),
        RunAuthority::Promoted { approver } => {
            let request =
                PromotionRequest::for_permit(&candidate, approver).map_err(PackError::Admission)?;
            let verified = PromotionAuthority::verify(&EligibleSourceVerifier, request)
                .map_err(PackError::Admission)?;
            DetectorAdmission::promote(&candidate, verified).map_err(PackError::Admission)
        }
    }
}

fn verdict_of(verdict: SastVerdict, family: SecurityFamily) -> PackVerdict {
    match verdict {
        SastVerdict::Vulnerable { findings } => PackVerdict::Found {
            nature: FindingNature::Vulnerability,
            findings,
        },
        SastVerdict::NoFlowObserved { reach } => PackVerdict::NotObserved {
            within: format!("{} flow", reach.name()),
        },
        SastVerdict::Unknown { unresolved } => PackVerdict::Unknown { unresolved },
        SastVerdict::Unsupported { language } => PackVerdict::Unsupported {
            reason: format!(
                "{} has no model for {}",
                family.finding_kind(),
                language.name()
            ),
        },
    }
}
