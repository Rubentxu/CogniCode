//! Catalog detectors submitted to the core scheduler: one answer per
//! requested (detector, language) pair, with basis and read scope, and an
//! honest status for what the core cannot run.

use std::num::NonZeroU64;
use std::sync::Arc;

use cognicode_core::application::change_tracking::read_scope::{
    ChangedFacts, READ_SCOPE_SCHEMA, ReadBreadth, ReadVerdict, ScopedRead, resolve_reads,
};
use cognicode_core::application::change_tracking::scheduling::{
    ConsumerClass, Coordinator, CostEstimator, QueuePolicy,
};
use cognicode_core::application::findings::{
    DETECTOR_SUBMISSION_SCHEMA, DetectorResult, DetectorStatus, DetectorSubmission, ObservedInputs,
    RunAuthority, RunContext, SecurityPack, SubmissionReport, SubmissionRequest, submit_detectors,
};
use cognicode_core::domain::budgets::BudgetDeclaration;
use cognicode_core::domain::execution::CorrelationId;
use cognicode_core::domain::findings::semantic_model::{
    CallArg, Observation, ObservationKind, ObservedCall, ObservedFunction, ObservedUnit,
    PACK_VERSION, SecurityFamily, SourceLanguage, StructuralFact, StructuralObservation,
};
use cognicode_core::domain::findings::{AnalysisScope, GroundingRef};
use cognicode_core::domain::kernel_ids::{EntityId, ExecutionId, FactId, SnapshotId};
use cognicode_core::domain::value_objects::WorkspaceId;
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;

const SQL: &str = "security.sql_injection";

fn call(line: u32, callee: &str, args: Vec<CallArg>, def: Option<&str>) -> Observation {
    Observation::new(
        line,
        ObservationKind::Call(ObservedCall {
            callee: callee.to_string(),
            args,
            def: def.map(str::to_string),
        }),
    )
    .grounded(GroundingRef::entity(
        EntityId::new(u64::from(line)),
        FactId::new(500 + u64::from(line)),
    ))
}

fn tainted_sql(name: &str, path: &str) -> ObservedFunction {
    ObservedFunction {
        id: name.to_string(),
        path: path.to_string(),
        language: SourceLanguage::Rust,
        observations: vec![
            call(
                10,
                "actix_web::HttpRequest::query_string",
                vec![CallArg::Literal],
                Some("input"),
            ),
            call(
                11,
                "rusqlite::Connection::execute",
                vec![CallArg::Value {
                    uses: vec!["input".to_string()],
                }],
                None,
            ),
        ],
        jvm: None,
    }
}

fn constant_sql(name: &str, path: &str) -> ObservedFunction {
    ObservedFunction {
        observations: vec![
            call(
                10,
                "actix_web::HttpRequest::query_string",
                vec![CallArg::Literal],
                Some("unused"),
            ),
            call(
                11,
                "rusqlite::Connection::execute",
                vec![CallArg::Literal],
                None,
            ),
        ],
        ..tainted_sql(name, path)
    }
}

fn secret_unit(path: &str) -> ObservedUnit {
    ObservedUnit {
        path: path.to_string(),
        language: SourceLanguage::Rust,
        facts: vec![StructuralObservation {
            line: 4,
            grounding: Some(GroundingRef::entity(EntityId::new(4), FactId::new(704))),
            fact: StructuralFact::StringBinding {
                name: "dbPassword".to_string(),
                value: "s3cr3t-Value-42".to_string(),
            },
        }],
    }
}

fn context() -> RunContext {
    RunContext {
        scope: AnalysisScope::new(
            WorkspaceId::try_new("workspace").expect("valid workspace id"),
            SnapshotId::new(7),
        ),
        execution_id: ExecutionId::new(1),
        correlation: CorrelationId::new("detector-submission-test").expect("valid correlation"),
        authority: RunAuthority::Candidate,
    }
}

