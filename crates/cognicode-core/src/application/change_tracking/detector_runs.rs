//! Production detector runs over revision facts: the writer of read-sets
//! that [`assess_reuse`] consumes.
//!
//! ## What runs
//!
//! Three detectors, listed by [`ProductionDetector`], run through the real
//! seam: `DetectorAdmission` -> `ExecutionPermit` -> `DetectorExecutor` ->
//! `AstBackend`. Each projects its AST view from the revision's own facts,
//! so a run reads exactly the facts its detector is about and nothing else:
//!
//! - the function-definition detector (the IR the grounded finding flow
//!   already defines) reads the definitions of kind `Function`;
//! - the type-definition detector reads the definitions of kind `Struct`,
//!   `Class`, `Enum` and `Trait`;
//! - the unprotected-bucket detector reads the Terraform declarations of
//!   S3 buckets and of their public access blocks, and the references of
//!   those blocks. It is the one detector that also reads the revision's
//!   coverage: a Terraform module with a file that has no facts is not
//!   judged, because the file may hold the block (see `terraform_buckets`).
//!
//! The other detectors the platform ships read source text, not revision
//! facts, so a facts read-set recorded for them would claim something the
//! detector never did. They write none, and nothing here reuses them.
//!
//! ## What is recorded
//!
//! One [`DetectorRunRecord`] per run, on the existing
//! [`DetectorReuseStore`] under the detector's own id:
//!
//! - the **read-set**: the ids of the facts the projection handed the
//!   detector, never a guess about what it might have touched;
//! - the **effective capability**: the detector's `requires` intersected
//!   with what the backend the registry planned actually offers;
//! - the layered identity (rule, parser, sanitizer) and the findings, each
//!   passed through the same `project_finding` -> `to_guard_observation`
//!   seam every other consumer uses, so nothing is promoted on the way.
//!
//! ## Reuse never guesses
//!
//! [`detect_with_reuse`] reuses a prior record only when ALL hold: the
//! record exists and was produced on the base snapshot, its read-set was
//! not truncated, the identity layers (rule, capability, parser,
//! sanitizer) are unchanged, and the one planner proves the detector
//! `Unaffected` over complete facts. Anything else re-runs and says why.
//! Facts are numbered per snapshot, so a reused finding keeps naming the
//! snapshot that produced its evidence (`rebind_candidate`) instead of
//! pretending those ids belong to the candidate.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::application::findings::grounded_finding_flow::{
    SUBJECT_FUNCTION_DEFINITION, function_definition_detector_ir,
};
use crate::application::graph_cache::workspace_key;
use crate::application::services::detector_reuse_store::DetectorReuseStore;
use crate::domain::evidence_kernel::fact::{Fact, FactValue};
use crate::domain::evidence_kernel::ids::{FactId, SnapshotId};
use crate::domain::execution::{ActorRef, CorrelationId};
use crate::domain::findings::admission::{AdmissionSource, DetectorAdmission, ExecutionPermit};
use crate::domain::findings::ast_backend::{AstBackend, AstConstruct, AstInput, AstUnit};
use crate::domain::findings::detector_ir::{DetectorIr, DetectorStep, SubjectPattern};
use crate::domain::findings::detector_reuse::{
    CachedFinding, DETECTOR_READ_BOUNDARY_SCHEMA, DetectorReadBoundary, DetectorReuseIndex,
    DetectorRunIdentity, DetectorRunRecord, InvalidationLevel, OutputLocation, ReboundFinding,
    ReuseDecision, cache_observation, rebind_candidate, reuse_decision, semantic_fingerprint,
};
use crate::domain::findings::execution::{
    AnalysisInput, BackendRegistry, DetectorExecutor, ExecutionRequest,
};
use crate::domain::findings::grounding::GroundingRef;
use crate::domain::findings::guard_transfer::to_guard_observation;
use crate::domain::findings::ports::EvidenceSink;
use crate::domain::findings::projection::project_finding;
use crate::domain::findings::scope::AnalysisScope;
use crate::domain::findings::{
    AnalysisCapability, DetectorAuthority, DetectorDigest, DetectorId, FindingKind,
};
use crate::domain::kernel_ids::ExecutionId;
use crate::domain::scm::ChangeSet;
use crate::domain::value_objects::WorkspaceId;

use super::planner::WorkDisposition;
use super::revision_facts::{ProducedRevision, assess_reuse, producer_versions};
use super::scm_delta::RevisionFacts;
use super::terraform_buckets;

const DEFINES: &str = "core:defines";
const FUNCTION_KIND: &str = "kind=Function";
const TYPE_KINDS: [&str; 4] = ["kind=Struct", "kind=Class", "kind=Enum", "kind=Trait"];

const TYPE_DEFINITION_SUBJECT: &str = "syntax.type_definition";
const TYPE_DEFINITION_ID: &str = "structure.type_definition";

const UNPROTECTED_BUCKET_SUBJECT: &str = "infrastructure.s3_bucket";
const UNPROTECTED_BUCKET_ID: &str = "infrastructure.unprotected_s3_bucket";
const UNUSED_HELM_VALUE_SUBJECT: &str = "configuration.unused_helm_value";
const UNUSED_HELM_VALUE_ID: &str = "configuration.unused_helm_value";

/// A detector that reads revision facts and records what it read.
///
/// Each one is its own detector: its own id, version, capability request
/// and read-set. Only what differs between them lives here: the IR, which
/// definitions the projection hands over, and how a construct is worded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProductionDetector {
    /// Every function definition.
    FunctionDefinition,
    /// Every struct, class, enum and trait definition.
    TypeDefinition,
    /// Every Terraform `aws_s3_bucket` that no public access block of its
    /// module references. Reads Terraform declaration and reference facts,
    /// not definitions of code.
    UnprotectedBucket,
    /// Declared chart values no template references in a complete static chart.
    UnusedHelmValue,
}

impl ProductionDetector {
    /// Every detector that writes a read-set, in the order they are recorded.
    pub const ALL: [Self; 4] = [
        Self::FunctionDefinition,
        Self::TypeDefinition,
        Self::UnprotectedBucket,
        Self::UnusedHelmValue,
    ];

    fn ir(self) -> Result<DetectorIr, String> {
        match self {
            Self::FunctionDefinition => function_definition_detector_ir(),
            Self::TypeDefinition => type_definition_detector_ir(),
            Self::UnprotectedBucket => unprotected_bucket_detector_ir(),
            Self::UnusedHelmValue => unused_helm_value_detector_ir(),
        }
    }

    fn version(self) -> &'static str {
        match self {
            Self::FunctionDefinition => "cgh-044-1.0.0",
            Self::TypeDefinition => "1.0.0",
            Self::UnprotectedBucket => "1.0.0",
            Self::UnusedHelmValue => "1.0.0",
        }
    }

    fn subject(self) -> &'static str {
        match self {
            Self::FunctionDefinition => SUBJECT_FUNCTION_DEFINITION,
            Self::TypeDefinition => TYPE_DEFINITION_SUBJECT,
            Self::UnprotectedBucket => UNPROTECTED_BUCKET_SUBJECT,
            Self::UnusedHelmValue => UNUSED_HELM_VALUE_SUBJECT,
        }
    }

    fn noun(self) -> &'static str {
        match self {
            Self::FunctionDefinition => "function",
            Self::TypeDefinition => "type",
            Self::UnprotectedBucket => "S3 bucket",
            Self::UnusedHelmValue => "Helm value",
        }
    }

    /// Whether a `core:defines` fact with this kind detail is read.
    fn reads_kind(self, detail: Option<&str>) -> bool {
        match self {
            Self::FunctionDefinition => detail == Some(FUNCTION_KIND),
            Self::TypeDefinition => detail.is_some_and(|kind| TYPE_KINDS.contains(&kind)),
            // Not a definition of code: its projection reads Terraform facts.
            Self::UnprotectedBucket | Self::UnusedHelmValue => false,
        }
    }
}

fn type_definition_detector_ir() -> Result<DetectorIr, String> {
    Ok(DetectorIr {
        id: DetectorId::new(TYPE_DEFINITION_ID).map_err(|e| e.to_string())?,
        name: TYPE_DEFINITION_ID.to_string(),
        policy: Default::default(),
        requires: [AnalysisCapability::AstPattern].into_iter().collect(),
        authority: DetectorAuthority::Candidate,
        steps: vec![
            DetectorStep::Match {
                subject: SubjectPattern::new(TYPE_DEFINITION_SUBJECT).map_err(|e| e.to_string())?,
            },
            DetectorStep::Produce {
                kind: FindingKind::new(TYPE_DEFINITION_ID).map_err(|e| e.to_string())?,
            },
        ],
    })
}

fn unprotected_bucket_detector_ir() -> Result<DetectorIr, String> {
    Ok(DetectorIr {
        id: DetectorId::new(UNPROTECTED_BUCKET_ID).map_err(|e| e.to_string())?,
        name: UNPROTECTED_BUCKET_ID.to_string(),
        policy: Default::default(),
        requires: [AnalysisCapability::AstPattern].into_iter().collect(),
        authority: DetectorAuthority::Candidate,
        steps: vec![
            DetectorStep::Match {
                subject: SubjectPattern::new(UNPROTECTED_BUCKET_SUBJECT)
                    .map_err(|e| e.to_string())?,
            },
            DetectorStep::Produce {
                kind: FindingKind::new(UNPROTECTED_BUCKET_ID).map_err(|e| e.to_string())?,
            },
        ],
    })
}

fn unused_helm_value_detector_ir() -> Result<DetectorIr, String> {
    Ok(DetectorIr {
        id: DetectorId::new(UNUSED_HELM_VALUE_ID).map_err(|error| error.to_string())?,
        name: UNUSED_HELM_VALUE_ID.to_string(),
        policy: Default::default(),
        requires: [AnalysisCapability::AstPattern].into_iter().collect(),
        authority: DetectorAuthority::Candidate,
        steps: vec![
            DetectorStep::Match {
                subject: SubjectPattern::new(UNUSED_HELM_VALUE_SUBJECT)
                    .map_err(|error| error.to_string())?,
            },
            DetectorStep::Produce {
                kind: FindingKind::new(UNUSED_HELM_VALUE_ID).map_err(|error| error.to_string())?,
            },
        ],
    })
}

