//! Detector executor — the mandatory execution seam (M6, cycle e57).
//!
//! ```text
//! ExecutionPermit ──► DetectorExecutor::prepare ──► BackendRegistry::plan
//!                             │                          │
//!                             │                          ▼
//!                             │                    DetectorBackend (e.g. AstBackend)
//!                             │                          │
//!                             │                          ▼
//!                             │                    DetectorOutcome
//!                             ▼
//!                     PreparedExecution
//!                             │
//!              ┌──────────────┴──────────────┐
//!              ▼                             ▼
//!   EvidenceSink (sync)            application bridge (async)
//!              └──────────────┬──────────────┘
//!                             ▼
//!                   PreparedExecution::finalize
//!                             │
//!                             ▼
//!                     FindingAssembler ──► Finding
//! ```
//!
//! The split exists so evidence can be persisted **asynchronously outside the
//! domain** (the kernel store is async; the domain must not `block_on`). It is
//! not an escape hatch: [`PreparedExecution`] has private fields and can only
//! be created by [`DetectorExecutor::prepare`], so admission, planning and the
//! backend-contract checks cannot be skipped. [`DetectorExecutor::execute`] is
//! implemented as `prepare` + `finalize` so there is exactly one
//! implementation of the seam.
//!
//! The executor **only** accepts an [`AdmittedDetector`], so a raw
//! [`DetectorIr`](super::DetectorIr) whose authority the caller claims can
//! never be executed directly. Backends never build [`Finding`]s — they return
//! raw [`DetectorOutcome`]s and the assembler owns the rest.
//!
//! Pure domain: backends reach analysis primitives through their input and
//! persist evidence only through the [`EvidenceSink`] port.

use std::collections::BTreeSet;
use std::fmt;

use super::admission::{AdmittedDetector, ExecutionPermit};
use super::assembler::{AssemblyError, FindingAssembler};
use super::ast_backend::AstInput;
use super::binding::EvidenceBindings;
use super::dataflow_input::DataflowInput;
use super::detector_ir::{AnalysisCapability, DetectorExecutionRef, DetectorIrError};
use super::finding::{EvidenceClass, Finding, FindingGate};
use super::graph_backend::GraphInput;
use super::outcome::{DetectorDiagnostic, DetectorOutcome, ProducedEvidence};
use super::ports::{EvidenceError, EvidenceSink};
use super::scope::AnalysisScope;
use super::verifier::{FindingVerifier, VerificationError};
use crate::domain::kernel_ids::ExecutionId;

/// The shared input envelope handed to a backend.
///
/// Views are optional: a backend advertises the capabilities it provides and
/// fails loud ([`BackendError::MissingInput`]) when its view is absent.
#[derive(Debug, Clone, Default)]
pub struct AnalysisInput {
    /// The exact `(workspace, snapshot)` the views were projected from.
    ///
    /// Required for execution: `FactId`/`EvidenceId` are canonical per
    /// snapshot, so a run without a scope could not be verified later.
    pub scope: Option<AnalysisScope>,
    /// AST/construct view (AstBackend).
    pub ast: Option<AstInput>,
    /// Graph view (GraphBackend).
    pub graph: Option<GraphInput>,
    /// Dataflow view (dataflow backends, e.g. `M5DataflowBackend`).
    pub dataflow: Option<DataflowInput>,
}

/// A backend that executes a detector against an [`AnalysisInput`].
pub trait DetectorBackend {
    /// Stable backend name (used in the execution record).
    fn name(&self) -> &'static str;

    /// Capabilities this backend provides.
    fn capabilities(&self) -> BTreeSet<AnalysisCapability>;

    /// The strongest evidence class this backend may claim.
    ///
    /// A backend that produces evidence stronger than its ceiling is in
    /// breach of contract: the executor rejects the run instead of trusting
    /// an overstated backend (e.g. AST claiming `RuntimeTrace`).
    fn evidence_ceiling(&self) -> EvidenceClass;

    /// Run the (already admitted) detector against the input.
    fn run(
        &self,
        admitted: &AdmittedDetector,
        input: &AnalysisInput,
    ) -> Result<DetectorOutcome, BackendError>;
}

