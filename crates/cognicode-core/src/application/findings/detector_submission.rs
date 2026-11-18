//! Submitting catalog detectors to the core scheduler.
//!
//! A caller that wants a set of the security pack's detectors evaluated names
//! them the way a catalog reference does (id, pack version, languages) and
//! hands over what it observed. Each `(detector, language)` pair then runs as
//! one unit of work through the existing [`Coordinator`] and the existing
//! [`SecurityPack`]; this module adds no scheduler and no evaluator, only the
//! resolution of the reference against the pack's capability matrix and the
//! shape of the answer.
//!
//! ```text
//! DetectorSubmission ─ resolve ─▶ capability matrix cell ─┐
//!   (id, version, languages)         Unsupported ─────────┼─▶ DetectorResult
//! ObservedInputs ─ select by language ─▶ Coordinator::run ─┘   (status, basis,
//!   (functions, units)                     SecurityPack         read scope)
//! ```
//!
//! # Honesty of the answer
//!
//! Every requested `(detector, language)` pair is answered, in the order
//! asked, and none is dropped:
//!
//! - a detector the pack does not publish, a pack version other than the one
//!   this core publishes, a language the catalog does not name, and a pair the
//!   capability matrix marks unsupported are all answered
//!   [`DetectorStatus::Unsupported`] with the core's own reason;
//! - a pair with nothing observed to analyse is [`DetectorStatus::Unknown`],
//!   never "not observed": no run happened;
//! - work the scheduler refused, cancelled or saw fail is
//!   [`DetectorStatus::Inconclusive`] (required) or
//!   [`DetectorStatus::Deferred`] (optional), never a pass;
//! - when several observed inputs are analysed for one pair, the answer is the
//!   weakest honest summary: any finding makes it `found`, any input that
//!   could not decide or was not modelled makes it `unknown` rather than
//!   `not_observed`.
//!
//! # Basis and read scope
//!
//! [`ResultBasis`] names what the answer rests on: pack and detector version,
//! the logic digest, the backend, the evidence ceiling, the snapshot and how
//! many observed inputs were analysed. [`DetectorResult::read_scope`] is the
//! set of observed inputs the run read, as a [`ReadScope`] under
//! `cognicode.read-scope/v1`: one `(path, function)` per taint input and one
//! path per structural unit, read pointwise. It is `None` when nothing ran, so
//! a consumer resolving it with
//! [`resolve_reads`](crate::application::change_tracking::read_scope::resolve_reads)
//! revalidates instead of trusting an answer that read nothing.
//!
//! # Wire
//!
//! [`SubmissionReport`] serialises under [`DETECTOR_SUBMISSION_SCHEMA`]. New
//! fields are additive; a reader ignores fields it does not know.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::application::change_tracking::planner::WorkId;
use crate::application::change_tracking::read_scope::{ReadBreadth, ReadScope, ScopedRead};
use crate::application::change_tracking::scheduling::{
    CheckOutput, Computed, ConsumerClass, Coordinator, DaemonProbe, InconclusiveReason,
    RevisionTag, RunStatus, ScheduleRequest, WorkKey,
};
use crate::domain::findings::semantic_model::{
    AnalysisReach, CapabilityMatrix, ObservedFunction, ObservedUnit, PackVerdict, SecurityFamily,
    SourceLanguage, StructuralFamily, Support, Weakness,
};
use crate::domain::findings::{EvidenceClass, EvidenceLookup, EvidenceSink};
use crate::domain::kernel_ids::ExecutionId;
use crate::domain::naming::NamespacedName;

use super::{PackError, PackRun, RunContext, SecurityPack};

/// The schema string of a published [`SubmissionReport`].
pub const DETECTOR_SUBMISSION_SCHEMA: &str = "cognicode.detector-submission/v1";

