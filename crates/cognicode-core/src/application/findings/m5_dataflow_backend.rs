//! M5 dataflow backend (M6, cycle e60).
//!
//! `DetectorBackend` (domain) implemented in the **application** layer on top
//! of [`ProgramAnalysisService::taint_flow`](crate::application::program_analysis::ProgramAnalysisService::taint_flow).
//! It owns the IR → engine translation and imports **no** second taint engine:
//! M5 stays the analysis authority, M6 only interprets the detector's intent.
//!
//! ## Mapping
//!
//! | IR | M5 request |
//! |----|-----------|
//! | `MATCH <subject>` | source sites |
//! | `FLOW <a> -> <b>` | `a` → source sites, `b` → sink sites |
//! | `EXCLUDE <x>` | `x` → untaint sites |
//!
//! Reachability is **not** recomputed here: whatever M5 returns is what M6
//! reports. An eliminated (sanitised) path cannot reappear.
//!
//! Capabilities: `{Dataflow}` — **not** `{GraphQuery, Dataflow}`: the
//! capability describes what the backend offers to the Detector IR, not the
//! techniques it uses internally.
//!
//! Evidence ceiling: `B` (strong static evidence *within the declared model*:
//! intra-procedural, statement-level, not variable-precise sanitisation).
//!
//! Engine errors stay errors: `AnalyticsError → BackendError::Analysis →
//! ExecutionError::Backend`, never an empty outcome.

use std::collections::{BTreeMap, BTreeSet};

use super::taint_runner::TaintFlowRunner;
use crate::application::program_analysis::{TaintFlowPath, TaintFlowRequest, TaintFlowStatement};
use crate::domain::findings::{
    AdmissionSource, AdmittedDetector, AnalysisCapability, AnalysisInput, BackendError,
    CausalObservation, CausalStepKind, DataflowFunction, DataflowStatement, DetectorBackend,
    DetectorDiagnostic, DetectorMatch, DetectorOutcome, EvidenceClass, EvidenceKind, FindingKind,
    ProducedEvidence, SubjectPattern,
};
use crate::domain::plan::limits::PlanLimits;

/// Default limits for a dataflow run (bounded by design D5).
fn default_limits() -> PlanLimits {
    PlanLimits {
        time_ms: Some(30_000),
        cancellation: None,
        max_depth: None,
        max_hops: None,
        max_visited_nodes: Some(1_000_000),
        max_visited_edges: None,
        max_result_rows: Some(100_000),
        max_path_count: None,
        max_memory_bytes: Some(512 * 1024 * 1024),
    }
}

/// A `DetectorBackend` over the M5 forward-taint engine.
pub struct M5DataflowBackend<R> {
    runner: R,
    limits: PlanLimits,
}

impl<R: TaintFlowRunner> M5DataflowBackend<R> {
    /// Construct the backend over a runner (production: `ProgramAnalysisService`).
    pub fn new(runner: R) -> Self {
        Self {
            runner,
            limits: default_limits(),
        }
    }

    /// Construct with explicit limits.
    pub fn with_limits(runner: R, limits: PlanLimits) -> Self {
        Self { runner, limits }
    }

    /// Build the M5 request for one function.
    fn request_for(
        &self,
        function: &DataflowFunction,
        source_subjects: &[&SubjectPattern],
        sink_subject: &SubjectPattern,
        untaint_subjects: &[&SubjectPattern],
    ) -> TaintFlowRequest {
        let sources = site_ids(function, |s| s.has_any_subject(source_subjects));
        let sinks = site_ids(function, |s| s.subjects.contains(sink_subject));
        let untaints = site_ids(function, |s| s.has_any_subject(untaint_subjects));

        let dfg_digest = crate::domain::findings::digest::sha256_hex(
            serde_json::to_string(&function.statements)
                .unwrap_or_default()
                .as_bytes(),
        );

        TaintFlowRequest {
            function_id: function.id.clone(),
            dfg_digest,
            statements: function
                .statements
                .iter()
                .map(|s| TaintFlowStatement {
                    id: s.id as usize,
                    kind: "stmt".to_string(),
                    defs: s.defs.clone(),
                    uses: s.uses.clone(),
                })
                .collect(),
            sources,
            sinks,
            untaints,
        }
    }
}