/// A registry of backends with a capability-based planner.
#[derive(Default)]
pub struct BackendRegistry {
    backends: Vec<Box<dyn DetectorBackend>>,
}

impl BackendRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a backend.
    pub fn register(&mut self, backend: Box<dyn DetectorBackend>) {
        self.backends.push(backend);
    }

    /// Choose the first backend that provides every required capability.
    pub fn plan(
        &self,
        requires: &BTreeSet<AnalysisCapability>,
    ) -> Result<&dyn DetectorBackend, PlanError> {
        for backend in &self.backends {
            let caps = backend.capabilities();
            if requires.is_subset(&caps) {
                return Ok(backend.as_ref());
            }
        }

        let available: BTreeSet<AnalysisCapability> = self
            .backends
            .iter()
            .flat_map(|b| b.capabilities())
            .collect();
        Err(PlanError {
            missing: requires.difference(&available).copied().collect(),
        })
    }

    /// Number of registered backends.
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    /// Whether no backend is registered.
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }
}

/// The mandatory execution seam.
pub struct DetectorExecutor<'a> {
    registry: &'a BackendRegistry,
}

impl<'a> DetectorExecutor<'a> {
    /// Construct an executor over a registry.
    pub fn new(registry: &'a BackendRegistry) -> Self {
        Self { registry }
    }

    /// Run everything that does not need I/O: admission, planning, the
    /// backend, and the backend-contract checks.
    ///
    /// The returned [`PreparedExecution`] carries the raw outcome and the
    /// execution reference. Evidence is **not** persisted yet: that is the
    /// async step the caller performs (directly with an [`EvidenceSink`], or
    /// through the application bridge that writes to the kernel).
    ///
    /// Takes an [`ExecutionPermit`] (not a bare [`AdmittedDetector`]): the
    /// permit can only be minted by
    /// [`DetectorAdmission`](super::DetectorAdmission), so a caller cannot
    /// assert authority.
    pub fn prepare(
        &self,
        permit: &ExecutionPermit,
        input: &AnalysisInput,
        execution_id: ExecutionId,
    ) -> Result<PreparedExecution, ExecutionError> {
        let admitted = permit.admitted();

        // A run must be pinned to a scope: without one its ids could not be
        // verified against any snapshot later.
        let scope = input.scope.clone().ok_or(ExecutionError::MissingScope)?;
        if !scope.is_valid() {
            return Err(ExecutionError::InvalidScope);
        }

        // The admitted definition is re-validated: admission validated it, but
        // the executor never trusts an unvalidated definition.
        admitted
            .definition
            .validate()
            .map_err(ExecutionError::InvalidDefinition)?;

        let backend = self
            .registry
            .plan(&admitted.definition.requires)
            .map_err(ExecutionError::Plan)?;

        let outcome = backend
            .run(admitted, input)
            .map_err(ExecutionError::Backend)?;

        // A backend may not invent a finding kind: every match must carry the
        // single kind the detector's PRODUCE step declares.
        let expected_kind = admitted.definition.produced_kind().cloned();
        for m in &outcome.matches {
            if expected_kind.as_ref() != Some(&m.kind) {
                return Err(ExecutionError::BackendContract(
                    BackendContractViolation::UnexpectedFindingKind {
                        backend: backend.name().to_string(),
                        expected: expected_kind.clone(),
                        produced: m.kind.clone(),
                    },
                ));
            }
        }

        // A backend may not claim evidence stronger than its declared ceiling,
        // so it cannot inflate a finding's evidence class.
        let ceiling = backend.evidence_ceiling();
        for (index, produced) in outcome.produced_evidence.iter().enumerate() {
            let class = produced.kind.class();
            if class < ceiling {
                return Err(ExecutionError::BackendContract(
                    BackendContractViolation::EvidenceCeilingExceeded {
                        backend: backend.name().to_string(),
                        index,
                        kind: produced.kind,
                        class,
                        ceiling,
                    },
                ));
            }
        }

        let execution = permit
            .execution_ref(Some(execution_id), Some(scope))
            .map_err(ExecutionError::InvalidDefinition)?;

        Ok(PreparedExecution {
            admitted: admitted.clone(),
            backend: backend.name().to_string(),
            execution,
            outcome,
        })
    }