fn coordinator(effects: Option<u64>) -> Coordinator {
    let declaration = match effects {
        Some(n) => BudgetDeclaration::effects(NonZeroU64::new(n).expect("non-zero")),
        None => BudgetDeclaration::none(),
    };
    Coordinator::new(
        QueuePolicy {
            capacity: 4,
            ci_reserved: 1,
        },
        declaration,
        Arc::new(CostEstimator::new()),
    )
}

fn submission(id: &str, version: &str, languages: &[&str], required: bool) -> DetectorSubmission {
    DetectorSubmission {
        id: id.to_string(),
        pack_version: version.to_string(),
        languages: languages.iter().map(|l| (*l).to_string()).collect(),
        required,
    }
}

fn submit(
    coordinator: &Coordinator,
    submissions: &[DetectorSubmission],
    inputs: &ObservedInputs,
) -> SubmissionReport {
    let context = context();
    let mut store = InMemoryEvidenceStore::with_scope(context.scope.clone());
    submit_detectors(
        &SecurityPack::standard(),
        coordinator,
        &SubmissionRequest {
            submissions,
            inputs,
            context: &context,
            consumer: ConsumerClass::Ci,
        },
        &mut store,
    )
}

fn result<'a>(report: &'a SubmissionReport, detector: &str, language: &str) -> &'a DetectorResult {
    report
        .results
        .iter()
        .find(|r| r.detector == detector && r.language == language)
        .unwrap_or_else(|| panic!("no answer for {detector} on {language}"))
}

#[test]
fn a_taint_detector_runs_through_the_scheduler_and_answers_with_basis_and_read_scope() {
    let coordinator = coordinator(None);
    let inputs = ObservedInputs {
        functions: vec![
            tainted_sql("handler", "src/handler.rs"),
            constant_sql("report", "src/report.rs"),
        ],
        units: Vec::new(),
    };
    let report = submit(
        &coordinator,
        &[submission(SQL, PACK_VERSION, &["rust"], true)],
        &inputs,
    );

    assert_eq!(report.schema, DETECTOR_SUBMISSION_SCHEMA);
    assert_eq!(report.pack_version, PACK_VERSION);
    let answer = result(&report, SQL, "rust");
    assert_eq!(answer.status, DetectorStatus::Found);
    assert_eq!(answer.findings.len(), 1);
    assert_eq!(answer.findings[0].kind, SQL);
    assert_eq!(answer.blocking, 0, "a candidate detector never blocks");

    assert_eq!(answer.basis.pack_version, PACK_VERSION);
    assert_eq!(answer.basis.snapshot, 7);
    assert_eq!(answer.basis.inputs_analysed, 2);
    assert_eq!(answer.basis.backend.as_deref(), Some("m5_dataflow"));
    assert!(answer.basis.detector_version.is_some());
    assert!(answer.basis.logic_digest.is_some());

    let scope = answer
        .read_scope
        .as_ref()
        .expect("a run publishes its reads");
    assert_eq!(scope.schema, READ_SCOPE_SCHEMA);
    assert_eq!(scope.breadth, ReadBreadth::Pointwise);
    assert!(scope.is_exact());
    assert_eq!(
        scope.reads,
        vec![
            ScopedRead {
                path: "src/handler.rs".to_string(),
                symbol: Some("handler".to_string())
            },
            ScopedRead {
                path: "src/report.rs".to_string(),
                symbol: Some("report".to_string())
            },
        ]
    );

    let physical = coordinator.metrics().physical_computations;
    assert_eq!(physical, 1, "the pair ran as one unit of scheduled work");
}

#[test]
fn a_read_scope_revalidates_on_a_change_it_read_and_not_on_an_unrelated_one() {
    let inputs = ObservedInputs {
        functions: vec![tainted_sql("handler", "src/handler.rs")],
        units: Vec::new(),
    };
    let report = submit(
        &coordinator(None),
        &[submission(SQL, PACK_VERSION, &["rust"], true)],
        &inputs,
    );
    let scope = result(&report, SQL, "rust").read_scope.clone();
    let resolution = resolve_reads(scope.as_ref(), &ChangedFacts::default());
    assert_eq!(resolution.verdict, ReadVerdict::Valid);
}