/// One catalog detector a caller wants evaluated, by catalog reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorSubmission {
    /// The detector id, as the pack publishes it (the finding kind).
    pub id: String,
    /// The pack version the reference was written against.
    pub pack_version: String,
    /// The languages the detector is needed on. One answer per language.
    pub languages: Vec<String>,
    /// Whether the caller's plan requires this detector. A required detector
    /// the scheduler could not run is answered `inconclusive`; an optional one
    /// is answered `deferred`.
    #[serde(default)]
    pub required: bool,
}

/// What the caller observed, to be analysed by the submitted detectors.
///
/// Taint detectors read `functions`; structural detectors read `units`. An
/// input is analysed by a detector only on its own language.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedInputs {
    /// Functions, for the taint families.
    #[serde(default)]
    pub functions: Vec<ObservedFunction>,
    /// Source units, for the structural families.
    #[serde(default)]
    pub units: Vec<ObservedUnit>,
}

/// A submission to run: what to evaluate, over what, and as whom.
#[derive(Debug, Clone, Copy)]
pub struct SubmissionRequest<'a> {
    /// The detectors to evaluate.
    pub submissions: &'a [DetectorSubmission],
    /// What was observed.
    pub inputs: &'a ObservedInputs,
    /// Scope, correlation and authority of the runs. Its execution id is the
    /// first of the consecutive ids the runs use.
    pub context: &'a RunContext,
    /// The consumer class the scheduler admits the work as.
    pub consumer: ConsumerClass,
}

/// What a submitted `(detector, language)` pair came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectorStatus {
    /// The detector produced findings.
    Found,
    /// Nothing was found, within the scope the reason states. Never "clean".
    NotObserved,
    /// Nothing was found, but checks that could not decide remain, or nothing
    /// was observed to analyse.
    Unknown,
    /// The core has no model for this detector and language: nothing was
    /// analysed. The reason is the core's.
    Unsupported,
    /// A required detector did not produce an answer.
    Inconclusive,
    /// An optional detector was refused by the scheduler and not run.
    Deferred,
}

/// One finding of a detector, in plain words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmittedFinding {
    /// The finding kind.
    pub kind: String,
    /// The evaluated risk.
    pub risk: String,
    /// The strength of the supporting evidence.
    pub evidence_class: String,
    /// The message the detector gave.
    pub message: String,
}

/// What a detector answer rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultBasis {
    /// The pack version the core publishes.
    pub pack_version: String,
    /// The language the answer is about, as asked.
    pub language: String,
    /// The snapshot the run was pinned to.
    pub snapshot: u64,
    /// The detector version; `None` when the core has none for the pair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detector_version: Option<String>,
    /// Digest of the detector's logic, so the answer names the exact rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logic_digest: Option<String>,
    /// The backend that runs the detector on the language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    /// The strongest evidence class that backend can produce.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_ceiling: Option<EvidenceClass>,
    /// The flow reach of a taint detector; `None` for structural ones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reach: Option<AnalysisReach>,
    /// How many observed inputs were analysed. Zero when nothing ran.
    pub inputs_analysed: usize,
}

/// The answer for one submitted `(detector, language)` pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorResult {
    /// The detector id, as submitted.
    pub detector: String,
    /// The language, as submitted.
    pub language: String,
    /// What the pair came to.
    pub status: DetectorStatus,
    /// Why, in the core's words.
    pub reason: String,
    /// Callees and checks that could not decide, sorted.
    #[serde(default)]
    pub unresolved: Vec<String>,
    /// The findings, when there are any.
    #[serde(default)]
    pub findings: Vec<SubmittedFinding>,
    /// How many findings clear the family's blocking gate. Always zero for a
    /// candidate detector.
    pub blocking: usize,
    /// Notes qualifying the answer, as `code: message`.
    #[serde(default)]
    pub diagnostics: Vec<String>,
    /// What the answer rests on.
    pub basis: ResultBasis,
    /// What the run read; `None` when nothing ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_scope: Option<ReadScope>,
}

/// The answers to a submission, one per requested pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionReport {
    /// The schema this value was written under.
    pub schema: String,
    /// The pack version the core publishes.
    pub pack_version: String,
    /// One answer per `(detector, language)` pair, in the order asked.
    pub results: Vec<DetectorResult>,
}