/// Why a detector run could not be produced.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum DetectorRunError {
    /// The detector definition or its admission failed.
    #[error("detector admission failed: {0}")]
    Admission(String),
    /// The executor refused or failed the run.
    #[error("detector execution failed: {0}")]
    Execution(String),
    /// The facts and the findings disagree about what was matched.
    #[error("the detector produced {findings} finding(s) for {constructs} construct(s)")]
    Arity { findings: usize, constructs: usize },
    /// The reuse plan could not be computed.
    #[error("the reuse plan could not be computed: {0}")]
    Plan(String),
    /// Persisting the record failed.
    #[error("the detector run could not be persisted: {0}")]
    Persist(String),
}

/// One definition fact a detector reads.
struct DefinitionRead {
    path: String,
    fqn: String,
    line: u32,
    fact: FactId,
    construct: AstConstruct,
}

/// The pure projection of a revision's facts into what `detector` reads:
/// the `core:defines` facts of the kinds it is about, each grounded on its
/// own fact.
///
/// Units follow path order and constructs follow fact order, so the same
/// facts always project to the same view.
fn definition_reads(
    detector: ProductionDetector,
    facts: &RevisionFacts,
) -> Result<Vec<DefinitionRead>, DetectorRunError> {
    let subject = SubjectPattern::new(detector.subject())
        .map_err(|error| DetectorRunError::Admission(error.to_string()))?;
    let by_id: BTreeMap<FactId, &Fact> = facts.facts.iter().map(|f| (f.id, f)).collect();
    let mut reads = Vec::new();
    for (path, ids) in &facts.path_facts {
        for id in ids {
            let Some(fact) = by_id.get(id) else { continue };
            let FactValue::Text(fqn) = &fact.object else {
                continue;
            };
            if fact.predicate.as_str() != DEFINES
                || !detector.reads_kind(fact.provenance.detail.as_deref())
            {
                continue;
            }
            // The extractor names a symbol `<path>:<name>:<line>`.
            let line = fqn
                .rsplit(':')
                .next()
                .and_then(|tail| tail.parse::<u32>().ok())
                .unwrap_or(0);
            reads.push(DefinitionRead {
                path: path.as_str().to_string(),
                fqn: fqn.clone(),
                line,
                fact: fact.id,
                construct: AstConstruct {
                    subject: subject.clone(),
                    line,
                    detail: format!("{} `{fqn}`", detector.noun()),
                    grounding: Some(GroundingRef::entity(fact.subject, fact.id)),
                },
            });
        }
    }
    Ok(reads)
}

/// What one detector reads of one revision.
struct Projection {
    /// The constructs handed to the detector, one per possible finding.
    reads: Vec<DefinitionRead>,
    /// Every fact consulted to decide them: the read-set the record keeps.
    /// For a detector whose answer depends on more than the facts it
    /// reports on, this is wider than `reads`.
    observed: BTreeSet<FactId>,
}

/// The pure projection of a revision into what `detector` reads.
fn projection(
    detector: ProductionDetector,
    produced: &ProducedRevision,
) -> Result<Projection, DetectorRunError> {
    match detector {
        ProductionDetector::FunctionDefinition | ProductionDetector::TypeDefinition => {
            let reads = definition_reads(detector, &produced.facts)?;
            let observed = reads.iter().map(|read| read.fact).collect();
            Ok(Projection { reads, observed })
        }
        ProductionDetector::UnprotectedBucket => bucket_projection(detector, produced),
        ProductionDetector::UnusedHelmValue => helm_value_projection(detector, produced),
    }
}

fn helm_value_projection(
    detector: ProductionDetector,
    produced: &ProducedRevision,
) -> Result<Projection, DetectorRunError> {
    const CHART_COMPLETE: &str = "helm:analysis_complete";
    const TEMPLATE_COMPLETE: &str = "helm:template_analysis_complete";
    const DECLARED: &str = "helm:declared_value";
    const VALUE_REF: &str = "helm:template_value_ref";

    let subject = SubjectPattern::new(detector.subject())
        .map_err(|error| DetectorRunError::Admission(error.to_string()))?;
    let paths: BTreeMap<FactId, String> = produced
        .facts
        .path_facts
        .iter()
        .flat_map(|(path, ids)| ids.iter().map(move |id| (*id, path.as_str().to_string())))
        .collect();
    let mut charts: BTreeMap<_, Vec<&Fact>> = BTreeMap::new();
    let mut observed = BTreeSet::new();
    for fact in &produced.facts.facts {
        if matches!(
            fact.predicate.as_str(),
            CHART_COMPLETE | TEMPLATE_COMPLETE | DECLARED | VALUE_REF
        ) {
            observed.insert(fact.id);
            charts.entry(fact.subject).or_default().push(fact);
        }
    }

    let mut reads = Vec::new();
    for (chart_subject, facts) in charts {
        let chart_complete = facts.iter().any(|fact| {
            fact.predicate.as_str() == CHART_COMPLETE
                && matches!(&fact.object, FactValue::Text(value) if value == "true")
        });
        let templates_complete = facts
            .iter()
            .filter(|fact| fact.predicate.as_str() == TEMPLATE_COMPLETE)
            .all(|fact| matches!(&fact.object, FactValue::Text(value) if value.ends_with(":true")));
        if !chart_complete || !templates_complete {
            continue;
        }
        let references: BTreeSet<&str> = facts
            .iter()
            .filter(|fact| fact.predicate.as_str() == VALUE_REF)
            .filter_map(|fact| match &fact.object {
                FactValue::Text(value) => value.split_once('@').map(|(name, _)| name),
                _ => None,
            })
            .collect();
        for declaration in facts
            .iter()
            .filter(|fact| fact.predicate.as_str() == DECLARED)
        {
            let FactValue::Text(value) = &declaration.object else {
                continue;
            };
            let used = references.iter().any(|reference| {
                *reference == value
                    || reference.starts_with(&format!("{value}."))
                    || value.starts_with(&format!("{reference}."))
            });
            if used {
                continue;
            }
            let path = paths
                .get(&declaration.id)
                .cloned()
                .unwrap_or_else(|| "Chart.yaml".to_string());
            reads.push(DefinitionRead {
                path,
                fqn: format!("{}:{value}", chart_subject.0),
                line: 0,
                fact: declaration.id,
                construct: AstConstruct {
                    subject: subject.clone(),
                    line: 0,
                    detail: format!("declared Helm value `.Values.{value}` is unused"),
                    grounding: Some(GroundingRef::entity(declaration.subject, declaration.id)),
                },
            });
        }
    }
    Ok(Projection { reads, observed })
}

/// The unprotected buckets of the revision's Terraform facts, each grounded
/// on the bucket's own declaration fact.
fn bucket_projection(
    detector: ProductionDetector,
    produced: &ProducedRevision,
) -> Result<Projection, DetectorRunError> {
    let subject = SubjectPattern::new(detector.subject())
        .map_err(|error| DetectorRunError::Admission(error.to_string()))?;
    let by_id: BTreeMap<FactId, &Fact> = produced.facts.facts.iter().map(|f| (f.id, f)).collect();
    let scan = terraform_buckets::scan(&produced.facts, &produced.coverage);
    let reads = scan
        .unprotected
        .into_iter()
        .filter_map(|bucket| {
            let fact = by_id.get(&bucket.fact)?;
            Some(DefinitionRead {
                path: bucket.path,
                line: 0,
                construct: AstConstruct {
                    subject: subject.clone(),
                    // The declaration fact names a file and an identity, not
                    // a line.
                    line: 0,
                    detail: format!(
                        "{} `{}` has no public access block in its module",
                        detector.noun(),
                        bucket.fqn
                    ),
                    grounding: Some(GroundingRef::entity(fact.subject, fact.id)),
                },
                fqn: bucket.fqn,
                fact: bucket.fact,
            })
        })
        .collect();
    Ok(Projection {
        reads,
        observed: scan.observed,
    })
}

fn ast_view(reads: &[DefinitionRead]) -> AstInput {
    let mut units: BTreeMap<&str, Vec<AstConstruct>> = BTreeMap::new();
    for read in reads {
        units
            .entry(read.path.as_str())
            .or_default()
            .push(read.construct.clone());
    }
    AstInput {
        units: units
            .into_iter()
            .map(|(path, constructs)| AstUnit {
                path: path.to_string(),
                constructs,
            })
            .collect(),
    }
}

fn registry() -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    registry.register(Box::new(AstBackend));
    registry
}

fn permit(detector: ProductionDetector) -> Result<ExecutionPermit, DetectorRunError> {
    let ir = detector.ir().map_err(DetectorRunError::Admission)?;
    DetectorAdmission::admit(ir, detector.version(), AdmissionSource::Builtin)
        .map_err(|error| DetectorRunError::Admission(error.to_string()))
}

/// What the planned backend actually offers of what the detector requires.
fn effective_capabilities(
    permit: &ExecutionPermit,
    registry: &BackendRegistry,
) -> Result<BTreeSet<AnalysisCapability>, DetectorRunError> {
    let requires = &permit.admitted().definition.requires;
    let backend = registry
        .plan(requires)
        .map_err(|error| DetectorRunError::Execution(error.to_string()))?;
    let offered = backend.capabilities();
    Ok(requires.intersection(&offered).copied().collect())
}

fn identity_of(
    permit: &ExecutionPermit,
    registry: &BackendRegistry,
) -> Result<DetectorRunIdentity, DetectorRunError> {
    let reference = permit
        .execution_ref(None)
        .map_err(|error| DetectorRunError::Admission(error.to_string()))?;
    Ok(DetectorRunIdentity {
        detector_id: reference.id,
        version: reference.version,
        authority: reference.authority_at_execution,
        digests: reference.digests,
        effective_capabilities: effective_capabilities(permit, registry)?,
        // The parse/extraction layer is the facts producer.
        parser: DetectorDigest::from_content(&producer_versions().facts),
        // Nothing normalizes the facts between producer and detector.
        sanitizer: DetectorDigest::from_content("none"),
    })
}