/// Ids of statements matching `pred`, in id order.
fn site_ids(function: &DataflowFunction, pred: impl Fn(&DataflowStatement) -> bool) -> Vec<usize> {
    let mut ids: Vec<usize> = function
        .statements
        .iter()
        .filter(|s| pred(s))
        .map(|s| s.id as usize)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

/// Render a statement id as `subject @ path:line` (or `statement N`).
fn render(by_id: &BTreeMap<u64, &DataflowStatement>, id: u64) -> String {
    match by_id.get(&id) {
        Some(s) => {
            let subject = s
                .subjects
                .iter()
                .next()
                .map(|x| x.to_string())
                .unwrap_or_else(|| "stmt".to_string());
            format!("{} @ {}:{}", subject, s.location.path, s.location.line)
        }
        None => format!("statement {id}"),
    }
}

impl<R: TaintFlowRunner> DetectorBackend for M5DataflowBackend<R> {
    fn name(&self) -> &'static str {
        "m5_dataflow"
    }

    fn capabilities(&self) -> BTreeSet<AnalysisCapability> {
        // What it offers to the Detector IR — not the techniques it uses
        // internally (it builds a DFG, but that is not a GraphQuery offer).
        [AnalysisCapability::Dataflow].into_iter().collect()
    }

    fn evidence_ceiling(&self) -> EvidenceClass {
        EvidenceClass::B
    }

    fn run(
        &self,
        admitted: &AdmittedDetector,
        input: &AnalysisInput,
    ) -> Result<DetectorOutcome, BackendError> {
        let dataflow = input
            .dataflow
            .as_ref()
            .ok_or(BackendError::MissingInput("dataflow"))?;

        let mut match_subjects: Vec<&SubjectPattern> = Vec::new();
        let mut flows: Vec<(&SubjectPattern, &SubjectPattern)> = Vec::new();
        let mut excludes: Vec<&SubjectPattern> = Vec::new();
        let mut produce = None;
        for step in &admitted.definition.steps {
            match step {
                crate::domain::findings::DetectorStep::Match { subject } => {
                    match_subjects.push(subject)
                }
                crate::domain::findings::DetectorStep::Flow { source, sink, .. } => {
                    flows.push((source, sink))
                }
                crate::domain::findings::DetectorStep::Exclude { path_contains } => {
                    excludes.push(path_contains)
                }
                crate::domain::findings::DetectorStep::Produce { kind } => produce = Some(kind),
                crate::domain::findings::DetectorStep::Verify { .. } => {}
            }
        }

        let produce =
            produce.ok_or_else(|| BackendError::Internal("detector has no PRODUCE step".into()))?;
        let (flow_source, flow_sink) = match flows.as_slice() {
            [] => {
                return Ok(DetectorOutcome {
                    produced_evidence: vec![],
                    matches: vec![],
                    diagnostics: vec![DetectorDiagnostic {
                        code: "no_flow_steps".to_string(),
                        message: "detector declares no FLOW step; nothing to analyse".to_string(),
                    }],
                });
            }
            [single] => *single,
            many => {
                return Err(BackendError::UnsupportedIr(format!(
                    "detector declares {} FLOW steps; multi-FLOW semantics are not defined yet",
                    many.len()
                )));
            }
        };

        // Source sites: MATCH subjects plus the FLOW source subject.
        let mut source_subjects: Vec<&SubjectPattern> = match_subjects;
        source_subjects.push(flow_source);

        let mut outcome = DetectorOutcome::empty();
        for function in &dataflow.functions {
            let request = self.request_for(function, &source_subjects, flow_sink, &excludes);
            if request.sources.is_empty() || request.sinks.is_empty() {
                outcome.diagnostics.push(DetectorDiagnostic {
                    code: "insufficient_sites".to_string(),
                    message: format!(
                        "function `{}` has {} source site(s) and {} sink site(s); skipping",
                        function.id,
                        request.sources.len(),
                        request.sinks.len()
                    ),
                });
                continue;
            }

            // M5 is the authority on reachability: engine errors stay errors.
            let result = self
                .runner
                .taint_flow(&request, &self.limits)
                .map_err(|e| BackendError::Analysis(format!("taint_flow: {e}")))?;

            let by_id: BTreeMap<u64, &DataflowStatement> =
                function.statements.iter().map(|s| (s.id, s)).collect();

            for path in &result.paths {
                outcome.produced_evidence.push(evidence_for(
                    function,
                    path,
                    &by_id,
                    flow_source,
                    flow_sink,
                ));
                let evidence_index = outcome.produced_evidence.len() - 1;

                let mut causal = Vec::with_capacity(path.intermediates.len() + 2);
                causal.push(CausalObservation {
                    kind: CausalStepKind::Source,
                    detail: render(&by_id, path.source as u64),
                    subject: None,
                    fact: None,
                    evidence: Some(evidence_index),
                });
                for mid in &path.intermediates {
                    causal.push(CausalObservation {
                        kind: CausalStepKind::Flow,
                        detail: render(&by_id, *mid as u64),
                        subject: None,
                        fact: None,
                        evidence: Some(evidence_index),
                    });
                }
                causal.push(CausalObservation {
                    kind: CausalStepKind::Sink,
                    detail: render(&by_id, path.sink as u64),
                    subject: None,
                    fact: None,
                    evidence: Some(evidence_index),
                });

                outcome.matches.push(DetectorMatch {
                    kind: produce.clone(),
                    message: format!(
                        "{} reached {} in `{}` ({} hop path)",
                        flow_source,
                        flow_sink,
                        function.id,
                        path.intermediates.len() + 1
                    ),
                    evidence: vec![evidence_index],
                    causal,
                });
            }
        }

        Ok(outcome)
    }
}

