//! Detector executor — the mandatory execution seam (M6, cycle e57).
//!
//! ```text
//! AdmittedDetector ──► DetectorExecutor ──► BackendRegistry::plan
//!                             │                    │
//!                             │                    ▼
//!                             │              DetectorBackend (e.g. AstBackend)
//!                             │                    │
//!                             │                    ▼
//!                             │              DetectorOutcome
//!                             ▼
//!                      EvidenceSink (assigns EvidenceId)
//!                             │
//!                             ▼
//!                     FindingAssembler ──► Finding
//! ```
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

use super::admission::AdmittedDetector;
use super::assembler::{AssemblyError, FindingAssembler};
use super::ast_backend::AstInput;
use super::detector_ir::{AnalysisCapability, DetectorExecutionRef, DetectorIrError};
use super::finding::{Finding, FindingGate};
use super::outcome::{DetectorDiagnostic, DetectorOutcome};
use super::ports::{EvidenceError, EvidenceSink};
use super::verifier::{FindingVerifier, VerificationError};
use crate::domain::kernel_ids::{EvidenceId, ExecutionId};

/// The shared input envelope handed to a backend.
///
/// Views are optional: a backend advertises the capabilities it provides and
/// fails loud ([`BackendError::MissingInput`]) when its view is absent.
#[derive(Debug, Clone, Default)]
pub struct AnalysisInput {
    /// AST/construct view (AstBackend).
    pub ast: Option<AstInput>,
}

/// A backend that executes a detector against an [`AnalysisInput`].
pub trait DetectorBackend {
    /// Stable backend name (used in the execution record).
    fn name(&self) -> &'static str;

    /// Capabilities this backend provides.
    fn capabilities(&self) -> BTreeSet<AnalysisCapability>;

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

    /// Execute an admitted detector.
    pub fn execute(
        &self,
        admitted: &AdmittedDetector,
        input: &AnalysisInput,
        sink: &mut dyn EvidenceSink,
        execution_id: ExecutionId,
    ) -> Result<ExecutionRecord, ExecutionError> {
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

        // Persist evidence first, so the assembler can assign real ids.
        let mut evidence = Vec::with_capacity(outcome.produced_evidence.len());
        for produced in &outcome.produced_evidence {
            let id = sink
                .record(produced.clone())
                .map_err(ExecutionError::Evidence)?;
            evidence.push(id);
        }

        let execution = admitted
            .execution_ref(Some(execution_id))
            .map_err(ExecutionError::InvalidDefinition)?;

        let findings = FindingAssembler::assemble(admitted, &execution, &outcome, &evidence)
            .map_err(ExecutionError::Assembly)?;

        Ok(ExecutionRecord {
            backend: backend.name().to_string(),
            execution,
            findings,
            evidence,
            diagnostics: outcome.diagnostics,
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
    /// Evidence ids assigned this run (index-aligned with produced evidence).
    pub evidence: Vec<EvidenceId>,
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
    /// An internal backend failure.
    Internal(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInput(view) => write!(f, "backend input `{view}` is missing"),
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

/// Why execution failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    /// The admitted definition failed validation.
    InvalidDefinition(DetectorIrError),
    /// No backend could satisfy the requirements.
    Plan(PlanError),
    /// A backend failed.
    Backend(BackendError),
    /// Persisting evidence failed.
    Evidence(EvidenceError),
    /// Assembling findings failed.
    Assembly(AssemblyError),
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDefinition(err) => write!(f, "invalid detector definition: {err}"),
            Self::Plan(err) => write!(f, "planning failed: {err}"),
            Self::Backend(err) => write!(f, "backend failed: {err}"),
            Self::Evidence(err) => write!(f, "{err}"),
            Self::Assembly(err) => write!(f, "assembly failed: {err}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::findings::ast_backend::AstBackend;

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
}