#[test]
fn a_negative_is_scoped_and_an_undecided_call_is_unknown_never_clean() {
    let mut hidden = tainted_sql("hidden", "src/hidden.rs");
    hidden.observations[1] = call(
        11,
        "com.example.Helper.run",
        vec![CallArg::Value {
            uses: vec!["input".to_string()],
        }],
        None,
    );
    let safe = submit(
        &coordinator(None),
        &[submission(SQL, PACK_VERSION, &["rust"], true)],
        &ObservedInputs {
            functions: vec![constant_sql("report", "src/report.rs")],
            units: Vec::new(),
        },
    );
    assert_eq!(
        result(&safe, SQL, "rust").status,
        DetectorStatus::NotObserved
    );

    let mixed = submit(
        &coordinator(None),
        &[submission(SQL, PACK_VERSION, &["rust"], true)],
        &ObservedInputs {
            functions: vec![constant_sql("report", "src/report.rs"), hidden],
            units: Vec::new(),
        },
    );
    let answer = result(&mixed, SQL, "rust");
    assert_eq!(answer.status, DetectorStatus::Unknown);
    assert_eq!(
        answer.unresolved,
        vec!["com.example.Helper.run".to_string()]
    );
}

#[test]
fn a_structural_detector_reads_units_and_a_language_without_a_model_is_unsupported() {
    let secret = SecurityFamily::HardcodedSecret.finding_kind().to_string();
    let report = submit(
        &coordinator(None),
        &[submission(&secret, PACK_VERSION, &["rust", "python"], true)],
        &ObservedInputs {
            functions: Vec::new(),
            units: vec![secret_unit("src/config.rs")],
        },
    );
    assert_eq!(report.results.len(), 2, "one answer per requested language");
    let found = result(&report, &secret, "rust");
    assert_eq!(found.status, DetectorStatus::Found);
    assert_eq!(
        found
            .read_scope
            .as_ref()
            .expect("a run publishes its reads")
            .reads,
        vec![ScopedRead {
            path: "src/config.rs".to_string(),
            symbol: None
        }]
    );

    let python = result(&report, &secret, "python");
    assert_eq!(python.status, DetectorStatus::Unsupported);
    assert!(python.read_scope.is_none());
    assert!(python.findings.is_empty());
    assert_eq!(python.basis.inputs_analysed, 0);
    assert_eq!(
        resolve_reads(python.read_scope.as_ref(), &ChangedFacts::default()).verdict,
        ReadVerdict::Revalidate,
        "an answer that read nothing is never trusted as still valid"
    );
}

#[test]
fn a_required_detector_the_core_cannot_run_is_answered_with_the_core_reason_never_omitted() {
    let unsafe_code = SecurityFamily::UnsafeCode.finding_kind().to_string();
    let report = submit(
        &coordinator(None),
        &[
            submission(&unsafe_code, PACK_VERSION, &["java", "go"], true),
            submission(SQL, PACK_VERSION, &["terraform"], true),
        ],
        &ObservedInputs::default(),
    );
    assert_eq!(report.results.len(), 3);
    for answer in &report.results {
        assert_eq!(answer.status, DetectorStatus::Unsupported, "{answer:?}");
        assert!(!answer.reason.is_empty());
    }
    // The reason is the capability matrix's own, not a paraphrase.
    let matrix = SecurityPack::standard().capability_matrix();
    let cell = matrix
        .entries
        .iter()
        .find(|e| e.language == SourceLanguage::Java && e.family.finding_kind() == unsafe_code)
        .expect("the matrix has the cell");
    let cognicode_core::domain::findings::semantic_model::Support::Unsupported { reason } =
        &cell.support
    else {
        panic!("the matrix must mark unsafe code unsupported on Java");
    };
    assert!(
        result(&report, &unsafe_code, "java")
            .reason
            .contains(reason.as_str())
    );
}