/// The identity a run of `detector` executed now would carry.
///
/// # Errors
/// The detector could not be admitted or no backend serves it.
pub fn current_identity(
    detector: ProductionDetector,
) -> Result<DetectorRunIdentity, DetectorRunError> {
    identity_of(&permit(detector)?, &registry())
}

/// The id under which `detector`'s records are kept.
///
/// # Errors
/// The detector definition is invalid.
pub fn detector_id(detector: ProductionDetector) -> Result<DetectorId, DetectorRunError> {
    Ok(current_identity(detector)?.detector_id)
}

/// Runs `detector` over `facts` through the real executor and records what
/// it read and under which capability.
///
/// `sink` receives the run's evidence. Evidence ids are allocated by the
/// sink, so a caller that wants reproducible ids passes a fresh one per run.
///
/// # Errors
/// Admission, execution or finding/construct arity failed. A failed run
/// records nothing: there is no half-written read-set.
pub fn run_detector(
    detector: ProductionDetector,
    produced: &ProducedRevision,
    workspace: &WorkspaceId,
    sink: &mut dyn EvidenceSink,
) -> Result<DetectorRunRecord, DetectorRunError> {
    let permit = permit(detector)?;
    let registry = registry();
    let identity = identity_of(&permit, &registry)?;
    let Projection { reads, observed } = projection(detector, produced)?;
    let facts = &produced.facts;

    let scope = AnalysisScope::new(workspace.clone(), facts.snapshot);
    let input = AnalysisInput {
        scope: Some(scope.clone()),
        ast: Some(ast_view(&reads)),
        graph: None,
        dataflow: None,
    };
    let correlation = CorrelationId::new("cgh-044-detector-run")
        .map_err(|error| DetectorRunError::Execution(error.to_string()))?;
    let request = ExecutionRequest::new(
        ExecutionId::new(1),
        scope.clone(),
        ActorRef::kernel(),
        correlation,
    );
    let executed = DetectorExecutor::new(&registry)
        .execute(&permit, &input, sink, request)
        .map_err(|error| DetectorRunError::Execution(error.to_string()))?;

    // The backend emits one match per construct, in construct order. The
    // AST view lists constructs by unit, so the matches follow the same
    // order and the pairing below is positional.
    let in_view: Vec<&DefinitionRead> = {
        let mut ordered: Vec<&DefinitionRead> = reads.iter().collect();
        ordered.sort_by(|a, b| a.path.cmp(&b.path));
        ordered
    };
    if executed.findings.len() != in_view.len() {
        return Err(DetectorRunError::Arity {
            findings: executed.findings.len(),
            constructs: in_view.len(),
        });
    }
    let findings = executed
        .findings
        .iter()
        .zip(&in_view)
        .map(|(finding, read)| {
            let projection = project_finding(finding, &facts.snapshot);
            CachedFinding {
                semantic_fingerprint: semantic_fingerprint(
                    &identity.digests.semantic,
                    &read.fqn,
                    finding.kind.as_str(),
                ),
                output_locations: vec![OutputLocation {
                    file: read.path.clone(),
                    line: read.line,
                    // The extractor names a line, not a column.
                    column: 0,
                }],
                observation: cache_observation(&to_guard_observation(&projection)),
            }
        })
        .collect();

    Ok(DetectorRunRecord {
        identity,
        read_boundary: projection_boundary(detector, produced, facts.snapshot),
        observed_facts: observed,
        observed_truncated: false,
        produced_snapshot: facts.snapshot,
        findings,
    })
}

/// What [`ensure_run`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunWritten {
    /// A current record for this snapshot already existed.
    AlreadyRecorded,
    /// The detector ran and its record was written.
    Written,
}

/// Makes sure the store holds a current run of `detector` for `produced`:
/// keeps the existing record when it is for this very snapshot under the
/// identity a run would carry now, and runs and writes otherwise. The
/// records kept for other snapshots are left as they were. `sink` is only
/// used when a run happens.
///
/// # Errors
/// The run or the write failed; the previous record is left as it was.
pub fn ensure_run(
    detector: ProductionDetector,
    store: &DetectorReuseStore,
    produced: &ProducedRevision,
    workspace: &WorkspaceId,
    sink: &mut dyn EvidenceSink,
) -> Result<RunWritten, DetectorRunError> {
    let identity = current_identity(detector)?;
    let current = store.load().is_some_and(|index| {
        index
            .record_for(identity.detector_id.as_str(), produced.facts.snapshot)
            .is_some_and(|record| record.identity == identity && !record.observed_truncated)
    });
    if current {
        return Ok(RunWritten::AlreadyRecorded);
    }
    let record = run_detector(detector, produced, workspace, sink)?;
    store
        .record(record)
        .map_err(|error| DetectorRunError::Persist(error.to_string()))?;
    Ok(RunWritten::Written)
}

/// [`ensure_run`] for every [`ProductionDetector`] against the store
/// `cache_root` keeps for the repository at `repository`: the one entry point
/// the goals use to write read-sets.
///
/// Each detector is written on its own: one that fails leaves its previous
/// record as it was and does not stop the others.
pub fn ensure_runs_in_cache(
    cache_root: &Path,
    repository: &Path,
    produced: &ProducedRevision,
    sink: &mut dyn EvidenceSink,
) -> Vec<(ProductionDetector, Result<RunWritten, DetectorRunError>)> {
    let workspace = WorkspaceId::try_new(workspace_key(repository))
        .map_err(|error| DetectorRunError::Admission(error.to_string()));
    let store = DetectorReuseStore::for_workspace(cache_root, repository);
    ProductionDetector::ALL
        .into_iter()
        .map(|detector| {
            let written = workspace
                .clone()
                .and_then(|workspace| ensure_run(detector, &store, produced, &workspace, sink));
            (detector, written)
        })
        .collect()
}

/// [`detect_with_reuse`] of `detector` over the read-set the store under
/// `cache_root` keeps for the base revision of `repository`: the one entry
/// point the goals use to publish the head findings.
///
/// Without a cache, or without a record for this base, there is no prior
/// read-set and the detector runs again.
///
/// # Errors
/// The plan could not be computed or the re-run failed.
pub fn detect_in_cache(
    detector: ProductionDetector,
    cache_root: Option<&Path>,
    repository: &Path,
    base: &ProducedRevision,
    head: &ProducedRevision,
    changes: &ChangeSet,
    sink: &mut dyn EvidenceSink,
) -> Result<Detection, DetectorRunError> {
    let workspace = WorkspaceId::try_new(workspace_key(repository))
        .map_err(|error| DetectorRunError::Admission(error.to_string()))?;
    let id = detector_id(detector)?;
    let index =
        cache_root.and_then(|root| DetectorReuseStore::for_workspace(root, repository).load());
    let prior = index
        .as_ref()
        .and_then(|index| index.record_for(id.as_str(), base.facts.snapshot));
    detect_with_reuse(detector, prior, base, head, changes, &workspace, sink)
}

/// Whether the findings of a detection came from a prior run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reuse {
    /// Proven unaffected: the findings are the prior run's, re-referenced.
    Reused { produced_in: SnapshotId },
    /// The detector ran again, for this reason.
    Rerun { reason: String },
}

/// The findings of the detector on the head revision, and how they were got.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detection {
    pub findings: Vec<ReboundFinding>,
    pub reuse: Reuse,
    /// The record of the fresh run, when there was one.
    pub fresh: Option<DetectorRunRecord>,
}

fn rerun(
    detector: ProductionDetector,
    head: &ProducedRevision,
    workspace: &WorkspaceId,
    sink: &mut dyn EvidenceSink,
    reason: impl Into<String>,
) -> Result<Detection, DetectorRunError> {
    let record = run_detector(detector, head, workspace, sink)?;
    let findings = record
        .findings
        .iter()
        .map(|finding| ReboundFinding {
            produced_in: head.facts.snapshot,
            reported_in: head.facts.snapshot,
            semantic_fingerprint: finding.semantic_fingerprint.clone(),
            output_locations: finding.output_locations.clone(),
            observation: finding.observation.clone(),
        })
        .collect();
    Ok(Detection {
        findings,
        reuse: Reuse::Rerun {
            reason: reason.into(),
        },
        fresh: Some(record),
    })
}

fn levels_word(levels: &[InvalidationLevel]) -> String {
    levels
        .iter()
        .map(|level| format!("{level:?}").to_lowercase())
        .collect::<Vec<_>>()
        .join(", ")
}

fn projection_boundary(
    detector: ProductionDetector,
    produced: &ProducedRevision,
    snapshot: SnapshotId,
) -> Option<DetectorReadBoundary> {
    let details: BTreeSet<String> = match detector {
        ProductionDetector::FunctionDefinition => &[FUNCTION_KIND][..],
        ProductionDetector::TypeDefinition => &TYPE_KINDS[..],
        ProductionDetector::UnprotectedBucket => return None,
    }
    .iter()
    .map(|detail| (*detail).to_string())
    .collect();
    let identity = current_identity(detector).ok()?;
    Some(DetectorReadBoundary {
        schema: DETECTOR_READ_BOUNDARY_SCHEMA.to_string(),
        detector_id: identity.detector_id.clone(),
        identity,
        snapshot,
        predicates: [DEFINES.to_string()].into_iter().collect(),
        details,
        complete: produced.coverage.is_complete(),
    })
}