/// Evaluates the submitted detectors through `coordinator` and `pack`.
///
/// `store` receives the evidence of the runs that execute. Every requested
/// pair is answered; see the module documentation for what each status
/// means. A submission naming no language is answered once, as unsupported,
/// under the empty language.
pub fn submit_detectors<S>(
    pack: &SecurityPack,
    coordinator: &Coordinator,
    request: &SubmissionRequest<'_>,
    store: &mut S,
) -> SubmissionReport
where
    S: EvidenceSink + EvidenceLookup,
{
    let matrix = pack.capability_matrix();
    let mut next_execution = request.context.execution_id.get();
    let mut results = Vec::new();
    for submission in request.submissions {
        let languages: Vec<&str> = if submission.languages.is_empty() {
            vec![""]
        } else {
            submission.languages.iter().map(String::as_str).collect()
        };
        for language in languages {
            let result = match resolve(&matrix, submission, language) {
                Err(reason) => unsupported(&matrix, request, submission, language, reason),
                Ok(target) => {
                    let selected = Selected::of(&target, request.inputs);
                    answer_pair(
                        pack,
                        coordinator,
                        request,
                        submission,
                        &target,
                        selected,
                        &mut next_execution,
                        store,
                    )
                }
            };
            results.push(result);
        }
    }
    SubmissionReport {
        schema: DETECTOR_SUBMISSION_SCHEMA.to_string(),
        pack_version: matrix.pack_version,
        results,
    }
}

/// A pair the matrix supports: what runs it and what it rests on.
struct Target {
    family: SecurityFamily,
    language: SourceLanguage,
    reach: Option<AnalysisReach>,
    backend: String,
    detector_version: String,
    logic_digest: String,
    evidence_ceiling: EvidenceClass,
}

/// Resolves a catalog reference for one language against the matrix, or says
/// why the core cannot run it.
fn resolve(
    matrix: &CapabilityMatrix,
    submission: &DetectorSubmission,
    language: &str,
) -> Result<Target, String> {
    let Some(family) = SecurityFamily::ALL
        .into_iter()
        .find(|family| family.finding_kind() == submission.id)
    else {
        let mut known: Vec<&str> = SecurityFamily::ALL
            .iter()
            .map(|family| family.finding_kind())
            .collect();
        known.sort_unstable();
        return Err(format!(
            "the catalog has no detector `{}` (it publishes: {})",
            submission.id,
            known.join(", ")
        ));
    };
    if submission.pack_version != matrix.pack_version {
        return Err(format!(
            "detector `{}` was requested at pack version `{}` but this core publishes pack \
             version `{}`",
            submission.id, submission.pack_version, matrix.pack_version
        ));
    }
    if language.is_empty() {
        return Err(format!(
            "the submission of `{}` names no language",
            submission.id
        ));
    }
    let Some(source) = SourceLanguage::ALL
        .into_iter()
        .find(|candidate| candidate.name() == language)
    else {
        let known: Vec<&str> = SourceLanguage::ALL.iter().map(|l| l.name()).collect();
        return Err(format!(
            "`{language}` is not a language the catalog names (it names: {})",
            known.join(", ")
        ));
    };
    match matrix.get(source, family).map(|cell| &cell.support) {
        Some(Support::Supported {
            backend,
            evidence_ceiling,
            detector_version,
            logic_digest,
            reach,
            ..
        }) => Ok(Target {
            family,
            language: source,
            reach: *reach,
            backend: backend.clone(),
            detector_version: detector_version.clone(),
            logic_digest: logic_digest.clone(),
            evidence_ceiling: *evidence_ceiling,
        }),
        Some(Support::Unsupported { reason }) => Err(format!(
            "detector `{}` is not supported for {language}: {reason}",
            submission.id
        )),
        None => Err(format!(
            "detector `{}` is not supported for {language}: the capability matrix has no entry \
             for it",
            submission.id
        )),
    }
}

