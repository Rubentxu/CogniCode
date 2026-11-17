//! Production detector runs over revision facts (CGH-044, CAA-11c): the
//! writer of read-sets that [`assess_reuse`] consumes.
//!
//! ## What runs
//!
//! The function-definition detector (the IR the grounded finding flow
//! already defines) runs through the real seam: `DetectorAdmission` ->
//! `ExecutionPermit` -> `DetectorExecutor` -> `AstBackend`. Its AST view is
//! projected from the revision's own facts, so a run reads exactly the
//! `core:defines` facts of kind `Function` and nothing else.
//!
//! ## What is recorded
//!
//! One [`DetectorRunRecord`] per run, on the existing
//! [`DetectorReuseStore`]:
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
use crate::domain::findings::detector_ir::SubjectPattern;
use crate::domain::findings::detector_reuse::{
    CachedFinding, DetectorReuseIndex, DetectorRunIdentity, DetectorRunRecord, InvalidationLevel,
    OutputLocation, ReboundFinding, ReuseDecision, cache_observation, rebind_candidate,
    reuse_decision, semantic_fingerprint,
};
use crate::domain::findings::execution::{
    AnalysisInput, BackendRegistry, DetectorExecutor, ExecutionRequest,
};
use crate::domain::findings::grounding::GroundingRef;
use crate::domain::findings::guard_transfer::to_guard_observation;
use crate::domain::findings::projection::project_finding;
use crate::domain::findings::scope::AnalysisScope;
use crate::domain::findings::{AnalysisCapability, DetectorDigest, DetectorId};
use crate::domain::kernel_ids::ExecutionId;
use crate::domain::scm::ChangeSet;
use crate::domain::value_objects::WorkspaceId;
use crate::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;

use super::planner::WorkDisposition;
use super::revision_facts::{ProducedRevision, assess_reuse, producer_versions};
use super::scm_delta::RevisionFacts;

/// The version stamped on the detector this module runs.
pub const DETECTOR_VERSION: &str = "cgh-044-1.0.0";

const DEFINES: &str = "core:defines";
const FUNCTION_KIND: &str = "kind=Function";

/// Why a detector run could not be produced.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
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

/// One function-definition fact the detector reads.
struct FunctionRead {
    path: String,
    fqn: String,
    line: u32,
    fact: FactId,
    construct: AstConstruct,
}

/// The pure projection of a revision's facts into what the detector reads:
/// every `core:defines` fact of kind `Function`, grounded on its own fact.
///
/// Units follow path order and constructs follow fact order, so the same
/// facts always project to the same view.
fn function_reads(facts: &RevisionFacts) -> Result<Vec<FunctionRead>, DetectorRunError> {
    let subject = SubjectPattern::new(SUBJECT_FUNCTION_DEFINITION)
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
                || fact.provenance.detail.as_deref() != Some(FUNCTION_KIND)
            {
                continue;
            }
            // The extractor names a symbol `<path>:<name>:<line>`.
            let line = fqn
                .rsplit(':')
                .next()
                .and_then(|tail| tail.parse::<u32>().ok())
                .unwrap_or(0);
            reads.push(FunctionRead {
                path: path.as_str().to_string(),
                fqn: fqn.clone(),
                line,
                fact: fact.id,
                construct: AstConstruct {
                    subject: subject.clone(),
                    line,
                    detail: format!("function `{fqn}`"),
                    grounding: Some(GroundingRef::entity(fact.subject, fact.id)),
                },
            });
        }
    }
    Ok(reads)
}