    /// Execute a permitted detector, persisting evidence synchronously.
    ///
    /// A convenience over [`prepare`](Self::prepare) for callers that already
    /// hold a sync [`EvidenceSink`]. Callers that must do async I/O (such as
    /// the kernel write bridge) use `prepare` + `finalize` directly.
    pub fn execute(
        &self,
        permit: &ExecutionPermit,
        input: &AnalysisInput,
        sink: &mut dyn EvidenceSink,
        execution_id: ExecutionId,
    ) -> Result<ExecutionRecord, ExecutionError> {
        let prepared = self.prepare(permit, input, execution_id)?;
        let bindings = sink
            .persist(prepared.produced_evidence())
            .map_err(ExecutionError::Evidence)?;
        prepared.finalize(&bindings)
    }
}

/// An execution that has run but whose evidence has not been persisted yet.
///
/// Fields are private and the only constructor is
/// [`DetectorExecutor::prepare`], so a `PreparedExecution` is proof that
/// admission, planning and the backend-contract checks all ran.
#[derive(Debug, Clone)]
pub struct PreparedExecution {
    admitted: AdmittedDetector,
    backend: String,
    execution: DetectorExecutionRef,
    outcome: DetectorOutcome,
}

impl PreparedExecution {
    /// The admitted detector that ran.
    pub fn admitted(&self) -> &AdmittedDetector {
        &self.admitted
    }

    /// The backend that ran.
    pub fn backend(&self) -> &str {
        &self.backend
    }

    /// The detector execution reference (captures the scope).
    pub fn execution(&self) -> &DetectorExecutionRef {
        &self.execution
    }

    /// The raw backend outcome.
    pub fn outcome(&self) -> &DetectorOutcome {
        &self.outcome
    }

    /// The evidence that still needs persisting, in backend order.
    pub fn produced_evidence(&self) -> &[ProducedEvidence] {
        &self.outcome.produced_evidence
    }

    /// The scope this run is pinned to.
    pub fn scope(&self) -> Option<&AnalysisScope> {
        self.execution.scope.as_ref()
    }

    /// Assemble findings once the evidence has been persisted.
    ///
    /// `bindings` must be index-aligned with [`Self::produced_evidence`].
    pub fn finalize(self, bindings: &EvidenceBindings) -> Result<ExecutionRecord, ExecutionError> {
        let findings =
            FindingAssembler::assemble(&self.admitted, &self.execution, &self.outcome, bindings)
                .map_err(ExecutionError::Assembly)?;

        Ok(ExecutionRecord {
            backend: self.backend,
            execution: self.execution,
            findings,
            bindings: bindings.clone(),
            diagnostics: self.outcome.diagnostics,
        })
    }
}

/// The result of one detector execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRecord {
    /// Backend that ran.
    pub backend: String,
    /// The detector as executed.
    pub execution: DetectorExecutionRef,
    /// Findings produced.
    pub findings: Vec<Finding>,
    /// How each produced evidence item was bound (index-aligned), so a caller
    /// can tell a grounded route from a merely explainable one.
    pub bindings: EvidenceBindings,
    /// Non-fatal diagnostics.
    pub diagnostics: Vec<DetectorDiagnostic>,
}

impl ExecutionRecord {
    /// The findings that pass referential verification AND the gate.
    pub fn verified_blocking<'f>(
        &'f self,
        verifier: &FindingVerifier<'_>,
        gate: &FindingGate,
    ) -> Vec<&'f Finding> {
        self.findings
            .iter()
            .filter(|f| verifier.can_block(f, gate))
            .collect()
    }

    /// Why each finding failed verification (empty when all pass).
    pub fn verification_failures(
        &self,
        verifier: &FindingVerifier<'_>,
    ) -> Vec<(usize, VerificationError)> {
        self.findings
            .iter()
            .enumerate()
            .filter_map(|(i, f)| verifier.verify_for_gate(f).err().map(|e| (i, e)))
            .collect()
    }
}