fn evidence_for(
    function: &DataflowFunction,
    path: &TaintFlowPath,
    by_id: &BTreeMap<u64, &DataflowStatement>,
    flow_source: &SubjectPattern,
    flow_sink: &SubjectPattern,
) -> ProducedEvidence {
    let mut rendered: Vec<String> = Vec::new();
    rendered.push(render(by_id, path.source as u64));
    for mid in &path.intermediates {
        rendered.push(render(by_id, *mid as u64));
    }
    rendered.push(render(by_id, path.sink as u64));
    ProducedEvidence {
        kind: EvidenceKind::DataflowPath,
        detail: format!(
            "{} -> {} in `{}`: {}",
            flow_source,
            flow_sink,
            function.id,
            rendered.join(" -> ")
        ),
        subject: None,
        fact: None,
    }
}

/// Marker used by tests / callers that only need the default source.
pub fn default_admission_source() -> AdmissionSource {
    AdmissionSource::HumanCurated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::program_analysis::{
        ProgramAnalysisService, TaintFlowPath, TaintFlowRequest, TaintFlowResult,
    };
    use crate::domain::analytics::descriptor::AnalyticsError;
    use crate::domain::findings::admission::{AdmissionSource, DetectorAdmission, ExecutionPermit};
    use crate::domain::findings::detector_ir::{
        DetectorFindingPolicy, DetectorId, DetectorIr, DetectorStep, FindingKind,
    };
    use crate::domain::findings::{
        DataflowFunction, DataflowInput, DataflowLocation, DataflowStatement, DetectorAuthority,
    };
    use std::sync::Mutex;

    /// Counting spy: records every request it receives.
    struct CountingRunner {
        calls: Mutex<Vec<TaintFlowRequest>>,
    }

    impl CountingRunner {
        fn new() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
            }
        }
        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
        fn last_request(&self) -> TaintFlowRequest {
            self.calls.lock().unwrap().last().cloned().expect("no call")
        }
    }

    impl TaintFlowRunner for CountingRunner {
        fn taint_flow(
            &self,
            request: &TaintFlowRequest,
            _limits: &PlanLimits,
        ) -> Result<TaintFlowResult, AnalyticsError> {
            self.calls.lock().unwrap().push(request.clone());
            // Fixed answer: source -> mid -> sink.
            Ok(TaintFlowResult {
                paths: vec![TaintFlowPath {
                    source: 1,
                    sink: 3,
                    intermediates: vec![2],
                }],
                tainted_statements: vec![1, 2, 3],
                untaint_statements: vec![],
            })
        }
    }

    fn stmt(
        id: u64,
        subjects: &[&str],
        line: u32,
        defs: &[&str],
        uses: &[&str],
    ) -> DataflowStatement {
        DataflowStatement {
            id,
            defs: defs.iter().map(|v| v.to_string()).collect(),
            uses: uses.iter().map(|v| v.to_string()).collect(),
            subjects: subjects
                .iter()
                .map(|s| SubjectPattern::new(*s).unwrap())
                .collect(),
            location: DataflowLocation {
                path: "src/handler.rs".to_string(),
                line,
            },
        }
    }

    /// A real def/use chain: `a = input(); b = f(a); sink(b)`.
    fn chained_statements() -> Vec<DataflowStatement> {
        vec![
            stmt(1, &["security.user_input"], 10, &["a"], &[]),
            stmt(2, &[], 11, &["b"], &["a"]),
            stmt(3, &["security.sql_execution"], 12, &[], &["b"]),
        ]
    }

    fn ir() -> DetectorIr {
        DetectorIr {
            id: DetectorId::new("security.sql_injection").unwrap(),
            name: "sql injection".to_string(),
            policy: DetectorFindingPolicy::default(),
            requires: [AnalysisCapability::Dataflow].into_iter().collect(),
            authority: DetectorAuthority::Candidate,
            steps: vec![
                DetectorStep::Match {
                    subject: SubjectPattern::new("security.user_input").unwrap(),
                },
                DetectorStep::Flow {
                    source: SubjectPattern::new("security.user_input").unwrap(),
                    sink: SubjectPattern::new("security.sql_execution").unwrap(),
                    max_hops: None,
                },
                DetectorStep::Exclude {
                    path_contains: SubjectPattern::new("security.sanitizer").unwrap(),
                },
                DetectorStep::Produce {
                    kind: FindingKind::new("security.sql_injection").unwrap(),
                },
            ],
        }
    }

    fn permit() -> ExecutionPermit {
        DetectorAdmission::admit(ir(), "1.0.0", AdmissionSource::HumanCurated).unwrap()
    }

    fn input_with(statements: Vec<DataflowStatement>) -> AnalysisInput {
        AnalysisInput {
            ast: None,
            graph: None,
            dataflow: Some(DataflowInput {
                functions: vec![DataflowFunction {
                    id: "handler".to_string(),
                    statements,
                }],
            }),
        }
    }

    #[test]
    fn capabilities_and_ceiling_are_dataflow_only() {
        let backend = M5DataflowBackend::new(CountingRunner::new());
        assert_eq!(backend.name(), "m5_dataflow");
        assert_eq!(
            backend.capabilities(),
            [AnalysisCapability::Dataflow].into_iter().collect(),
            "it must NOT advertise GraphQuery"
        );
        assert_eq!(backend.evidence_ceiling(), EvidenceClass::B);
    }

    #[test]
    fn calls_the_m5_engine_once_with_the_expected_sites() {
        let backend = M5DataflowBackend::new(CountingRunner::new());
        let statements = chained_statements();
        let outcome = backend
            .run(permit().admitted(), &input_with(statements))
            .unwrap();

        assert_eq!(backend.runner.call_count(), 1, "M5 must be invoked");
        let request = backend.runner.last_request();
        assert_eq!(request.function_id, "handler");
        assert_eq!(request.sources, vec![1], "MATCH/FLOW source = statement 1");
        assert_eq!(request.sinks, vec![3], "FLOW sink = statement 3");
        assert!(request.untaints.is_empty());
        assert_eq!(request.statements.len(), 3);
        assert!(!request.dfg_digest.is_empty());

        assert_eq!(outcome.matches.len(), 1);
        assert_eq!(
            outcome.produced_evidence[0].kind,
            EvidenceKind::DataflowPath
        );
    }

    #[test]
    fn maps_untaints_to_excluded_subjects() {
        let backend = M5DataflowBackend::new(CountingRunner::new());
        let statements = vec![
            stmt(1, &["security.user_input"], 10, &["a"], &[]),
            stmt(2, &["security.sanitizer"], 11, &["b"], &["a"]),
            stmt(3, &["security.sql_execution"], 12, &[], &["b"]),
        ];
        backend
            .run(permit().admitted(), &input_with(statements))
            .unwrap();
        assert_eq!(backend.runner.last_request().untaints, vec![2]);
    }

    #[test]
    fn builds_source_flow_sink_causal_chain() {
        let backend = M5DataflowBackend::new(CountingRunner::new());
        let statements = chained_statements();
        let outcome = backend
            .run(permit().admitted(), &input_with(statements))
            .unwrap();
        let kinds: Vec<CausalStepKind> = outcome.matches[0].causal.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            vec![
                CausalStepKind::Source,
                CausalStepKind::Flow,
                CausalStepKind::Sink
            ]
        );
        assert!(
            outcome.matches[0]
                .causal
                .iter()
                .all(|c| c.evidence == Some(0))
        );
    }

    #[test]
    fn engine_errors_stay_errors() {
        struct FailingRunner;
        impl TaintFlowRunner for FailingRunner {
            fn taint_flow(
                &self,
                _request: &TaintFlowRequest,
                _limits: &PlanLimits,
            ) -> Result<TaintFlowResult, AnalyticsError> {
                Err(AnalyticsError::Internal("engine exploded".to_string()))
            }
        }
        let backend = M5DataflowBackend::new(FailingRunner);
        let statements = vec![
            stmt(1, &["security.user_input"], 10, &["a"], &[]),
            stmt(3, &["security.sql_execution"], 12, &[], &["a"]),
        ];
        let err = backend
            .run(permit().admitted(), &input_with(statements))
            .unwrap_err();
        assert!(
            matches!(err, BackendError::Analysis(_)),
            "an engine error must not degrade into an empty outcome, got {err:?}"
        );
    }

    #[test]
    fn missing_dataflow_input_fails_loud() {
        let backend = M5DataflowBackend::new(CountingRunner::new());
        let err = backend
            .run(
                permit().admitted(),
                &AnalysisInput {
                    ast: None,
                    graph: None,
                    dataflow: None,
                },
            )
            .unwrap_err();
        assert_eq!(err, BackendError::MissingInput("dataflow"));
    }

    #[test]
    fn multiple_flow_steps_fail_loud() {
        let mut ir = ir();
        ir.steps.insert(
            2,
            DetectorStep::Flow {
                source: SubjectPattern::new("security.user_input").unwrap(),
                sink: SubjectPattern::new("security.user_input").unwrap(),
                max_hops: None,
            },
        );
        let permit = DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::HumanCurated).unwrap();
        let backend = M5DataflowBackend::new(CountingRunner::new());
        let statements = vec![
            stmt(1, &["security.user_input"], 10, &["a"], &[]),
            stmt(3, &["security.sql_execution"], 12, &[], &["a"]),
        ];
        let err = backend
            .run(permit.admitted(), &input_with(statements))
            .unwrap_err();
        assert!(matches!(err, BackendError::UnsupportedIr(_)));
    }

    #[test]
    fn real_program_analysis_service_is_used_end_to_end() {
        // Composition check with the real M5 service (no spy).
        let backend = M5DataflowBackend::new(ProgramAnalysisService::new());
        let statements = chained_statements();
        let outcome = backend
            .run(permit().admitted(), &input_with(statements))
            .unwrap();
        assert_eq!(outcome.matches.len(), 1);
        assert!(
            outcome.produced_evidence[0]
                .detail
                .contains("security.user_input -> security.sql_execution")
        );
    }
}