/// `detector`'s findings on `head`, reusing `prior` when it is proven safe
/// and running the detector again otherwise.
///
/// `prior` is the record of a run on `base`. Reuse is decided by layers
/// (`reuse_decision`) and by the one planner (`assess_reuse`); a missing,
/// foreign-snapshot, truncated or stale record re-runs, and so does any
/// verdict short of `Unaffected`.
///
/// # Errors
/// The plan could not be computed or the re-run failed.
pub fn detect_with_reuse(
    detector: ProductionDetector,
    prior: Option<&DetectorRunRecord>,
    base: &ProducedRevision,
    head: &ProducedRevision,
    changes: &ChangeSet,
    workspace: &WorkspaceId,
    sink: &mut dyn EvidenceSink,
) -> Result<Detection, DetectorRunError> {
    let Some(prior) = prior else {
        return rerun(
            detector,
            head,
            workspace,
            sink,
            "no prior read-set is recorded",
        );
    };
    if prior.produced_snapshot != base.facts.snapshot {
        return rerun(
            detector,
            head,
            workspace,
            sink,
            "the prior read-set was not recorded against the base revision",
        );
    }
    if prior.observed_truncated {
        return rerun(
            detector,
            head,
            workspace,
            sink,
            "the prior read-set is truncated",
        );
    }
    let current = current_identity(detector)?;
    if let ReuseDecision::Invalidated { levels } = reuse_decision(prior, &current, None) {
        return rerun(
            detector,
            head,
            workspace,
            sink,
            format!("invalidated layer(s): {}", levels_word(&levels)),
        );
    }

    let mut index = DetectorReuseIndex::empty();
    index.remember(prior.clone());
    let assessment = assess_reuse(base, head, changes, Some(&index))
        .map_err(|error| DetectorRunError::Plan(error.to_string()))?;
    let work = prior.identity.detector_id.as_str();
    let verdict = assessment
        .works
        .iter()
        .find(|verdict| verdict.work == work)
        .map(|verdict| verdict.disposition);
    match verdict {
        Some(WorkDisposition::Unaffected) => Ok(Detection {
            findings: rebind_candidate(prior, head.facts.snapshot),
            reuse: Reuse::Reused {
                produced_in: prior.produced_snapshot,
            },
            fresh: None,
        }),
        Some(WorkDisposition::Affected) => rerun(
            detector,
            head,
            workspace,
            sink,
            "a change touches a fact the detector read",
        ),
        Some(WorkDisposition::Unknown) | None => rerun(
            detector,
            head,
            workspace,
            sink,
            format!("reuse is not proven: {}", assessment.reason),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::change_tracking::revision_facts::{
        FactsLimits, ReuseStatus, produce_revision_facts,
    };
    use crate::domain::findings::binding::{EvidenceBinding, EvidenceBindings, GroundingFailure};
    use crate::domain::findings::detector_reuse::CachedObservation;
    use crate::domain::findings::outcome::ProducedEvidence;
    use crate::domain::findings::ports::EvidenceError;
    use crate::domain::kernel_ids::EvidenceId;
    use crate::domain::scm::fake::FakeSource;
    use crate::domain::scm::{
        ChangeReader, ProviderCapabilities, RevisionReader, SourceRef, SourceRevision,
    };

    const ALPHA_V1: &[u8] = b"pub fn alpha() { first(); }\n";
    const ALPHA_BODY_EDIT: &[u8] = b"pub fn alpha() { second(); }\n";
    const ALPHA_RENAMED: &[u8] = b"pub fn alpha_two() { first(); }\n";
    const BETA: &[u8] = b"pub fn beta() { helper(); }\n";
    const GAMMA: &[u8] = b"pub fn gamma() {}\n";
    const POINT: &[u8] = b"pub struct Point { x: i32 }\n";
    const POINT_RENAMED: &[u8] = b"pub struct Shape { x: i32 }\n";
    const NOTHING: &[u8] = b"// nothing defined here\n";

    /// Binds each item to the fact it names, with ids numbered from one, as
    /// the production sink does; a run only reads the bindings back.
    #[derive(Default)]
    struct TestSink {
        recorded: u64,
    }

    impl EvidenceSink for TestSink {
        fn persist(
            &mut self,
            produced: &[ProducedEvidence],
        ) -> Result<EvidenceBindings, EvidenceError> {
            let entries = produced
                .iter()
                .map(|item| {
                    self.recorded += 1;
                    match item.grounding {
                        Some(grounding) => EvidenceBinding::grounded(
                            EvidenceId::new(self.recorded),
                            grounding.fact,
                        ),
                        None => EvidenceBinding::ungrounded(GroundingFailure::NoFact),
                    }
                })
                .collect();
            Ok(EvidenceBindings::new(entries))
        }
    }

    const FUNCTIONS: ProductionDetector = ProductionDetector::FunctionDefinition;
    const TYPES: ProductionDetector = ProductionDetector::TypeDefinition;
    const HELM_VALUES: ProductionDetector = ProductionDetector::UnusedHelmValue;

    fn fresh_run(
        facts: &ProducedRevision,
        workspace: &WorkspaceId,
    ) -> Result<DetectorRunRecord, DetectorRunError> {
        run_detector(FUNCTIONS, facts, workspace, &mut TestSink::default())
    }

    fn fresh_run_of(
        detector: ProductionDetector,
        facts: &ProducedRevision,
    ) -> Result<DetectorRunRecord, DetectorRunError> {
        run_detector(detector, facts, &workspace(), &mut TestSink::default())
    }

    fn fresh_ensure(
        store: &DetectorReuseStore,
        produced: &ProducedRevision,
        workspace: &WorkspaceId,
    ) -> Result<RunWritten, DetectorRunError> {
        ensure_run(
            FUNCTIONS,
            store,
            produced,
            workspace,
            &mut TestSink::default(),
        )
    }

    fn fresh_detect_of(
        detector: ProductionDetector,
        prior: Option<&DetectorRunRecord>,
        base: &ProducedRevision,
        head: &ProducedRevision,
        changes: &ChangeSet,
    ) -> Result<Detection, DetectorRunError> {
        detect_with_reuse(
            detector,
            prior,
            base,
            head,
            changes,
            &workspace(),
            &mut TestSink::default(),
        )
    }

    fn fresh_detect(
        prior: Option<&DetectorRunRecord>,
        base: &ProducedRevision,
        head: &ProducedRevision,
        changes: &ChangeSet,
        workspace: &WorkspaceId,
    ) -> Result<Detection, DetectorRunError> {
        detect_with_reuse(
            FUNCTIONS,
            prior,
            base,
            head,
            changes,
            workspace,
            &mut TestSink::default(),
        )
    }

    fn fresh_rerun(head: &ProducedRevision, reason: &str) -> Result<Detection, DetectorRunError> {
        fresh_rerun_of(FUNCTIONS, head, reason)
    }

    fn fresh_rerun_of(
        detector: ProductionDetector,
        head: &ProducedRevision,
        reason: &str,
    ) -> Result<Detection, DetectorRunError> {
        rerun(
            detector,
            head,
            &workspace(),
            &mut TestSink::default(),
            reason,
        )
    }

    fn workspace() -> WorkspaceId {
        WorkspaceId::try_new("ws").expect("workspace")
    }

    fn helm_fake(template: &[u8]) -> FakeSource {
        FakeSource::new(
            "fake",
            ProviderCapabilities {
                ancestor_queries: true,
                ..ProviderCapabilities::NONE
            },
        )
        .with_revision(
            "main",
            &[],
            &[
                (
                    "Chart.yaml",
                    b"apiVersion: v2\nname: demo\nversion: 1.0.0\n",
                ),
                ("values.yaml", b"used: true\nunused: true\n"),
                ("templates/deployment.yaml", template),
            ],
        )
    }

    fn helm_parent_with_subchart_fake() -> FakeSource {
        FakeSource::new(
            "fake",
            ProviderCapabilities {
                ancestor_queries: true,
                ..ProviderCapabilities::NONE
            },
        )
        .with_revision(
            "main",
            &[],
            &[
                (
                    "Chart.yaml",
                    b"apiVersion: v2\nname: parent\nversion: 1.0.0\ndependencies:\n  - name: db\n    version: 1.0.0\n    repository: file://charts/db\n",
                ),
                ("values.yaml", b"db:\n  password: parent-override\n"),
                (
                    "charts/db/Chart.yaml",
                    b"apiVersion: v2\nname: db\nversion: 1.0.0\n",
                ),
                ("charts/db/values.yaml", b"password: child-default\nunusedChild: true\n"),
                (
                    "charts/db/templates/secret.yaml",
                    b"data:\n  password: {{ .Values.password }}\n",
                ),
            ],
        )
    }

    #[test]
    fn a_parent_with_subchart_values_abstains_without_disabling_child_analysis() {
        let fake = helm_parent_with_subchart_fake();
        let produced = produce(&fake, "main");
        let marker_value = |path: &str| {
            let ids = produced
                .facts
                .path_facts
                .get(&crate::domain::scm::PathKey::new(path).expect("path"))
                .expect("chart marker facts");
            ids.iter()
                .filter_map(|id| produced.facts.facts.iter().find(|fact| fact.id == *id))
                .find(|fact| fact.predicate.as_str() == "helm:analysis_complete")
                .and_then(|fact| match &fact.object {
                    FactValue::Text(value) => Some(value.as_str()),
                    _ => None,
                })
                .expect("analysis marker")
        };
        assert_eq!(marker_value("Chart.yaml"), "false");
        assert_eq!(marker_value("charts/db/Chart.yaml"), "true");

        let record = fresh_run_of(HELM_VALUES, &produced).expect("detector run");

        assert_eq!(record.findings.len(), 1, "{:#?}", record.findings);
        assert_eq!(
            record.findings[0].output_locations[0].file,
            "charts/db/values.yaml"
        );
    }

    #[test]
    fn unused_helm_value_finding_reads_every_chart_proof_fact() {
        let fake = helm_fake(b"{{ .Values.used }}\n");
        let produced = produce(&fake, "main");
        let record = fresh_run_of(HELM_VALUES, &produced).expect("detector run");
        let expected: BTreeSet<FactId> = produced
            .facts
            .facts
            .iter()
            .filter(|fact| {
                matches!(
                    fact.predicate.as_str(),
                    "helm:analysis_complete"
                        | "helm:template_analysis_complete"
                        | "helm:declared_value"
                        | "helm:template_value_ref"
                )
            })
            .map(|fact| fact.id)
            .collect();

        assert_eq!(record.findings.len(), 1);
        assert_eq!(record.observed_facts, expected);
        assert!(!record.observed_truncated);
        let path = record.findings[0].output_locations[0].file.as_str();
        assert_eq!(path, "values.yaml");
    }

    #[test]
    fn unused_helm_value_detector_abstains_on_dynamic_or_incomplete_templates() {
        for template in [
            &b"{{ include \\\"helper\\\" . }}"[..],
            &b"{{ tpl .Values.source . }}"[..],
            &b"{{ toYaml .Values }}"[..],
            &b"{{ .Values.unterminated"[..],
        ] {
            let fake = helm_fake(template);
            let produced = produce(&fake, "main");
            let record = fresh_run_of(HELM_VALUES, &produced).expect("detector run");
            assert!(record.findings.is_empty(), "{template:?}");
            assert!(produced.coverage.gaps.iter().any(|gap| matches!(
                gap.reason,
                super::super::revision_facts::GapReason::TemplateNotRendered { .. }
            )));
        }
    }

    fn fake() -> FakeSource {
        let base: &[(&str, &[u8])] = &[
            ("src/a.rs", ALPHA_V1),
            ("src/b.rs", BETA),
            ("src/t.rs", POINT),
        ];
        let body_edit: &[(&str, &[u8])] = &[
            ("src/a.rs", ALPHA_BODY_EDIT),
            ("src/b.rs", BETA),
            ("src/t.rs", POINT),
        ];
        let renamed: &[(&str, &[u8])] = &[
            ("src/a.rs", ALPHA_RENAMED),
            ("src/b.rs", BETA),
            ("src/t.rs", POINT),
        ];
        let function_removed: &[(&str, &[u8])] = &[
            ("src/a.rs", NOTHING),
            ("src/b.rs", BETA),
            ("src/t.rs", POINT),
        ];
        let type_removed: &[(&str, &[u8])] = &[
            ("src/a.rs", ALPHA_V1),
            ("src/b.rs", BETA),
            ("src/t.rs", NOTHING),
        ];
        let type_renamed: &[(&str, &[u8])] = &[
            ("src/a.rs", ALPHA_V1),
            ("src/b.rs", BETA),
            ("src/t.rs", POINT_RENAMED),
        ];
        let added: &[(&str, &[u8])] = &[
            ("src/a.rs", ALPHA_V1),
            ("src/b.rs", BETA),
            ("src/t.rs", POINT),
            ("src/c.rs", GAMMA),
        ];
        FakeSource::new(
            "fake",
            ProviderCapabilities {
                ancestor_queries: true,
                ..ProviderCapabilities::NONE
            },
        )
        .with_revision("main", &[], base)
        .with_revision("body-edit", &["main"], body_edit)
        .with_revision("renamed", &["main"], renamed)
        .with_revision("function-removed", &["main"], function_removed)
        .with_revision("type-removed", &["main"], type_removed)
        .with_revision("type-renamed", &["main"], type_renamed)
        .with_revision("added", &["main"], added)
    }

    fn revision(fake: &FakeSource, name: &str) -> SourceRevision {
        fake.resolve(&SourceRef::new("fake", "demo").expect("source"), name)
            .expect("revision")
    }

    fn produce(fake: &FakeSource, name: &str) -> ProducedRevision {
        produce_revision_facts(fake, &revision(fake, name), None, &FactsLimits::default())
            .expect("facts")
    }

    fn between(fake: &FakeSource, base: &ProducedRevision, head: &ProducedRevision) -> ChangeSet {
        fake.changes_between(&base.revision, &head.revision)
            .expect("changes")
    }

    /// What a finding means, apart from which snapshot it names: its
    /// fingerprint, its state and message, and the evidence ids it cites.
    /// The snapshot an evidence handle names is provenance, and a reused
    /// finding keeps naming the snapshot that produced it.
    fn semantics(findings: &[ReboundFinding]) -> Vec<(String, String, Vec<u64>)> {
        let mut meaning: Vec<_> = findings
            .iter()
            .map(|f| {
                let (state, evidence) = match &f.observation {
                    CachedObservation::Advisory {
                        evidence, message, ..
                    } => (
                        format!("advisory: {message}"),
                        evidence.iter().map(|e| e.evidence).collect(),
                    ),
                    other => (format!("{other:?}"), Vec::new()),
                };
                (f.semantic_fingerprint.to_string(), state, evidence)
            })
            .collect();
        meaning.sort();
        meaning
    }

    fn full_rerun(head: &ProducedRevision) -> Vec<ReboundFinding> {
        fresh_rerun(head, "reference")
            .expect("full re-execution")
            .findings
    }

    #[test]
    fn a_run_records_the_read_set_and_the_effective_capability() {
        let fake = fake();
        let base = produce(&fake, "main");

        let record = fresh_run(&base, &workspace()).expect("run");

        assert_eq!(record.findings.len(), 2, "alpha and beta");
        assert_eq!(record.produced_snapshot, base.facts.snapshot);
        assert!(!record.observed_truncated);
        let boundary = record
            .read_boundary
            .as_ref()
            .expect("supported projection boundary");
        assert_eq!(boundary.detector_id, record.identity.detector_id);
        assert_eq!(boundary.snapshot, base.facts.snapshot);
        assert_eq!(boundary.predicates, BTreeSet::from([DEFINES.to_string()]));
        assert_eq!(
            boundary.details,
            BTreeSet::from([FUNCTION_KIND.to_string()])
        );
        assert!(boundary.complete);
        let defined: BTreeSet<FactId> = base
            .facts
            .facts
            .iter()
            .filter(|f| {
                f.predicate.as_str() == DEFINES
                    && f.provenance.detail.as_deref() == Some(FUNCTION_KIND)
            })
            .map(|f| f.id)
            .collect();
        assert_eq!(defined.len(), 2);
        assert_eq!(
            record.observed_facts, defined,
            "the read-set is exactly the function definitions the detector was handed"
        );
        assert_eq!(
            record.identity.effective_capabilities,
            BTreeSet::from([AnalysisCapability::AstPattern])
        );
        assert_eq!(
            record.identity,
            current_identity(FUNCTIONS).expect("identity")
        );
        assert!(
            record
                .findings
                .iter()
                .all(|f| matches!(f.observation, CachedObservation::Advisory { .. })),
            "a candidate detector over AST evidence is advisory, never promoted"
        );
    }

    #[test]
    fn supported_projection_boundary_survives_record_and_scope_round_trip() {
        let produced = produce(&fake(), "main");
        let record = fresh_run(&produced, &workspace()).expect("run");
        let scope = crate::application::change_tracking::read_scope::ReadScope::from_run(
            &record,
            &produced.facts,
            crate::application::change_tracking::read_scope::ReadBreadth::Aggregate,
        )
        .expect("complete, supported boundary");
        let record_wire = serde_json::to_value(&record).expect("record wire");
        let scope_wire = serde_json::to_value(&scope).expect("scope wire");
        let restored: DetectorRunRecord = serde_json::from_value(record_wire).expect("record");
        let restored_scope: crate::application::change_tracking::read_scope::ReadScope =
            serde_json::from_value(scope_wire).expect("scope");

        assert_eq!(restored, record);
        assert_eq!(restored_scope, scope);
        assert!(
            restored
                .read_boundary
                .as_ref()
                .unwrap()
                .is_supported_for(&restored)
        );
    }

    #[test]
    fn a_boundary_cannot_be_used_after_detector_identity_changes() {
        let produced = produce(&fake(), "main");
        let mut record = fresh_run(&produced, &workspace()).expect("run");
        record.identity.version.push_str("-changed");

        assert!(
            crate::application::change_tracking::read_scope::ReadScope::from_run(
                &record,
                &produced.facts,
                crate::application::change_tracking::read_scope::ReadBreadth::Pointwise,
            )
            .is_none()
        );

        record.identity = record.read_boundary.as_ref().unwrap().identity.clone();
        record.identity.digests.logic =
            crate::domain::findings::digest::DetectorDigest::from_content("changed logic");
        assert!(
            crate::application::change_tracking::read_scope::ReadScope::from_run(
                &record,
                &produced.facts,
                crate::application::change_tracking::read_scope::ReadBreadth::Pointwise,
            )
            .is_none()
        );
    }

    #[test]
    fn incomplete_or_truncated_boundaries_never_publish_scopes() {
        let produced = produce(&fake(), "main");
        let mut record = fresh_run(&produced, &workspace()).expect("run");
        record.read_boundary.as_mut().unwrap().complete = false;
        assert!(
            crate::application::change_tracking::read_scope::ReadScope::from_run(
                &record,
                &produced.facts,
                crate::application::change_tracking::read_scope::ReadBreadth::Pointwise,
            )
            .is_none()
        );

        record.read_boundary.as_mut().unwrap().complete = true;
        record.observed_truncated = true;
        assert!(
            crate::application::change_tracking::read_scope::ReadScope::from_run(
                &record,
                &produced.facts,
                crate::application::change_tracking::read_scope::ReadBreadth::Pointwise,
            )
            .is_none()
        );

        record.observed_truncated = false;
        record.read_boundary.as_mut().unwrap().schema =
            "cognicode.detector-read-boundary/v99".to_string();
        assert!(
            crate::application::change_tracking::read_scope::ReadScope::from_run(
                &record,
                &produced.facts,
                crate::application::change_tracking::read_scope::ReadBreadth::Pointwise,
            )
            .is_none()
        );
    }

    #[test]
    fn legacy_run_records_decode_without_a_trusted_boundary() {
        let fake = fake();
        let produced = produce(&fake, "main");
        let record = fresh_run(&produced, &workspace()).expect("run");
        let mut wire = serde_json::to_value(record).expect("serialize");
        wire.as_object_mut()
            .expect("record object")
            .remove("read_boundary");

        let legacy: DetectorRunRecord = serde_json::from_value(wire).expect("legacy record");
        assert!(legacy.read_boundary.is_none());
        assert!(
            crate::application::change_tracking::read_scope::ReadScope::from_run(
                &legacy,
                &produced.facts,
                crate::application::change_tracking::read_scope::ReadBreadth::Pointwise,
            )
            .is_none()
        );
    }

    #[test]
    fn a_run_is_deterministic() {
        let fake = fake();
        let base = produce(&fake, "main");
        assert_eq!(
            fresh_run(&base, &workspace()).expect("first"),
            fresh_run(&base, &workspace()).expect("second")
        );
    }

    #[test]
    fn reuse_over_an_unaffected_change_equals_a_full_reexecution() {
        let fake = fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "body-edit");
        let prior = fresh_run(&base, &workspace()).expect("prior run");

        let detection = fresh_detect(
            Some(&prior),
            &base,
            &head,
            &between(&fake, &base, &head),
            &workspace(),
        )
        .expect("detection");

        assert_eq!(
            detection.reuse,
            Reuse::Reused {
                produced_in: base.facts.snapshot
            }
        );
        assert!(detection.fresh.is_none(), "nothing ran again");
        assert_eq!(
            semantics(&detection.findings),
            semantics(&full_rerun(&head))
        );
        assert!(
            detection
                .findings
                .iter()
                .all(|f| f.reported_in == head.facts.snapshot
                    && f.produced_in == base.facts.snapshot),
            "reused findings name both the candidate and the snapshot that produced them"
        );
        assert!(
            detection.findings.iter().all(|f| matches!(
                &f.observation,
                CachedObservation::Advisory { evidence, .. }
                    if evidence.iter().all(|e| e.snapshot == base.facts.snapshot.0)
            )),
            "reused evidence keeps naming the snapshot that produced it"
        );
    }

    #[test]
    fn a_change_to_a_read_fact_forces_a_reexecution() {
        let fake = fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "renamed");
        let prior = fresh_run(&base, &workspace()).expect("prior run");

        let detection = fresh_detect(
            Some(&prior),
            &base,
            &head,
            &between(&fake, &base, &head),
            &workspace(),
        )
        .expect("detection");

        assert!(matches!(detection.reuse, Reuse::Rerun { .. }));
        assert!(detection.fresh.is_some());
        assert_eq!(
            semantics(&detection.findings),
            semantics(&full_rerun(&head))
        );
        assert_ne!(
            semantics(&detection.findings),
            semantics(&rebind_candidate(&prior, head.facts.snapshot)),
            "the stale findings would have been wrong"
        );
    }

    #[test]
    fn a_new_definition_forces_a_reexecution_that_finds_it() {
        let fake = fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "added");
        let prior = fresh_run(&base, &workspace()).expect("prior run");

        let detection = fresh_detect(
            Some(&prior),
            &base,
            &head,
            &between(&fake, &base, &head),
            &workspace(),
        )
        .expect("detection");

        assert!(matches!(detection.reuse, Reuse::Rerun { .. }));
        assert_eq!(detection.findings.len(), 3, "alpha, beta and gamma");
    }

    #[test]
    fn a_different_effective_capability_is_never_reused() {
        let fake = fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "body-edit");
        let mut prior = fresh_run(&base, &workspace()).expect("prior run");
        prior
            .identity
            .effective_capabilities
            .insert(AnalysisCapability::Dataflow);

        let detection = fresh_detect(
            Some(&prior),
            &base,
            &head,
            &between(&fake, &base, &head),
            &workspace(),
        )
        .expect("detection");

        let Reuse::Rerun { reason } = detection.reuse else {
            panic!("a record under another capability must not be reused");
        };
        assert!(reason.contains("capability"), "{reason}");
    }

    #[test]
    fn a_record_made_by_the_previous_facts_producer_is_never_reused() {
        // Before syntax errors became gaps, a damaged file contributed the
        // declarations that survived the fault, and a detector recorded its
        // findings over them. That record names the previous producer in its
        // parser layer, and it must not survive the change.
        let fake = fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "body-edit");
        let mut prior = fresh_run(&base, &workspace()).expect("prior run");
        prior.identity.parser = DetectorDigest::from_content("cognicode.revision-facts/v1");

        let detection = fresh_detect(
            Some(&prior),
            &base,
            &head,
            &between(&fake, &base, &head),
            &workspace(),
        )
        .expect("detection");

        let Reuse::Rerun { reason } = detection.reuse else {
            panic!("a record under the previous facts producer must not be reused");
        };
        assert!(reason.contains("parser"), "{reason}");
    }

    #[test]
    fn a_missing_foreign_or_truncated_read_set_is_never_reused() {
        let fake = fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "body-edit");
        let changes = between(&fake, &base, &head);
        let prior = fresh_run(&base, &workspace()).expect("prior run");
        let on_head = fresh_run(&head, &workspace()).expect("run on head");
        let truncated = DetectorRunRecord {
            observed_truncated: true,
            ..prior.clone()
        };

        for (record, expected) in [
            (None, "no prior read-set"),
            (Some(&on_head), "not recorded against the base"),
            (Some(&truncated), "truncated"),
        ] {
            let detection =
                fresh_detect(record, &base, &head, &changes, &workspace()).expect("detection");
            let Reuse::Rerun { reason } = detection.reuse else {
                panic!("{expected}: must not be reused");
            };
            assert!(reason.contains(expected), "{reason}");
            assert_eq!(
                semantics(&detection.findings),
                semantics(&full_rerun(&head))
            );
        }
    }

    #[test]
    fn missing_facts_never_prove_reuse() {
        let fake = fake();
        let base = produce(&fake, "main");
        let mut head = produce(&fake, "body-edit");
        head.coverage.unanalyzed_files += 1;
        head.coverage
            .gaps
            .push(super::super::revision_facts::FactGap {
                path: crate::domain::scm::PathKey::new("src/a.rs").expect("path"),
                reason: super::super::revision_facts::GapReason::NotText,
            });
        let prior = fresh_run(&base, &workspace()).expect("prior run");

        let detection = fresh_detect(
            Some(&prior),
            &base,
            &head,
            &between(&fake, &base, &head),
            &workspace(),
        )
        .expect("detection");

        assert!(matches!(detection.reuse, Reuse::Rerun { .. }));
    }

    #[test]
    fn ensure_run_writes_once_and_rewrites_when_the_snapshot_moves() {
        let fake = fake();
        let dir = tempfile::tempdir().expect("dir");
        let store = DetectorReuseStore::new(dir.path());
        let base = produce(&fake, "main");
        let head = produce(&fake, "body-edit");

        assert_eq!(
            fresh_ensure(&store, &base, &workspace()).expect("first"),
            RunWritten::Written
        );
        assert_eq!(
            fresh_ensure(&store, &base, &workspace()).expect("second"),
            RunWritten::AlreadyRecorded
        );
        let id = detector_id(FUNCTIONS).expect("id");
        let index = store.load().expect("index");
        assert!(index.record_for(id.as_str(), base.facts.snapshot).is_some());

        assert_eq!(
            fresh_ensure(&store, &head, &workspace()).expect("moved"),
            RunWritten::Written
        );
        let index = store.load().expect("index");
        assert!(index.record_for(id.as_str(), head.facts.snapshot).is_some());
        assert!(
            index.record_for(id.as_str(), base.facts.snapshot).is_some(),
            "moving to another snapshot keeps the previous base's record"
        );
    }

    #[test]
    fn a_stale_identity_is_rewritten_over_the_same_snapshot_only() {
        let fake = fake();
        let dir = tempfile::tempdir().expect("dir");
        let store = DetectorReuseStore::new(dir.path());
        let base = produce(&fake, "main");
        let other_base = produce(&fake, "renamed");
        let mut stale = fresh_run(&base, &workspace()).expect("run");
        stale
            .identity
            .effective_capabilities
            .insert(AnalysisCapability::Dataflow);
        store.record(stale).expect("stale");
        fresh_ensure(&store, &other_base, &workspace()).expect("other base");

        assert_eq!(
            fresh_ensure(&store, &base, &workspace()).expect("refresh"),
            RunWritten::Written,
            "a record under another capability is not current"
        );
        let id = detector_id(FUNCTIONS).expect("id");
        let index = store.load().expect("index");
        assert_eq!(index.records.len(), 2, "one record per base, no duplicate");
        assert_eq!(
            index
                .record_for(id.as_str(), base.facts.snapshot)
                .expect("base record")
                .identity,
            current_identity(FUNCTIONS).expect("identity")
        );
    }

    #[test]
    fn alternating_bases_each_keep_their_own_read_set() {
        let fake = fake();
        let dir = tempfile::tempdir().expect("dir");
        let store = DetectorReuseStore::new(dir.path());
        let first = produce(&fake, "main");
        let second = produce(&fake, "renamed");

        assert_eq!(
            fresh_ensure(&store, &first, &workspace()).expect("first"),
            RunWritten::Written
        );
        assert_eq!(
            fresh_ensure(&store, &second, &workspace()).expect("second"),
            RunWritten::Written
        );
        assert_eq!(
            fresh_ensure(&store, &first, &workspace()).expect("back to the first base"),
            RunWritten::AlreadyRecorded,
            "the second base must not have overwritten the first base's read-set"
        );
    }

    #[test]
    fn a_compare_still_reuses_its_base_after_another_base_was_recorded() {
        let fake = fake();
        let dir = tempfile::tempdir().expect("dir");
        let store = DetectorReuseStore::new(dir.path());
        let base = produce(&fake, "main");
        let other_base = produce(&fake, "renamed");
        let head = produce(&fake, "body-edit");
        fresh_ensure(&store, &base, &workspace()).expect("base");
        fresh_ensure(&store, &other_base, &workspace()).expect("other base");

        let index = store.load().expect("index");
        let assessment = assess_reuse(&base, &head, &between(&fake, &base, &head), Some(&index))
            .expect("assessment");

        assert_eq!(assessment.status, ReuseStatus::Evaluated, "{assessment:?}");
        assert_eq!(assessment.works.len(), 1, "only the record of this base");
        assert_eq!(assessment.works[0].disposition, WorkDisposition::Unaffected);
    }

    #[test]
    fn a_record_of_another_base_is_never_consumed() {
        let fake = fake();
        let dir = tempfile::tempdir().expect("dir");
        let store = DetectorReuseStore::new(dir.path());
        let base = produce(&fake, "main");
        let other_base = produce(&fake, "renamed");
        let head = produce(&fake, "body-edit");
        fresh_ensure(&store, &other_base, &workspace()).expect("other base only");

        let index = store.load().expect("index");
        let assessment = assess_reuse(&base, &head, &between(&fake, &base, &head), Some(&index))
            .expect("assessment");

        assert_eq!(
            assessment.status,
            ReuseStatus::NotEvaluated,
            "{assessment:?}"
        );
        assert!(assessment.works.is_empty());
    }

    #[test]
    fn the_store_replaces_a_records_own_base_and_keeps_the_others() {
        let fake = fake();
        let dir = tempfile::tempdir().expect("dir");
        let store = DetectorReuseStore::new(dir.path());
        let base = produce(&fake, "main");
        let head = produce(&fake, "body-edit");
        let mut other = fresh_run(&base, &workspace()).expect("run");
        other.identity.detector_id = DetectorId::new("other.detector").expect("id");

        store.record(other.clone()).expect("other");
        store
            .record(fresh_run(&base, &workspace()).expect("run"))
            .expect("first");
        store
            .record(fresh_run(&head, &workspace()).expect("run"))
            .expect("second base");
        let again = fresh_run(&base, &workspace()).expect("run");
        store.record(again.clone()).expect("same base again");

        let index = store.load().expect("index");
        assert_eq!(index.records.len(), 3, "other detector, base and head");
        assert_eq!(
            index.record_for("other.detector", base.facts.snapshot),
            Some(&other)
        );
        let id = detector_id(FUNCTIONS).expect("id");
        assert_eq!(
            index.record_for(id.as_str(), base.facts.snapshot),
            Some(&again)
        );
        assert!(index.record_for(id.as_str(), head.facts.snapshot).is_some());
    }

    fn type_facts(produced: &ProducedRevision) -> BTreeSet<FactId> {
        produced
            .facts
            .facts
            .iter()
            .filter(|f| {
                f.predicate.as_str() == DEFINES
                    && matches!(
                        f.provenance.detail.as_deref(),
                        Some("kind=Struct" | "kind=Class" | "kind=Enum" | "kind=Trait")
                    )
            })
            .map(|f| f.id)
            .collect()
    }

    fn detect_head(
        detector: ProductionDetector,
        fake: &FakeSource,
        base: &ProducedRevision,
        head: &ProducedRevision,
    ) -> Detection {
        let prior = fresh_run_of(detector, base).expect("prior run");
        fresh_detect_of(
            detector,
            Some(&prior),
            base,
            head,
            &between(fake, base, head),
        )
        .expect("detection")
    }

    #[test]
    fn the_type_detector_records_its_own_read_set_identity_and_capability() {
        let fake = fake();
        let base = produce(&fake, "main");

        let record = fresh_run_of(TYPES, &base).expect("run");

        let defined = type_facts(&base);
        assert_eq!(defined.len(), 1, "only Point is a type definition");
        assert_eq!(record.findings.len(), 1);
        assert_eq!(
            record.read_boundary.as_ref().unwrap().details,
            TYPE_KINDS.iter().map(|detail| detail.to_string()).collect()
        );
        assert_eq!(
            record.observed_facts, defined,
            "the read-set is exactly the type definitions the detector was handed"
        );
        assert!(!record.observed_truncated);
        assert_eq!(
            record.identity.effective_capabilities,
            BTreeSet::from([AnalysisCapability::AstPattern])
        );
        assert_eq!(record.identity, current_identity(TYPES).expect("identity"));
        let functions = current_identity(FUNCTIONS).expect("identity");
        assert_ne!(record.identity.detector_id, functions.detector_id);
        assert_ne!(record.identity.digests, functions.digests);
    }

    #[test]
    fn the_two_detectors_read_disjoint_facts() {
        let fake = fake();
        let base = produce(&fake, "main");

        let types = fresh_run_of(TYPES, &base).expect("types");
        let functions = fresh_run_of(FUNCTIONS, &base).expect("functions");

        assert!(
            types.observed_facts.is_disjoint(&functions.observed_facts),
            "a detector's read-set holds only what its own projection handed it"
        );
    }

    #[test]
    fn an_unaffected_change_reuses_every_detector_and_equals_a_full_rerun() {
        let fake = fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "body-edit");

        for detector in ProductionDetector::ALL {
            let detection = detect_head(detector, &fake, &base, &head);
            assert_eq!(
                detection.reuse,
                Reuse::Reused {
                    produced_in: base.facts.snapshot
                },
                "{detector:?}"
            );
            assert!(detection.fresh.is_none(), "{detector:?}: nothing ran");
            let reference = fresh_rerun_of(detector, &head, "reference").expect("rerun");
            assert_eq!(
                semantics(&detection.findings),
                semantics(&reference.findings),
                "{detector:?}"
            );
        }
    }

    #[test]
    fn a_changed_type_reruns_the_type_detector_and_finds_the_new_name() {
        let fake = fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "type-renamed");

        let detection = detect_head(TYPES, &fake, &base, &head);

        let Reuse::Rerun { reason } = &detection.reuse else {
            panic!("a read type definition changed: {:?}", detection.reuse);
        };
        assert!(
            reason.contains("touches a fact the detector read"),
            "{reason}"
        );
        let prior = fresh_run_of(TYPES, &base).expect("prior");
        assert_ne!(
            semantics(&detection.findings),
            semantics(&rebind_candidate(&prior, head.facts.snapshot)),
            "the stale finding would have named Point"
        );
        assert_eq!(
            semantics(&detection.findings),
            semantics(
                &fresh_rerun_of(TYPES, &head, "reference")
                    .expect("rerun")
                    .findings
            )
        );
    }

    #[test]
    fn removing_what_one_detector_read_leaves_the_other_decided_on_its_own() {
        let fake = fake();
        let base = produce(&fake, "main");
        let reused = Reuse::Reused {
            produced_in: base.facts.snapshot,
        };

        let head = produce(&fake, "function-removed");
        assert!(
            matches!(
                detect_head(FUNCTIONS, &fake, &base, &head).reuse,
                Reuse::Rerun { .. }
            ),
            "alpha was read and is gone"
        );
        assert_eq!(
            detect_head(TYPES, &fake, &base, &head).reuse,
            reused,
            "no type definition was read by the change"
        );

        let head = produce(&fake, "type-removed");
        assert!(
            matches!(
                detect_head(TYPES, &fake, &base, &head).reuse,
                Reuse::Rerun { .. }
            ),
            "Point was read and is gone"
        );
        assert_eq!(
            detect_head(FUNCTIONS, &fake, &base, &head).reuse,
            reused,
            "no function definition was read by the change"
        );
    }

    #[test]
    fn a_missing_foreign_or_truncated_type_read_set_is_never_reused() {
        let fake = fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "body-edit");
        let changes = between(&fake, &base, &head);
        let prior = fresh_run_of(TYPES, &base).expect("prior run");
        let on_head = fresh_run_of(TYPES, &head).expect("run on head");
        let functions = fresh_run_of(FUNCTIONS, &base).expect("functions run");
        let truncated = DetectorRunRecord {
            observed_truncated: true,
            ..prior.clone()
        };

        for (record, expected) in [
            (None, "no prior read-set"),
            (Some(&on_head), "not recorded against the base"),
            (Some(&truncated), "truncated"),
            (Some(&functions), "invalidated layer"),
        ] {
            let detection =
                fresh_detect_of(TYPES, record, &base, &head, &changes).expect("detection");
            let Reuse::Rerun { reason } = detection.reuse else {
                panic!("{expected}: must not be reused");
            };
            assert!(reason.contains(expected), "{reason}");
        }
    }

    #[test]
    fn ensure_runs_writes_one_record_per_detector_and_keeps_them_apart() {
        let fake = fake();
        let dir = tempfile::tempdir().expect("dir");
        let base = produce(&fake, "main");
        let repository = std::path::Path::new("/repo/demo");

        let written = ensure_runs_in_cache(dir.path(), repository, &base, &mut TestSink::default());
        assert_eq!(written.len(), ProductionDetector::ALL.len());
        assert!(
            written
                .iter()
                .all(|(_, outcome)| *outcome == Ok(RunWritten::Written)),
            "{written:?}"
        );

        let again = ensure_runs_in_cache(dir.path(), repository, &base, &mut TestSink::default());
        assert!(
            again
                .iter()
                .all(|(_, outcome)| *outcome == Ok(RunWritten::AlreadyRecorded)),
            "{again:?}"
        );
        let index = DetectorReuseStore::for_workspace(dir.path(), repository)
            .load()
            .expect("index");
        for detector in ProductionDetector::ALL {
            let id = detector_id(detector).expect("id");
            assert!(
                index.record_for(id.as_str(), base.facts.snapshot).is_some(),
                "{detector:?}"
            );
        }
    }

    #[test]
    fn detect_in_cache_decides_each_detector_over_its_own_record() {
        let fake = fake();
        let dir = tempfile::tempdir().expect("dir");
        let base = produce(&fake, "main");
        let head = produce(&fake, "body-edit");
        let changes = between(&fake, &base, &head);
        let repository = std::path::Path::new("/repo/demo");
        ensure_runs_in_cache(dir.path(), repository, &base, &mut TestSink::default());

        for detector in ProductionDetector::ALL {
            let detection = detect_in_cache(
                detector,
                Some(dir.path()),
                repository,
                &base,
                &head,
                &changes,
                &mut TestSink::default(),
            )
            .expect("detection");
            assert!(
                matches!(detection.reuse, Reuse::Reused { .. }),
                "{detector:?}: {:?}",
                detection.reuse
            );
        }
    }

    // ----- The Terraform detector ---------------------------------------

    const BUCKETS: ProductionDetector = ProductionDetector::UnprotectedBucket;

    const LOGS_BUCKET: &[u8] = b"resource \"aws_s3_bucket\" \"logs\" {\n  bucket = var.name\n}\n";
    const LOGS_BUCKET_TAGGED: &[u8] = b"resource \"aws_s3_bucket\" \"logs\" {\n  bucket = var.name\n  tags = {\n    Team = \"platform\"\n  }\n}\n";
    const LOGS_WITH_BLOCK: &[u8] = b"resource \"aws_s3_bucket\" \"logs\" {\n  bucket = var.name\n}\n\nresource \"aws_s3_bucket_public_access_block\" \"logs\" {\n  bucket                  = aws_s3_bucket.logs.id\n  block_public_acls       = true\n  block_public_policy     = true\n  ignore_public_acls      = true\n  restrict_public_buckets = true\n}\n";
    const BLOCK_OF_LOGS_ELSEWHERE: &[u8] = b"resource \"aws_s3_bucket_public_access_block\" \"logs\" {\n  bucket = aws_s3_bucket.logs.id\n}\n";
    const BLOCK_OF_ANOTHER_BUCKET: &[u8] = b"resource \"aws_s3_bucket_public_access_block\" \"logs\" {\n  bucket = aws_s3_bucket.other.id\n}\n";
    const BROKEN_TERRAFORM: &[u8] =
        b"resource \"aws_s3_bucket_public_access_block\" \"logs\" {\n  bucket = \n";
    const ARCHIVE_BUCKET: &[u8] =
        b"resource \"aws_s3_bucket\" \"archive\" {\n  bucket = \"archive\"\n}\n";

    /// A revision's name, its parents and its files.
    type TerraformRevision<'a> = (&'a str, &'a [&'a str], Vec<(&'a str, &'a [u8])>);

    fn terraform_fake() -> FakeSource {
        let revisions: [TerraformRevision; 9] = [
            (
                "main",
                &[],
                vec![
                    ("infra/main.tf", LOGS_BUCKET),
                    ("other/main.tf", ARCHIVE_BUCKET),
                ],
            ),
            (
                "unrelated-edit",
                &["main"],
                vec![
                    ("infra/main.tf", LOGS_BUCKET_TAGGED),
                    ("other/main.tf", ARCHIVE_BUCKET),
                ],
            ),
            (
                "protected",
                &["main"],
                vec![
                    ("infra/main.tf", LOGS_WITH_BLOCK),
                    ("other/main.tf", ARCHIVE_BUCKET),
                ],
            ),
            (
                "protected-in-another-file",
                &["main"],
                vec![
                    ("infra/main.tf", LOGS_BUCKET),
                    ("infra/protection.tf", BLOCK_OF_LOGS_ELSEWHERE),
                    ("other/main.tf", ARCHIVE_BUCKET),
                ],
            ),
            (
                "protected-in-another-module",
                &["main"],
                vec![
                    ("infra/main.tf", LOGS_BUCKET),
                    ("other/protection.tf", BLOCK_OF_LOGS_ELSEWHERE),
                    ("other/main.tf", ARCHIVE_BUCKET),
                ],
            ),
            (
                "block_of_another_bucket",
                &["main"],
                vec![
                    ("infra/main.tf", LOGS_BUCKET),
                    ("infra/protection.tf", BLOCK_OF_ANOTHER_BUCKET),
                    ("other/main.tf", ARCHIVE_BUCKET),
                ],
            ),
            (
                "module-unreadable",
                &["main"],
                vec![
                    ("infra/main.tf", LOGS_BUCKET),
                    ("infra/protection.tf", BROKEN_TERRAFORM),
                    ("other/main.tf", ARCHIVE_BUCKET),
                ],
            ),
            (
                "block-removed",
                &["protected"],
                vec![
                    ("infra/main.tf", LOGS_BUCKET),
                    ("other/main.tf", ARCHIVE_BUCKET),
                ],
            ),
            (
                "bucket-removed",
                &["main"],
                vec![("other/main.tf", ARCHIVE_BUCKET)],
            ),
        ];
        revisions.iter().fold(
            FakeSource::new(
                "fake",
                ProviderCapabilities {
                    ancestor_queries: true,
                    ..ProviderCapabilities::NONE
                },
            ),
            |source, (name, parents, files)| source.with_revision(name, parents, files),
        )
    }

    /// The buckets the detector reports on `name`, by declaration.
    fn unprotected_in(fake: &FakeSource, name: &str) -> Vec<String> {
        let head = produce(fake, name);
        let record = fresh_run_of(BUCKETS, &head).expect("run");
        let mut names: Vec<String> = record
            .findings
            .iter()
            .flat_map(|finding| finding.output_locations.iter().map(|l| l.file.clone()))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_bucket_no_public_access_block_references_is_reported_where_it_is_declared() {
        let fake = terraform_fake();

        assert_eq!(
            unprotected_in(&fake, "main"),
            ["infra/main.tf", "other/main.tf"]
        );
    }

    #[test]
    fn a_block_in_the_same_file_protects_the_bucket() {
        assert_eq!(
            unprotected_in(&terraform_fake(), "protected"),
            ["other/main.tf"]
        );
    }

    #[test]
    fn a_block_in_another_file_of_the_module_protects_the_bucket() {
        assert_eq!(
            unprotected_in(&terraform_fake(), "protected-in-another-file"),
            ["other/main.tf"]
        );
    }

    #[test]
    fn a_block_of_another_module_or_of_another_bucket_protects_nothing() {
        let fake = terraform_fake();

        assert_eq!(
            unprotected_in(&fake, "protected-in-another-module"),
            ["infra/main.tf", "other/main.tf"],
            "the block names `aws_s3_bucket.logs` of its own module"
        );
        assert_eq!(
            unprotected_in(&fake, "block_of_another_bucket"),
            ["infra/main.tf", "other/main.tf"]
        );
    }

    #[test]
    fn a_module_with_an_unreadable_file_is_not_judged_but_the_others_still_are() {
        let fake = terraform_fake();
        let head = produce(&fake, "module-unreadable");
        assert!(!head.coverage.is_complete(), "the broken file is a gap");

        assert_eq!(
            unprotected_in(&fake, "module-unreadable"),
            ["other/main.tf"],
            "infra/protection.tf may hold the block, so no absence is claimed for infra"
        );
    }

    #[test]
    fn the_bucket_detector_reads_only_what_decides_it() {
        let fake = terraform_fake();
        let head = produce(&fake, "protected");
        let record = fresh_run_of(BUCKETS, &head).expect("run");
        assert!(
            record.read_boundary.is_none(),
            "Terraform predicate completeness is unsupported"
        );

        let consulted: BTreeSet<String> = head
            .facts
            .facts
            .iter()
            .filter(|f| record.observed_facts.contains(&f.id))
            .map(|f| format!("{} {:?}", f.predicate.as_str(), f.object))
            .collect();
        let tags: BTreeSet<&str> = ["core:defines", "core:references"].into();
        assert!(
            consulted
                .iter()
                .all(|c| tags.iter().any(|tag| c.starts_with(tag))),
            "{consulted:?}"
        );
        assert!(
            consulted.iter().any(|c| c.contains("aws_s3_bucket.logs\"")),
            "the bucket declaration is read: {consulted:?}"
        );
        assert!(
            consulted
                .iter()
                .any(|c| c.contains("aws_s3_bucket_public_access_block.logs")),
            "the block declaration is read: {consulted:?}"
        );
        assert!(
            !consulted.iter().any(|c| c.contains("var.name")),
            "a reference of the bucket itself decides nothing: {consulted:?}"
        );
        assert!(
            record.observed_facts.len() < head.facts.facts.len(),
            "not every fact of the revision"
        );
    }

    fn assert_detection_equals_a_cold_run(fake: &FakeSource, head_name: &str, want: &Reuse) {
        let base = produce(fake, "main");
        let head = produce(fake, head_name);
        let detection = detect_head(BUCKETS, fake, &base, &head);
        match want {
            Reuse::Reused { .. } => assert_eq!(detection.reuse, *want, "{head_name}"),
            Reuse::Rerun { .. } => assert!(
                matches!(detection.reuse, Reuse::Rerun { .. }),
                "{head_name}: {:?}",
                detection.reuse
            ),
        }
        let cold = fresh_rerun_of(BUCKETS, &head, "reference").expect("cold");
        assert_eq!(
            semantics(&detection.findings),
            semantics(&cold.findings),
            "{head_name}: the reused or re-run findings are the cold run's"
        );
    }

    #[test]
    fn an_edit_outside_the_read_facts_is_reused_and_equals_a_cold_run() {
        let reused = Reuse::Reused {
            produced_in: produce(&terraform_fake(), "main").facts.snapshot,
        };
        assert_detection_equals_a_cold_run(&terraform_fake(), "unrelated-edit", &reused);
    }

    #[test]
    fn adding_the_block_that_protects_a_bucket_forces_a_rerun_that_drops_the_finding() {
        let fake = terraform_fake();
        let rerun = Reuse::Rerun {
            reason: String::new(),
        };
        for head in ["protected", "protected-in-another-file"] {
            assert_detection_equals_a_cold_run(&fake, head, &rerun);
        }
        let base = produce(&fake, "main");
        let head = produce(&fake, "protected");
        assert_eq!(
            detect_head(BUCKETS, &fake, &base, &head).findings.len(),
            1,
            "only the archive bucket is left"
        );
    }

    #[test]
    fn removing_the_block_forces_a_rerun_that_finds_the_bucket_again() {
        let fake = terraform_fake();
        // The base record holds the block's facts.
        let base = produce(&fake, "protected");
        let head = produce(&fake, "block-removed");
        let detection = detect_head(BUCKETS, &fake, &base, &head);

        assert!(matches!(detection.reuse, Reuse::Rerun { .. }));
        assert_eq!(detection.findings.len(), 2);
    }

    #[test]
    fn a_module_that_turns_unreadable_is_never_answered_from_the_prior_run() {
        let fake = terraform_fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "module-unreadable");
        let detection = detect_head(BUCKETS, &fake, &base, &head);

        assert!(
            matches!(detection.reuse, Reuse::Rerun { .. }),
            "facts are missing, so nothing is proven reusable: {:?}",
            detection.reuse
        );
        let reported: Vec<_> = detection
            .findings
            .iter()
            .flat_map(|f| f.output_locations.iter().map(|l| l.file.as_str()))
            .collect();
        assert_eq!(reported, ["other/main.tf"]);
    }

    #[test]
    fn removing_the_bucket_removes_its_finding() {
        let fake = terraform_fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "bucket-removed");
        let detection = detect_head(BUCKETS, &fake, &base, &head);

        assert!(matches!(detection.reuse, Reuse::Rerun { .. }));
        assert_eq!(detection.findings.len(), 1);
    }

    #[test]
    fn a_revision_without_terraform_yields_a_record_with_nothing_read() {
        let fake = fake();
        let record = fresh_run_of(BUCKETS, &produce(&fake, "main")).expect("run");

        assert!(record.findings.is_empty());
        assert!(record.observed_facts.is_empty());
    }
}