fn ast_view(reads: &[FunctionRead]) -> AstInput {
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

fn permit() -> Result<ExecutionPermit, DetectorRunError> {
    let ir = function_definition_detector_ir().map_err(DetectorRunError::Admission)?;
    DetectorAdmission::admit(ir, DETECTOR_VERSION, AdmissionSource::Builtin)
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

/// The identity a run executed now would carry.
///
/// # Errors
/// The detector could not be admitted or no backend serves it.
pub fn current_identity() -> Result<DetectorRunIdentity, DetectorRunError> {
    identity_of(&permit()?, &registry())
}

/// The id under which this detector's records are kept.
///
/// # Errors
/// The detector definition is invalid.
pub fn detector_id() -> Result<DetectorId, DetectorRunError> {
    Ok(current_identity()?.detector_id)
}

/// Runs the detector over `facts` through the real executor and records
/// what it read and under which capability.
///
/// # Errors
/// Admission, execution or finding/construct arity failed. A failed run
/// records nothing: there is no half-written read-set.
pub fn run_detector(
    facts: &RevisionFacts,
    workspace: &WorkspaceId,
) -> Result<DetectorRunRecord, DetectorRunError> {
    let permit = permit()?;
    let registry = registry();
    let identity = identity_of(&permit, &registry)?;
    let reads = function_reads(facts)?;

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
    let mut sink = InMemoryEvidenceStore::with_scope(scope);
    let executed = DetectorExecutor::new(&registry)
        .execute(&permit, &input, &mut sink, request)
        .map_err(|error| DetectorRunError::Execution(error.to_string()))?;

    // The backend emits one match per construct, in construct order. The
    // AST view lists constructs by unit, so the matches follow the same
    // order and the pairing below is positional.
    let in_view: Vec<&FunctionRead> = {
        let mut ordered: Vec<&FunctionRead> = reads.iter().collect();
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
        observed_facts: reads.iter().map(|read| read.fact).collect(),
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

/// Makes sure the store holds a current run of the detector for `produced`:
/// keeps the existing record when it is for this very snapshot under the
/// identity a run would carry now, and runs and writes otherwise. The
/// records kept for other snapshots are left as they were.
///
/// # Errors
/// The run or the write failed; the previous record is left as it was.
pub fn ensure_run(
    store: &DetectorReuseStore,
    produced: &ProducedRevision,
    workspace: &WorkspaceId,
) -> Result<RunWritten, DetectorRunError> {
    let identity = current_identity()?;
    let current = store.load().is_some_and(|index| {
        index
            .record_for(identity.detector_id.as_str(), produced.facts.snapshot)
            .is_some_and(|record| record.identity == identity && !record.observed_truncated)
    });
    if current {
        return Ok(RunWritten::AlreadyRecorded);
    }
    let record = run_detector(&produced.facts, workspace)?;
    store
        .record(record)
        .map_err(|error| DetectorRunError::Persist(error.to_string()))?;
    Ok(RunWritten::Written)
}

/// [`ensure_run`] against the store `cache_root` keeps for the repository at
/// `repository`: the one entry point the goals use to write a read-set.
///
/// # Errors
/// The run or the write failed; the previous record is left as it was.
pub fn ensure_run_in_cache(
    cache_root: &Path,
    repository: &Path,
    produced: &ProducedRevision,
) -> Result<RunWritten, DetectorRunError> {
    let workspace = WorkspaceId::try_new(workspace_key(repository))
        .map_err(|error| DetectorRunError::Admission(error.to_string()))?;
    ensure_run(
        &DetectorReuseStore::for_workspace(cache_root, repository),
        produced,
        &workspace,
    )
}

/// [`detect_with_reuse`] over the read-set the store under `cache_root`
/// keeps for the base revision of `repository`: the one entry point the goals
/// use to publish the head findings.
///
/// Without a cache, or without a record for this base, there is no prior
/// read-set and the detector runs again.
///
/// # Errors
/// The plan could not be computed or the re-run failed.
pub fn detect_in_cache(
    cache_root: Option<&Path>,
    repository: &Path,
    base: &ProducedRevision,
    head: &ProducedRevision,
    changes: &ChangeSet,
) -> Result<Detection, DetectorRunError> {
    let workspace = WorkspaceId::try_new(workspace_key(repository))
        .map_err(|error| DetectorRunError::Admission(error.to_string()))?;
    let detector = detector_id()?;
    let index =
        cache_root.and_then(|root| DetectorReuseStore::for_workspace(root, repository).load());
    let prior = index
        .as_ref()
        .and_then(|index| index.record_for(detector.as_str(), base.facts.snapshot));
    detect_with_reuse(prior, base, head, changes, &workspace)
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
    head: &ProducedRevision,
    workspace: &WorkspaceId,
    reason: impl Into<String>,
) -> Result<Detection, DetectorRunError> {
    let record = run_detector(&head.facts, workspace)?;
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

/// The detector's findings on `head`, reusing `prior` when it is proven safe
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
    prior: Option<&DetectorRunRecord>,
    base: &ProducedRevision,
    head: &ProducedRevision,
    changes: &ChangeSet,
    workspace: &WorkspaceId,
) -> Result<Detection, DetectorRunError> {
    let Some(prior) = prior else {
        return rerun(head, workspace, "no prior read-set is recorded");
    };
    if prior.produced_snapshot != base.facts.snapshot {
        return rerun(
            head,
            workspace,
            "the prior read-set was not recorded against the base revision",
        );
    }
    if prior.observed_truncated {
        return rerun(head, workspace, "the prior read-set is truncated");
    }
    let current = current_identity()?;
    if let ReuseDecision::Invalidated { levels } = reuse_decision(prior, &current, None) {
        return rerun(
            head,
            workspace,
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
        Some(WorkDisposition::Affected) => {
            rerun(head, workspace, "a change touches a fact the detector read")
        }
        Some(WorkDisposition::Unknown) | None => rerun(
            head,
            workspace,
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
    use crate::domain::findings::detector_reuse::CachedObservation;
    use crate::domain::scm::fake::FakeSource;
    use crate::domain::scm::{
        ChangeReader, ProviderCapabilities, RevisionReader, SourceRef, SourceRevision,
    };

    const ALPHA_V1: &[u8] = b"pub fn alpha() { first(); }\n";
    const ALPHA_BODY_EDIT: &[u8] = b"pub fn alpha() { second(); }\n";
    const ALPHA_RENAMED: &[u8] = b"pub fn alpha_two() { first(); }\n";
    const BETA: &[u8] = b"pub fn beta() { helper(); }\n";
    const GAMMA: &[u8] = b"pub fn gamma() {}\n";

    fn workspace() -> WorkspaceId {
        WorkspaceId::try_new("ws").expect("workspace")
    }

    fn fake() -> FakeSource {
        let base: &[(&str, &[u8])] = &[("src/a.rs", ALPHA_V1), ("src/b.rs", BETA)];
        let body_edit: &[(&str, &[u8])] = &[("src/a.rs", ALPHA_BODY_EDIT), ("src/b.rs", BETA)];
        let renamed: &[(&str, &[u8])] = &[("src/a.rs", ALPHA_RENAMED), ("src/b.rs", BETA)];
        let added: &[(&str, &[u8])] = &[
            ("src/a.rs", ALPHA_V1),
            ("src/b.rs", BETA),
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
        rerun(head, &workspace(), "reference")
            .expect("full re-execution")
            .findings
    }

    #[test]
    fn a_run_records_the_read_set_and_the_effective_capability() {
        let fake = fake();
        let base = produce(&fake, "main");

        let record = run_detector(&base.facts, &workspace()).expect("run");

        assert_eq!(record.findings.len(), 2, "alpha and beta");
        assert_eq!(record.produced_snapshot, base.facts.snapshot);
        assert!(!record.observed_truncated);
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
        assert_eq!(record.identity, current_identity().expect("identity"));
        assert!(
            record
                .findings
                .iter()
                .all(|f| matches!(f.observation, CachedObservation::Advisory { .. })),
            "a candidate detector over AST evidence is advisory, never promoted"
        );
    }

    #[test]
    fn a_run_is_deterministic() {
        let fake = fake();
        let base = produce(&fake, "main");
        assert_eq!(
            run_detector(&base.facts, &workspace()).expect("first"),
            run_detector(&base.facts, &workspace()).expect("second")
        );
    }

    #[test]
    fn reuse_over_an_unaffected_change_equals_a_full_reexecution() {
        let fake = fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "body-edit");
        let prior = run_detector(&base.facts, &workspace()).expect("prior run");

        let detection = detect_with_reuse(
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
        let prior = run_detector(&base.facts, &workspace()).expect("prior run");

        let detection = detect_with_reuse(
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
        let prior = run_detector(&base.facts, &workspace()).expect("prior run");

        let detection = detect_with_reuse(
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
        let mut prior = run_detector(&base.facts, &workspace()).expect("prior run");
        prior
            .identity
            .effective_capabilities
            .insert(AnalysisCapability::Dataflow);

        let detection = detect_with_reuse(
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
    fn a_missing_foreign_or_truncated_read_set_is_never_reused() {
        let fake = fake();
        let base = produce(&fake, "main");
        let head = produce(&fake, "body-edit");
        let changes = between(&fake, &base, &head);
        let prior = run_detector(&base.facts, &workspace()).expect("prior run");
        let on_head = run_detector(&head.facts, &workspace()).expect("run on head");
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
                detect_with_reuse(record, &base, &head, &changes, &workspace()).expect("detection");
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
        let prior = run_detector(&base.facts, &workspace()).expect("prior run");

        let detection = detect_with_reuse(
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
            ensure_run(&store, &base, &workspace()).expect("first"),
            RunWritten::Written
        );
        assert_eq!(
            ensure_run(&store, &base, &workspace()).expect("second"),
            RunWritten::AlreadyRecorded
        );
        let id = detector_id().expect("id");
        let index = store.load().expect("index");
        assert!(index.record_for(id.as_str(), base.facts.snapshot).is_some());

        assert_eq!(
            ensure_run(&store, &head, &workspace()).expect("moved"),
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
        let mut stale = run_detector(&base.facts, &workspace()).expect("run");
        stale
            .identity
            .effective_capabilities
            .insert(AnalysisCapability::Dataflow);
        store.record(stale).expect("stale");
        ensure_run(&store, &other_base, &workspace()).expect("other base");

        assert_eq!(
            ensure_run(&store, &base, &workspace()).expect("refresh"),
            RunWritten::Written,
            "a record under another capability is not current"
        );
        let id = detector_id().expect("id");
        let index = store.load().expect("index");
        assert_eq!(index.records.len(), 2, "one record per base, no duplicate");
        assert_eq!(
            index
                .record_for(id.as_str(), base.facts.snapshot)
                .expect("base record")
                .identity,
            current_identity().expect("identity")
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
            ensure_run(&store, &first, &workspace()).expect("first"),
            RunWritten::Written
        );
        assert_eq!(
            ensure_run(&store, &second, &workspace()).expect("second"),
            RunWritten::Written
        );
        assert_eq!(
            ensure_run(&store, &first, &workspace()).expect("back to the first base"),
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
        ensure_run(&store, &base, &workspace()).expect("base");
        ensure_run(&store, &other_base, &workspace()).expect("other base");

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
        ensure_run(&store, &other_base, &workspace()).expect("other base only");

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
        let mut other = run_detector(&base.facts, &workspace()).expect("run");
        other.identity.detector_id = DetectorId::new("other.detector").expect("id");

        store.record(other.clone()).expect("other");
        store
            .record(run_detector(&base.facts, &workspace()).expect("run"))
            .expect("first");
        store
            .record(run_detector(&head.facts, &workspace()).expect("run"))
            .expect("second base");
        let again = run_detector(&base.facts, &workspace()).expect("run");
        store.record(again.clone()).expect("same base again");

        let index = store.load().expect("index");
        assert_eq!(index.records.len(), 3, "other detector, base and head");
        assert_eq!(
            index.record_for("other.detector", base.facts.snapshot),
            Some(&other)
        );
        let id = detector_id().expect("id");
        assert_eq!(
            index.record_for(id.as_str(), base.facts.snapshot),
            Some(&again)
        );
        assert!(index.record_for(id.as_str(), head.facts.snapshot).is_some());
    }
}