/// The observed inputs one pair analyses: those of its language, of the kind
/// its family reads, with the family they are analysed under.
enum Selected<'a> {
    Functions(Weakness, Vec<&'a ObservedFunction>),
    Units(StructuralFamily, Vec<&'a ObservedUnit>),
}

impl<'a> Selected<'a> {
    fn of(target: &Target, inputs: &'a ObservedInputs) -> Self {
        match (target.family.weakness(), target.family.structural()) {
            (Some(weakness), _) => Self::Functions(
                weakness,
                inputs
                    .functions
                    .iter()
                    .filter(|function| function.language == target.language)
                    .collect(),
            ),
            (None, Some(family)) => Self::Units(
                family,
                inputs
                    .units
                    .iter()
                    .filter(|unit| unit.language == target.language)
                    .collect(),
            ),
            (None, None) => unreachable!("every family is taint or structural"),
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Functions(_, functions) => functions.len(),
            Self::Units(_, units) => units.len(),
        }
    }

    fn noun(&self) -> &'static str {
        match self {
            Self::Functions(..) => "function",
            Self::Units(..) => "unit",
        }
    }

    /// What analysing these inputs reads: a function by name within its file,
    /// a unit as a whole file.
    fn reads(&self) -> Vec<ScopedRead> {
        match self {
            Self::Functions(_, functions) => functions
                .iter()
                .map(|function| ScopedRead {
                    path: function.path.clone(),
                    symbol: Some(function.id.clone()),
                })
                .collect(),
            Self::Units(_, units) => units
                .iter()
                .map(|unit| ScopedRead {
                    path: unit.path.clone(),
                    symbol: None,
                })
                .collect(),
        }
    }
}

/// What the scheduled computation hands back: plain data, so the scheduler
/// can share it between requests for the same work.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Answer {
    status: DetectorStatus,
    reason: String,
    unresolved: Vec<String>,
    findings: Vec<SubmittedFinding>,
    blocking: usize,
    diagnostics: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
fn answer_pair<S>(
    pack: &SecurityPack,
    coordinator: &Coordinator,
    request: &SubmissionRequest<'_>,
    submission: &DetectorSubmission,
    target: &Target,
    selected: Selected<'_>,
    next_execution: &mut u64,
    store: &mut S,
) -> DetectorResult
where
    S: EvidenceSink + EvidenceLookup,
{
    let analysed = selected.len();
    if analysed == 0 {
        let reason = format!(
            "no observed {} of language {} was submitted: nothing was analysed",
            selected.noun(),
            target.language.name()
        );
        return result_of(
            request,
            submission,
            target,
            0,
            Answer {
                status: DetectorStatus::Unknown,
                unresolved: vec![reason.clone()],
                reason,
                findings: Vec::new(),
                blocking: 0,
                diagnostics: Vec::new(),
            },
            None,
        );
    }

    let scheduled = schedule_request(request, submission, target, analysed);
    let outcome = coordinator.run(&scheduled, DaemonProbe::ABSENT, |_cancelled| {
        let runs = analyse(pack, request.context, &selected, next_execution, store)
            .map_err(|error| error.to_string())?;
        Ok(Computed {
            output: CheckOutput::Pass(combine(&runs)),
            measured_units: analysed as u64,
        })
    });

    match answer_of(outcome.status, submission.required) {
        Ok(answer) => {
            let scope = ReadScope::from_scoped_reads(selected.reads(), ReadBreadth::Pointwise);
            result_of(request, submission, target, analysed, answer, Some(scope))
        }
        Err((status, reason)) => result_of(
            request,
            submission,
            target,
            0,
            Answer {
                status,
                reason,
                unresolved: Vec::new(),
                findings: Vec::new(),
                blocking: 0,
                diagnostics: Vec::new(),
            },
            None,
        ),
    }
}