/// A backend failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// The backend's required input view was absent.
    MissingInput(&'static str),
    /// The detector uses an IR construct this backend does not support yet.
    UnsupportedIr(String),
    /// The underlying analysis engine failed. An engine error must never be
    /// silently degraded into "nothing found".
    Analysis(String),
    /// An internal backend failure.
    Internal(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInput(view) => write!(f, "backend input `{view}` is missing"),
            Self::UnsupportedIr(msg) => write!(f, "unsupported detector IR: {msg}"),
            Self::Analysis(msg) => write!(f, "analysis engine error: {msg}"),
            Self::Internal(msg) => write!(f, "backend failure: {msg}"),
        }
    }
}

impl std::error::Error for BackendError {}

/// No registered backend provides the required capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanError {
    /// Capabilities required but not provided by any backend.
    pub missing: BTreeSet<AnalysisCapability>,
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.missing.iter().map(|c| c.name()).collect();
        write!(
            f,
            "no backend provides required capabilities: [{}]",
            names.join(", ")
        )
    }
}

impl std::error::Error for PlanError {}

/// A backend breached its declared contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendContractViolation {
    /// The backend produced a finding kind other than the detector's PRODUCE.
    UnexpectedFindingKind {
        /// Backend name.
        backend: String,
        /// The kind the detector declares.
        expected: Option<super::FindingKind>,
        /// The kind the backend produced.
        produced: super::FindingKind,
    },
    /// The backend produced evidence stronger than its declared ceiling.
    EvidenceCeilingExceeded {
        /// Backend name.
        backend: String,
        /// Index of the offending evidence.
        index: usize,
        /// The evidence kind produced.
        kind: super::outcome::EvidenceKind,
        /// The class that kind implies.
        class: EvidenceClass,
        /// The backend's declared ceiling.
        ceiling: EvidenceClass,
    },
}

impl fmt::Display for BackendContractViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedFindingKind {
                backend,
                expected,
                produced,
            } => write!(
                f,
                "backend `{backend}` produced finding kind `{produced}`, but the detector declares `{}`",
                expected
                    .as_ref()
                    .map(|k| k.to_string())
                    .unwrap_or_else(|| "<none>".to_string())
            ),
            Self::EvidenceCeilingExceeded {
                backend,
                index,
                kind,
                class,
                ceiling,
            } => write!(
                f,
                "backend `{backend}` produced evidence[{index}] ({kind}) at class {class}, stronger than its ceiling {ceiling}"
            ),
        }
    }
}

impl std::error::Error for BackendContractViolation {}