#[test]
fn a_reference_the_pack_cannot_honour_is_unsupported_with_the_reason_and_never_dropped() {
    let report = submit(
        &coordinator(None),
        &[
            submission("security.no_such_detector", PACK_VERSION, &["rust"], true),
            submission(SQL, "0.0.1", &["rust"], true),
            submission(SQL, PACK_VERSION, &["cobol"], true),
            submission(SQL, PACK_VERSION, &[], true),
        ],
        &ObservedInputs {
            functions: vec![tainted_sql("handler", "src/handler.rs")],
            units: Vec::new(),
        },
    );
    assert_eq!(report.results.len(), 4);
    assert!(
        report
            .results
            .iter()
            .all(|r| r.status == DetectorStatus::Unsupported)
    );
    assert!(report.results[0].reason.contains("no detector"));
    assert!(report.results[1].reason.contains("0.0.1"));
    assert!(report.results[2].reason.contains("cobol"));
    assert!(report.results[3].reason.contains("no language"));
}

#[test]
fn nothing_observed_is_unknown_and_runs_nothing() {
    let coordinator = coordinator(None);
    let report = submit(
        &coordinator,
        &[submission(SQL, PACK_VERSION, &["rust"], true)],
        &ObservedInputs::default(),
    );
    let answer = result(&report, SQL, "rust");
    assert_eq!(answer.status, DetectorStatus::Unknown);
    assert_eq!(answer.basis.inputs_analysed, 0);
    assert!(answer.read_scope.is_none());
    assert_eq!(coordinator.metrics().physical_computations, 0);
}

#[test]
fn work_the_scheduler_refuses_is_inconclusive_when_required_and_deferred_when_optional() {
    let inputs = ObservedInputs {
        functions: vec![tainted_sql("handler", "src/handler.rs")],
        units: Vec::new(),
    };
    // A budget of one effect cannot cover a cost of five.
    let strict = coordinator(Some(1));
    let heavy = ObservedInputs {
        functions: (0..5)
            .map(|n| tainted_sql(&format!("f{n}"), &format!("src/f{n}.rs")))
            .collect(),
        units: Vec::new(),
    };
    let required = submit(
        &strict,
        &[submission(SQL, PACK_VERSION, &["rust"], true)],
        &heavy,
    );
    let answer = result(&required, SQL, "rust");
    assert_eq!(answer.status, DetectorStatus::Inconclusive);
    assert!(answer.findings.is_empty());
    assert!(answer.read_scope.is_none());

    let optional = submit(
        &coordinator(Some(1)),
        &[submission(SQL, PACK_VERSION, &["rust"], false)],
        &heavy,
    );
    assert_eq!(
        result(&optional, SQL, "rust").status,
        DetectorStatus::Deferred
    );

    // The same submission within budget runs.
    let roomy = submit(
        &coordinator(Some(100)),
        &[submission(SQL, PACK_VERSION, &["rust"], true)],
        &inputs,
    );
    assert_eq!(result(&roomy, SQL, "rust").status, DetectorStatus::Found);
}

#[test]
fn the_report_survives_its_wire_form_and_readers_ignore_unknown_fields() {
    let report = submit(
        &coordinator(None),
        &[submission(SQL, PACK_VERSION, &["rust", "python"], true)],
        &ObservedInputs {
            functions: vec![tainted_sql("handler", "src/handler.rs")],
            units: Vec::new(),
        },
    );
    let wire = serde_json::to_string(&report).expect("serialises");
    let back: SubmissionReport = serde_json::from_str(&wire).expect("reads back");
    assert_eq!(back, report);

    let mut value: serde_json::Value = serde_json::from_str(&wire).expect("json");
    value["a_later_field"] = serde_json::json!(true);
    value["results"][0]["basis"]["a_later_basis_field"] = serde_json::json!(1);
    let tolerant: SubmissionReport = serde_json::from_value(value).expect("tolerant reader");
    assert_eq!(tolerant, report);
}