/// The scheduler's identity of the pair: the detector and language as the work,
/// the snapshot the run is pinned to, and the capabilities the detector needs.
fn schedule_request(
    request: &SubmissionRequest<'_>,
    submission: &DetectorSubmission,
    target: &Target,
    analysed: usize,
) -> ScheduleRequest {
    let work = NamespacedName::new(format!(
        "catalog.{}.{}",
        target.family.finding_kind(),
        target.language.name()
    ))
    .map(WorkId::new)
    .expect("a finding kind and a language name form a namespaced work id");
    let capabilities = target
        .family
        .detector(target.reach.unwrap_or(AnalysisReach::Intraprocedural))
        .requires;
    let snapshot = request.context.scope.snapshot;
    ScheduleRequest {
        consumer: request.consumer,
        key: WorkKey::new(work, snapshot, capabilities),
        revision: RevisionTag::new(format!("snapshot-{}", snapshot.0)),
        required: submission.required,
        direct_units: analysed as u64,
    }
}

/// Runs the family over every selected input, each under its own execution
/// id. The first failed run fails the whole computation: no verdict is
/// invented for the inputs after it.
fn analyse<S>(
    pack: &SecurityPack,
    context: &RunContext,
    selected: &Selected<'_>,
    next_execution: &mut u64,
    store: &mut S,
) -> Result<Vec<PackRun>, PackError>
where
    S: EvidenceSink + EvidenceLookup,
{
    let mut run_context = || {
        let scoped = RunContext {
            execution_id: ExecutionId::new(*next_execution),
            ..context.clone()
        };
        *next_execution += 1;
        scoped
    };
    match selected {
        Selected::Functions(weakness, functions) => functions
            .iter()
            .map(|function| pack.analyse_taint(*weakness, function, &run_context(), store))
            .collect(),
        Selected::Units(family, units) => units
            .iter()
            .map(|unit| pack.analyse_structural(*family, unit, &run_context(), store))
            .collect(),
    }
}

/// The weakest honest summary of several runs of one family.
///
/// Any finding makes the answer `found`. Otherwise any run that could not
/// decide, or that the family does not model, makes it `unknown` while some
/// other run did decide; when every run was unmodelled it is `unsupported`;
/// and only when every run decided nothing was found is it `not_observed`.
fn combine(runs: &[PackRun]) -> Answer {
    let findings: Vec<SubmittedFinding> = runs
        .iter()
        .flat_map(|run| &run.findings)
        .map(|finding| SubmittedFinding {
            kind: finding.kind.as_str().to_string(),
            risk: word(&finding.risk),
            evidence_class: word(&finding.evidence_class),
            message: finding.message.clone(),
        })
        .collect();
    let blocking = runs.iter().map(|run| run.blocking).sum();
    let diagnostics = runs
        .iter()
        .flat_map(|run| &run.diagnostics)
        .map(|d| format!("{}: {}", d.code, d.message))
        .collect();

    let mut unresolved = BTreeSet::new();
    let mut within = BTreeSet::new();
    let mut unmodelled = BTreeSet::new();
    for run in runs {
        match &run.verdict {
            PackVerdict::Found { .. } => {}
            PackVerdict::NotObserved { within: scope } => {
                within.insert(scope.clone());
            }
            PackVerdict::Unknown {
                unresolved: callees,
            } => unresolved.extend(callees.clone()),
            PackVerdict::Unsupported { reason } => {
                unmodelled.insert(reason.clone());
            }
        }
    }

    let (status, reason, unresolved) = if !findings.is_empty() {
        (
            DetectorStatus::Found,
            format!("{} finding(s)", findings.len()),
            Vec::new(),
        )
    } else if !unresolved.is_empty() || (!unmodelled.is_empty() && !within.is_empty()) {
        let mut open: Vec<String> = unresolved.into_iter().collect();
        open.extend(
            unmodelled
                .iter()
                .map(|reason| format!("unsupported: {reason}")),
        );
        (
            DetectorStatus::Unknown,
            format!("{} check(s) could not decide", open.len()),
            open,
        )
    } else if !unmodelled.is_empty() {
        (
            DetectorStatus::Unsupported,
            unmodelled.into_iter().collect::<Vec<_>>().join("; "),
            Vec::new(),
        )
    } else {
        (
            DetectorStatus::NotObserved,
            within.into_iter().collect::<Vec<_>>().join("; "),
            Vec::new(),
        )
    };
    Answer {
        status,
        reason,
        unresolved,
        findings,
        blocking,
        diagnostics,
    }
}