/// Why execution failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    /// The admitted definition failed validation.
    InvalidDefinition(DetectorIrError),
    /// The input carried no analysis scope.
    MissingScope,
    /// The input's scope is pinned to the invalid `SnapshotId::NONE` sentinel.
    InvalidScope,
    /// No backend could satisfy the requirements.
    Plan(PlanError),
    /// A backend failed.
    Backend(BackendError),
    /// A backend breached its contract.
    BackendContract(BackendContractViolation),
    /// Persisting evidence failed.
    Evidence(EvidenceError),
    /// Assembling findings failed.
    Assembly(AssemblyError),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDefinition(err) => write!(f, "invalid detector definition: {err}"),
            Self::MissingScope => {
                f.write_str("analysis input carries no (workspace, snapshot) scope")
            }
            Self::InvalidScope => {
                f.write_str("analysis input scope is pinned to the invalid snapshot sentinel")
            }
            Self::Plan(err) => write!(f, "planning failed: {err}"),
            Self::Backend(err) => write!(f, "backend failed: {err}"),
            Self::BackendContract(err) => write!(f, "backend contract violation: {err}"),
            Self::Evidence(err) => write!(f, "{err}"),
            Self::Assembly(err) => write!(f, "assembly failed: {err}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::findings::SubjectPattern;
    use crate::domain::findings::admission::{AdmissionSource, DetectorAdmission};
    use crate::domain::findings::ast_backend::AstBackend;
    use crate::domain::findings::detector_ir::{DetectorId, DetectorIr, DetectorStep, FindingKind};
    use crate::domain::findings::outcome::{
        DetectorMatch, DetectorOutcome, EvidenceKind, ProducedEvidence,
    };
    use crate::domain::kernel_ids::EvidenceId;

    fn scope() -> AnalysisScope {
        AnalysisScope::new(
            crate::domain::value_objects::WorkspaceId::try_new("ws").unwrap(),
            crate::domain::kernel_ids::SnapshotId::new(1),
        )
    }

    /// A sink that discards evidence and assigns sequential ids, grounding
    /// whatever the backend grounded.
    #[derive(Default)]
    struct CountingSink(usize);
    impl EvidenceSink for CountingSink {
        fn persist(
            &mut self,
            produced: &[super::super::ProducedEvidence],
        ) -> Result<EvidenceBindings, EvidenceError> {
            let mut entries = Vec::with_capacity(produced.len());
            for item in produced {
                self.0 += 1;
                let id = EvidenceId::new(self.0 as u64);
                entries.push(match item.grounding {
                    Some(g) => super::super::EvidenceBinding::grounded(id, g.fact),
                    None => super::super::EvidenceBinding::ungrounded(
                        super::super::GroundingFailure::NoFact,
                    ),
                });
            }
            Ok(EvidenceBindings::new(entries))
        }
    }

    /// A backend that declares a class-C ceiling but emits runtime evidence.
    struct OverclaimingBackend;
    impl DetectorBackend for OverclaimingBackend {
        fn name(&self) -> &'static str {
            "overclaiming"
        }
        fn capabilities(&self) -> BTreeSet<AnalysisCapability> {
            [AnalysisCapability::AstPattern].into_iter().collect()
        }
        fn evidence_ceiling(&self) -> EvidenceClass {
            EvidenceClass::C
        }
        fn run(
            &self,
            _admitted: &AdmittedDetector,
            _input: &AnalysisInput,
        ) -> Result<DetectorOutcome, BackendError> {
            Ok(DetectorOutcome {
                produced_evidence: vec![ProducedEvidence {
                    kind: EvidenceKind::RuntimeTrace, // class A > ceiling C
                    detail: "overclaimed".to_string(),
                    subject: None,
                    grounding: None,
                }],
                matches: vec![DetectorMatch {
                    // Must match the detector's PRODUCE kind to reach the
                    // evidence-ceiling check.
                    kind: FindingKind::new("security.weak_hash").unwrap(),
                    message: "overclaimed".to_string(),
                    evidence: vec![],
                    causal: vec![],
                }],
                diagnostics: vec![],
            })
        }
    }

    fn detector_with(requires: BTreeSet<AnalysisCapability>) -> DetectorIr {
        DetectorIr {
            id: DetectorId::new("security.weak_hash").unwrap(),
            name: "weak hash".to_string(),
            policy: super::super::DetectorFindingPolicy::default(),
            requires,
            authority: super::super::DetectorAuthority::Candidate,
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

    #[test]
    fn planner_errors_when_no_backend_covers_capabilities() {
        let registry = BackendRegistry::new();
        let mut requires = BTreeSet::new();
        requires.insert(AnalysisCapability::Dataflow);
        let err = registry.plan(&requires).err().expect("expected no backend");
        assert!(err.missing.contains(&AnalysisCapability::Dataflow));
    }

    #[test]
    fn planner_selects_a_capable_backend() {
        let mut registry = BackendRegistry::new();
        registry.register(Box::new(AstBackend));
        let mut requires = BTreeSet::new();
        requires.insert(AnalysisCapability::AstPattern);
        assert_eq!(registry.plan(&requires).unwrap().name(), "ast");
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
    }
    #[test]
    fn ast_is_not_semantic() {
        // AstBackend advertises only AstPattern, so a detector requiring
        // SemanticResolution cannot be planned onto it.
        let mut registry = BackendRegistry::new();
        registry.register(Box::new(AstBackend));
        let mut requires = BTreeSet::new();
        requires.insert(AnalysisCapability::SemanticResolution);
        let err = registry.plan(&requires).err().expect("no backend");
        assert!(
            err.missing
                .contains(&AnalysisCapability::SemanticResolution)
        );
    }

    #[test]
    fn backend_cannot_overclaim_evidence_class() {
        // A backend that declares a C ceiling may not emit runtime (A) evidence.
        let mut registry = BackendRegistry::new();
        registry.register(Box::new(OverclaimingBackend));
        let mut requires = BTreeSet::new();
        requires.insert(AnalysisCapability::AstPattern);
        let permit =
            DetectorAdmission::admit(detector_with(requires), "1.0.0", AdmissionSource::Builtin)
                .unwrap();

        let executor = DetectorExecutor::new(&registry);
        let mut sink = CountingSink::default();
        let err = executor
            .execute(
                &permit,
                &AnalysisInput {
                    scope: Some(scope()),
                    ..Default::default()
                },
                &mut sink,
                ExecutionId::new(1),
            )
            .expect_err("overclaim must be rejected");
        match err {
            ExecutionError::BackendContract(
                BackendContractViolation::EvidenceCeilingExceeded { class, ceiling, .. },
            ) => {
                assert_eq!(class, EvidenceClass::A);
                assert_eq!(ceiling, EvidenceClass::C);
            }
            other => panic!("expected a contract violation, got {other:?}"),
        }
    }
    /// e62.3 WU0: `SnapshotId::NONE` is the kernel's invalid sentinel, so a
    /// scope pinned to it is not pinned at all and must be refused before the
    /// backend runs — otherwise `Some(scope)` would satisfy the scope
    /// requirement while carrying no snapshot.
    #[test]
    fn a_scope_pinned_to_the_none_sentinel_is_refused() {
        let mut registry = BackendRegistry::new();
        registry.register(Box::new(AstBackend));
        let mut requires = BTreeSet::new();
        requires.insert(AnalysisCapability::AstPattern);
        let permit =
            DetectorAdmission::admit(detector_with(requires), "1.0.0", AdmissionSource::Builtin)
                .unwrap();

        let executor = DetectorExecutor::new(&registry);
        let mut sink = CountingSink::default();
        let unpinned = AnalysisScope::new(
            crate::domain::value_objects::WorkspaceId::try_new("ws").unwrap(),
            crate::domain::kernel_ids::SnapshotId::NONE,
        );
        let err = executor
            .execute(
                &permit,
                &AnalysisInput {
                    scope: Some(unpinned),
                    ..Default::default()
                },
                &mut sink,
                ExecutionId::new(1),
            )
            .expect_err("an unpinned scope must be refused");
        assert_eq!(err, ExecutionError::InvalidScope);
        assert_eq!(
            sink.0, 0,
            "no evidence may be recorded from an unpinned execution"
        );
    }

    /// A backend that reports a kind other than the detector's PRODUCE.
    struct KindLiarBackend;
    impl DetectorBackend for KindLiarBackend {
        fn name(&self) -> &'static str {
            "kind-liar"
        }
        fn capabilities(&self) -> BTreeSet<AnalysisCapability> {
            [AnalysisCapability::AstPattern].into_iter().collect()
        }
        fn evidence_ceiling(&self) -> EvidenceClass {
            EvidenceClass::C
        }
        fn run(
            &self,
            _admitted: &AdmittedDetector,
            _input: &AnalysisInput,
        ) -> Result<DetectorOutcome, BackendError> {
            Ok(DetectorOutcome {
                produced_evidence: vec![],
                matches: vec![DetectorMatch {
                    kind: FindingKind::new("architecture.layer_violation").unwrap(),
                    message: "invented".to_string(),
                    evidence: vec![],
                    causal: vec![],
                }],
                diagnostics: vec![],
            })
        }
    }

    #[test]
    fn backend_cannot_invent_a_finding_kind() {
        let mut registry = BackendRegistry::new();
        registry.register(Box::new(KindLiarBackend));
        let mut requires = BTreeSet::new();
        requires.insert(AnalysisCapability::AstPattern);
        let permit =
            DetectorAdmission::admit(detector_with(requires), "1.0.0", AdmissionSource::Builtin)
                .unwrap();

        let executor = DetectorExecutor::new(&registry);
        let mut sink = CountingSink::default();
        let err = executor
            .execute(
                &permit,
                &AnalysisInput {
                    scope: Some(scope()),
                    ..Default::default()
                },
                &mut sink,
                ExecutionId::new(1),
            )
            .expect_err("invented kind must be rejected");
        match err {
            ExecutionError::BackendContract(BackendContractViolation::UnexpectedFindingKind {
                expected,
                produced,
                ..
            }) => {
                assert_eq!(expected.unwrap().as_str(), "security.weak_hash");
                assert_eq!(produced.as_str(), "architecture.layer_violation");
            }
            other => panic!("expected an unexpected-kind violation, got {other:?}"),
        }
    }
}