/// What the scheduler's status makes of a pair: the computed answer, or why
/// there is none. Nothing the scheduler refused is ever an answer.
fn answer_of(
    status: RunStatus<Answer>,
    required: bool,
) -> Result<Answer, (DetectorStatus, String)> {
    let not_run = |reason: String| {
        let status = if required {
            DetectorStatus::Inconclusive
        } else {
            DetectorStatus::Deferred
        };
        Err((status, reason))
    };
    match status {
        RunStatus::Pass(answer) => Ok(answer),
        RunStatus::NotPass => Err((
            DetectorStatus::Inconclusive,
            "the scheduler reported the run as not passing".to_string(),
        )),
        RunStatus::Cancelled => Err((
            DetectorStatus::Inconclusive,
            "the revision was cancelled before the detector answered".to_string(),
        )),
        RunStatus::Deferred => not_run(
            "the scheduler refused the optional run because of the budget or the queue; it was \
             not run"
                .to_string(),
        ),
        RunStatus::Inconclusive(InconclusiveReason::BudgetExhausted {
            attempted,
            remaining,
        }) => Err((
            DetectorStatus::Inconclusive,
            format!(
                "the declared budget could not cover the run (it needs {attempted} unit(s), \
                 {remaining} remain)"
            ),
        )),
        RunStatus::Inconclusive(InconclusiveReason::QueueFull) => Err((
            DetectorStatus::Inconclusive,
            "the scheduler queue was full for this consumer class".to_string(),
        )),
        RunStatus::Inconclusive(InconclusiveReason::ComputeFailed(error)) => Err((
            DetectorStatus::Inconclusive,
            format!("the detector run failed: {error}"),
        )),
    }
}

fn result_of(
    request: &SubmissionRequest<'_>,
    submission: &DetectorSubmission,
    target: &Target,
    inputs_analysed: usize,
    answer: Answer,
    read_scope: Option<ReadScope>,
) -> DetectorResult {
    DetectorResult {
        detector: submission.id.clone(),
        language: target.language.name().to_string(),
        status: answer.status,
        reason: answer.reason,
        unresolved: answer.unresolved,
        findings: answer.findings,
        blocking: answer.blocking,
        diagnostics: answer.diagnostics,
        basis: ResultBasis {
            pack_version: submission.pack_version.clone(),
            language: target.language.name().to_string(),
            snapshot: request.context.scope.snapshot.0,
            detector_version: Some(target.detector_version.clone()),
            logic_digest: Some(target.logic_digest.clone()),
            backend: Some(target.backend.clone()),
            evidence_ceiling: Some(target.evidence_ceiling),
            reach: target.reach,
            inputs_analysed,
        },
        read_scope,
    }
}

/// The answer for a pair the core cannot run: unsupported, with the core's
/// reason, and a basis that claims nothing about a detector that did not run.
fn unsupported(
    matrix: &CapabilityMatrix,
    request: &SubmissionRequest<'_>,
    submission: &DetectorSubmission,
    language: &str,
    reason: String,
) -> DetectorResult {
    DetectorResult {
        detector: submission.id.clone(),
        language: language.to_string(),
        status: DetectorStatus::Unsupported,
        reason,
        unresolved: Vec::new(),
        findings: Vec::new(),
        blocking: 0,
        diagnostics: Vec::new(),
        basis: ResultBasis {
            pack_version: matrix.pack_version.clone(),
            language: language.to_string(),
            snapshot: request.context.scope.snapshot.0,
            detector_version: None,
            logic_digest: None,
            backend: None,
            evidence_ceiling: None,
            reach: None,
            inputs_analysed: 0,
        },
        read_scope: None,
    }
}

/// The wire word of a value that serialises to a string.
fn word<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(verdict: PackVerdict) -> PackRun {
        PackRun {
            family: SecurityFamily::SqlInjection,
            verdict,
            findings: Vec::new(),
            blocking: 0,
            diagnostics: Vec::new(),
            backend: None,
        }
    }

    fn not_observed() -> PackRun {
        run(PackVerdict::NotObserved {
            within: "intraprocedural flow".to_string(),
        })
    }

    fn unknown(callee: &str) -> PackRun {
        run(PackVerdict::Unknown {
            unresolved: vec![callee.to_string()],
        })
    }

    fn unsupported() -> PackRun {
        run(PackVerdict::Unsupported {
            reason: "no model".to_string(),
        })
    }

    #[test]
    fn every_run_deciding_nothing_found_is_not_observed() {
        let answer = combine(&[not_observed(), not_observed()]);
        assert_eq!(answer.status, DetectorStatus::NotObserved);
        assert_eq!(answer.reason, "intraprocedural flow");
    }

    #[test]
    fn one_undecided_run_makes_the_whole_answer_unknown_not_clean() {
        let answer = combine(&[
            not_observed(),
            unknown("helper::run"),
            unknown("helper::run"),
        ]);
        assert_eq!(answer.status, DetectorStatus::Unknown);
        assert_eq!(answer.unresolved, vec!["helper::run".to_string()]);
    }

    #[test]
    fn an_unmodelled_run_next_to_a_decided_one_is_unknown_and_alone_is_unsupported() {
        let mixed = combine(&[not_observed(), unsupported()]);
        assert_eq!(mixed.status, DetectorStatus::Unknown);
        assert_eq!(mixed.unresolved, vec!["unsupported: no model".to_string()]);

        let alone = combine(&[unsupported(), unsupported()]);
        assert_eq!(alone.status, DetectorStatus::Unsupported);
        assert_eq!(alone.reason, "no model");
    }

    #[test]
    fn nothing_the_scheduler_refused_is_ever_an_answer() {
        let refusals = [
            RunStatus::NotPass,
            RunStatus::Cancelled,
            RunStatus::Deferred,
            RunStatus::Inconclusive(InconclusiveReason::QueueFull),
            RunStatus::Inconclusive(InconclusiveReason::ComputeFailed("boom".to_string())),
        ];
        for status in refusals {
            let (required, _) = answer_of(status.clone(), true).expect_err("not an answer");
            assert_ne!(required, DetectorStatus::NotObserved);
            assert!(matches!(
                required,
                DetectorStatus::Inconclusive | DetectorStatus::Deferred
            ));
        }
        let (optional, _) = answer_of(RunStatus::Deferred, false).expect_err("not an answer");
        assert_eq!(optional, DetectorStatus::Deferred);
        let (required, _) = answer_of(RunStatus::Deferred, true).expect_err("not an answer");
        assert_eq!(required, DetectorStatus::Inconclusive);
    }

    #[test]
    fn a_reference_is_resolved_against_the_matrix_and_refused_with_a_reason() {
        let matrix = SecurityPack::standard().capability_matrix();
        let submission = |id: &str, version: &str| DetectorSubmission {
            id: id.to_string(),
            pack_version: version.to_string(),
            languages: Vec::new(),
            required: true,
        };
        let sql = SecurityFamily::SqlInjection.finding_kind();

        let target = resolve(&matrix, &submission(sql, &matrix.pack_version), "rust")
            .expect("rust is modelled for sql injection");
        assert_eq!(target.language, SourceLanguage::Rust);
        assert_eq!(target.backend, "m5_dataflow");

        let unknown_id = resolve(&matrix, &submission("nope", &matrix.pack_version), "rust")
            .err()
            .expect("unknown detector");
        assert!(unknown_id.contains("no detector `nope`"));
        let stale = resolve(&matrix, &submission(sql, "0.0.1"), "rust")
            .err()
            .expect("stale pack version");
        assert!(stale.contains("0.0.1") && stale.contains(&matrix.pack_version));
        let unnamed = resolve(&matrix, &submission(sql, &matrix.pack_version), "cobol")
            .err()
            .expect("unknown language");
        assert!(unnamed.contains("cobol"));
    }
}
