//! Semantic SAST models, CWE-89 (SQL injection) end to end.
//!
//! One shared detector, per-language API adapters, and the unchanged
//! execution seam:
//!
//! ```text
//! observed calls → adapter (catalog) → DataflowInput
//!   → DetectorIr (shared) → admission → planner → M5DataflowBackend
//!   → Finding (B) → assessment (Vulnerable | NoFlowObserved | Unknown)
//! ```
//!
//! The corpus below is labeled; the measured precision/recall are pinned so a
//! regression in either direction is visible. Cases the model cannot decide are
//! reported as `Unknown`, never as clean.

use cognicode_core::application::findings::M5DataflowBackend;
use cognicode_core::application::program_analysis::ProgramAnalysisService;
use cognicode_core::domain::execution::{ActorRef, CorrelationId};
use cognicode_core::domain::findings::semantic_model::{
    AnalysisReach, CallArg, Classification, Observation, ObservationKind, ObservedCall,
    ObservedFunction, PrecisionRecall, SastVerdict, SemanticModelCatalog, SourceLanguage, Weakness,
    assess, cwe89_sql_injection,
};
use cognicode_core::domain::findings::{
    AdmissionSource, AnalysisCapability, AnalysisInput, AnalysisScope, AstBackend, BackendRegistry,
    DetectorAdmission, DetectorExecutor, DetectorIr, EvidenceClass, ExecutionError,
    ExecutionRequest, FindingKind, GraphBackend,
};
use cognicode_core::domain::kernel_ids::{ExecutionId, SnapshotId};
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;

// ---------------------------------------------------------------------------
// Fixture vocabulary
// ---------------------------------------------------------------------------

fn call(callee: &str, args: Vec<CallArg>, def: Option<&str>) -> Observation {
    Observation::new(
        10,
        ObservationKind::Call(ObservedCall {
            callee: callee.to_string(),
            args,
            def: def.map(str::to_string),
        }),
    )
}

fn assign(def: &str, uses: &[&str]) -> Observation {
    Observation::new(
        10,
        ObservationKind::Assign {
            def: def.to_string(),
            uses: uses.iter().map(|u| u.to_string()).collect(),
        },
    )
}

fn lit() -> CallArg {
    CallArg::Literal
}

fn val(uses: &[&str]) -> CallArg {
    CallArg::Value {
        uses: uses.iter().map(|u| u.to_string()).collect(),
    }
}

struct Case {
    label: &'static str,
    language: SourceLanguage,
    /// Ground truth: does the fixture really allow SQL injection?
    vulnerable: bool,
    observations: Vec<Observation>,
}

fn function(case: &Case) -> ObservedFunction {
    ObservedFunction {
        id: case.label.to_string(),
        path: format!("fixtures/{}", case.label),
        language: case.language,
        observations: case.observations.clone(),
        jvm: None,
    }
}

const JAVA_PARAM: &str = "javax.servlet.http.HttpServletRequest.getParameter";
const JAVA_EXEC: &str = "java.sql.Statement.executeQuery";
const JAVA_PREPARE: &str = "java.sql.Connection.prepareStatement";
const JAVA_BIND: &str = "java.sql.PreparedStatement.setString";
const JAVA_RUN: &str = "java.sql.PreparedStatement.executeQuery";
const JAVA_PARSE: &str = "java.lang.Integer.parseInt";
const GO_FORM: &str = "net/http.Request.FormValue";
const GO_QUERY: &str = "database/sql.DB.Query";
const GO_ATOI: &str = "strconv.Atoi";
const RS_QS: &str = "actix_web::HttpRequest::query_string";
const RS_QUERY: &str = "sqlx::query";
const RS_BIND: &str = "sqlx::query::Query::bind";
const RS_PARSE: &str = "i64::from_str";

fn corpus() -> Vec<Case> {
    use SourceLanguage::{Go, Java, Rust};
    vec![
        // --- vulnerable: concatenation with external data -----------------
        Case {
            label: "java-concat",
            language: Java,
            vulnerable: true,
            observations: vec![
                call(JAVA_PARAM, vec![lit()], Some("id")),
                assign("sql", &["id"]),
                call(JAVA_EXEC, vec![val(&["sql"])], Some("rows")),
            ],
        },
        Case {
            label: "java-prepare-with-concatenated-text",
            language: Java,
            vulnerable: true,
            observations: vec![
                call(JAVA_PARAM, vec![lit()], Some("id")),
                call(JAVA_PREPARE, vec![val(&["id"])], Some("ps")),
                call(JAVA_RUN, vec![], Some("rows")),
            ],
        },
        Case {
            label: "go-concat",
            language: Go,
            vulnerable: true,
            observations: vec![
                call(GO_FORM, vec![lit()], Some("id")),
                call(GO_QUERY, vec![val(&["id"])], Some("rows")),
            ],
        },
        Case {
            label: "rust-concat",
            language: Rust,
            vulnerable: true,
            observations: vec![
                call(RS_QS, vec![], Some("qs")),
                assign("sql", &["qs"]),
                call(RS_QUERY, vec![val(&["sql"])], Some("q")),
            ],
        },
        // --- safe: parameterized queries ----------------------------------
        Case {
            label: "java-parameterized",
            language: Java,
            vulnerable: false,
            observations: vec![
                call(JAVA_PARAM, vec![lit()], Some("id")),
                call(JAVA_PREPARE, vec![lit()], Some("ps")),
                call(JAVA_BIND, vec![lit(), val(&["id"])], None),
                call(JAVA_RUN, vec![], Some("rows")),
            ],
        },
        Case {
            label: "go-parameterized",
            language: Go,
            vulnerable: false,
            observations: vec![
                call(GO_FORM, vec![lit()], Some("id")),
                call(GO_QUERY, vec![lit(), val(&["id"])], Some("rows")),
            ],
        },
        Case {
            label: "rust-bound",
            language: Rust,
            vulnerable: false,
            observations: vec![
                call(RS_QS, vec![], Some("qs")),
                call(RS_QUERY, vec![lit()], Some("q")),
                call(RS_BIND, vec![val(&["qs"])], Some("q2")),
            ],
        },
        // --- safe: sanitized (numeric coercion) ---------------------------
        Case {
            label: "java-sanitized",
            language: Java,
            vulnerable: false,
            observations: vec![
                call(JAVA_PARAM, vec![lit()], Some("id")),
                call(JAVA_PARSE, vec![val(&["id"])], Some("n")),
                assign("sql", &["n"]),
                call(JAVA_EXEC, vec![val(&["sql"])], Some("rows")),
            ],
        },
        Case {
            label: "go-sanitized",
            language: Go,
            vulnerable: false,
            observations: vec![
                call(GO_FORM, vec![lit()], Some("id")),
                call(GO_ATOI, vec![val(&["id"])], Some("n")),
                call(GO_QUERY, vec![val(&["n"])], Some("rows")),
            ],
        },
        Case {
            label: "rust-sanitized",
            language: Rust,
            vulnerable: false,
            observations: vec![
                call(RS_QS, vec![], Some("qs")),
                call(RS_PARSE, vec![val(&["qs"])], Some("n")),
                call(RS_QUERY, vec![val(&["n"])], Some("q")),
            ],
        },
        // --- safe: no external data ---------------------------------------
        Case {
            label: "java-constant-query",
            language: Java,
            vulnerable: false,
            observations: vec![call(JAVA_EXEC, vec![lit()], Some("rows"))],
        },
        // --- known limits, labeled honestly -------------------------------
        // The query runs inside an unmodeled project helper: the intra-
        // procedural model cannot see it, and must say so instead of "clean".
        Case {
            label: "java-sink-hidden-in-helper",
            language: Java,
            vulnerable: true,
            observations: vec![
                call(JAVA_PARAM, vec![lit()], Some("id")),
                call("com.example.Dao.run", vec![val(&["id"])], Some("rows")),
            ],
        },
        // A project-specific sanitizer the catalog does not know: the taint
        // engine keeps the value tainted, so this is a reported false positive.
        Case {
            label: "java-custom-sanitizer",
            language: Java,
            vulnerable: false,
            observations: vec![
                call(JAVA_PARAM, vec![lit()], Some("id")),
                call(
                    "com.example.Escaper.escape",
                    vec![val(&["id"])],
                    Some("safe"),
                ),
                call(JAVA_EXEC, vec![val(&["safe"])], Some("rows")),
            ],
        },
    ]
}

// ---------------------------------------------------------------------------
// Harness: the unchanged executor/M5 seam
// ---------------------------------------------------------------------------

fn scope() -> AnalysisScope {
    AnalysisScope::new(
        cognicode_core::domain::value_objects::WorkspaceId::try_new("workspace").unwrap(),
        SnapshotId::new(1),
    )
}

fn registry() -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    registry.register(Box::new(AstBackend));
    registry.register(Box::new(GraphBackend));
    registry.register(Box::new(M5DataflowBackend::new(
        ProgramAnalysisService::new(),
    )));
    registry
}

fn request() -> ExecutionRequest {
    ExecutionRequest::new(
        ExecutionId::new(7),
        scope(),
        ActorRef::detector("test.detector"),
        CorrelationId::new("test-correlation").unwrap(),
    )
}

fn admit(ir: DetectorIr) -> cognicode_core::domain::findings::ExecutionPermit {
    DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::HumanCurated).unwrap()
}

/// Classify, run the shared rule through the real executor, and assess.
fn analyse(case: &Case) -> (SastVerdict, usize, Vec<EvidenceClass>) {
    let catalog = SemanticModelCatalog::builtin();
    let classification = catalog.classify(Weakness::SqlInjection, &function(case));
    let Classification::Modeled(modeled) = &classification else {
        return (assess(&classification, 0).verdict, 0, vec![]);
    };

    let permit = admit(cwe89_sql_injection(AnalysisReach::Intraprocedural));
    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::with_scope(scope());
    let record = executor
        .execute(
            &permit,
            &AnalysisInput {
                scope: Some(scope()),
                ast: None,
                graph: None,
                dataflow: Some(modeled.dataflow_input()),
            },
            &mut store,
            request(),
        )
        .expect("execution must succeed");

    let classes = record.findings.iter().map(|f| f.evidence_class).collect();
    (
        assess(&classification, record.findings.len()).verdict,
        record.findings.len(),
        classes,
    )
}

fn case(label: &str) -> Case {
    corpus()
        .into_iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("unknown fixture {label}"))
}

// ---------------------------------------------------------------------------
// AC1 — parameterized vs concatenated
// ---------------------------------------------------------------------------

#[test]
fn concatenation_with_external_data_is_reported_in_every_language() {
    for label in [
        "java-concat",
        "java-prepare-with-concatenated-text",
        "go-concat",
        "rust-concat",
    ] {
        let (verdict, findings, classes) = analyse(&case(label));
        assert_eq!(findings, 1, "{label}: exactly one finding");
        assert_eq!(verdict, SastVerdict::Vulnerable { findings: 1 }, "{label}");
        assert_eq!(classes, vec![EvidenceClass::B], "{label}: taint path is B");
    }
}

#[test]
fn parameterized_queries_with_external_data_are_not_reported() {
    for label in ["java-parameterized", "go-parameterized", "rust-bound"] {
        let (verdict, findings, _) = analyse(&case(label));
        assert_eq!(
            findings, 0,
            "{label}: bound values never reach the SQL text"
        );
        assert_eq!(
            verdict,
            SastVerdict::NoFlowObserved {
                reach: AnalysisReach::Intraprocedural
            },
            "{label}"
        );
    }
}

#[test]
fn the_same_external_value_flips_verdict_only_by_where_it_lands() {
    // Same source, same sink API family: the value in the SQL text is
    // vulnerable, the value in a bound position is not.
    let (concat, _, _) = analyse(&case("go-concat"));
    let (param, _, _) = analyse(&case("go-parameterized"));
    assert!(matches!(concat, SastVerdict::Vulnerable { .. }));
    assert!(matches!(param, SastVerdict::NoFlowObserved { .. }));
}

// ---------------------------------------------------------------------------
// AC2 — exact capabilities; no higher-level evidence without them
// ---------------------------------------------------------------------------

#[test]
fn rule_declares_exact_capabilities_per_reach() {
    let intra = cwe89_sql_injection(AnalysisReach::Intraprocedural);
    assert_eq!(
        intra.requires,
        [AnalysisCapability::Dataflow].into_iter().collect()
    );

    let inter = cwe89_sql_injection(AnalysisReach::Interprocedural);
    assert_eq!(
        inter.requires,
        [
            AnalysisCapability::Dataflow,
            AnalysisCapability::SemanticResolution,
            AnalysisCapability::GraphQuery,
        ]
        .into_iter()
        .collect()
    );
}

#[test]
fn interprocedural_rule_is_declared_but_cannot_produce_evidence_without_resolution() {
    let permit = admit(cwe89_sql_injection(AnalysisReach::Interprocedural));
    let registry = registry();
    let executor = DetectorExecutor::new(&registry);
    let mut store = InMemoryEvidenceStore::with_scope(scope());

    let modeled = match SemanticModelCatalog::builtin()
        .classify(Weakness::SqlInjection, &function(&case("java-concat")))
    {
        Classification::Modeled(m) => m,
        other => panic!("java must be modeled, got {other:?}"),
    };
    let err = executor
        .execute(
            &permit,
            &AnalysisInput {
                scope: Some(scope()),
                ast: None,
                graph: None,
                dataflow: Some(modeled.dataflow_input()),
            },
            &mut store,
            request(),
        )
        .expect_err("no backend provides resolution: no evidence may be admitted");
    match err {
        ExecutionError::Plan(plan) => {
            assert!(
                plan.missing
                    .contains(&AnalysisCapability::SemanticResolution)
            );
        }
        other => panic!("expected a planning error, got {other:?}"),
    }
}

#[test]
fn both_reaches_produce_the_same_finding_kind() {
    let kind = FindingKind::new("security.sql_injection").unwrap();
    for reach in [
        AnalysisReach::Intraprocedural,
        AnalysisReach::Interprocedural,
    ] {
        let ir = cwe89_sql_injection(reach);
        assert!(
            ir.steps.iter().any(|s| matches!(
                s,
                cognicode_core::domain::findings::DetectorStep::Produce { kind: k } if *k == kind
            )),
            "{reach:?} must produce the shared kind"
        );
    }
}

// ---------------------------------------------------------------------------
// Honesty — Unknown / Unsupported are never "clean"
// ---------------------------------------------------------------------------

#[test]
fn sink_hidden_in_an_unmodeled_helper_is_unknown_not_clean() {
    let (verdict, findings, _) = analyse(&case("java-sink-hidden-in-helper"));
    assert_eq!(findings, 0);
    match verdict {
        SastVerdict::Unknown { unresolved } => {
            assert_eq!(unresolved, vec!["com.example.Dao.run".to_string()]);
        }
        other => panic!("expected Unknown, got {other:?}"),
    }
}

#[test]
fn unsupported_language_is_reported_as_unsupported() {
    let python = ObservedFunction {
        id: "p".to_string(),
        path: "fixtures/p".to_string(),
        language: SourceLanguage::Python,
        observations: vec![call(JAVA_PARAM, vec![lit()], Some("id"))],
        jvm: None,
    };
    let classification = SemanticModelCatalog::builtin().classify(Weakness::SqlInjection, &python);
    assert_eq!(
        classification,
        Classification::Unsupported {
            language: SourceLanguage::Python
        }
    );
    assert_eq!(
        assess(&classification, 0).verdict,
        SastVerdict::Unsupported {
            language: SourceLanguage::Python
        }
    );
}

// ---------------------------------------------------------------------------
// AC3 — measurable precision / recall
// ---------------------------------------------------------------------------

#[test]
fn corpus_precision_and_recall_are_measured_and_pinned() {
    let mut tally = PrecisionRecall::default();
    for case in corpus() {
        let (verdict, _, _) = analyse(&case);
        tally.record(case.vulnerable, &verdict);
    }

    // 4 detected, 1 hidden in a helper (Unknown), 1 false positive from the
    // unmodeled custom sanitizer, 7 true negatives.
    assert_eq!(tally.true_positives(), 4);
    assert_eq!(tally.false_positives(), 1);
    assert_eq!(tally.true_negatives(), 7);
    assert_eq!(tally.false_negatives(), 0);
    assert_eq!(tally.unknown_on_vulnerable(), 1);
    assert_eq!(tally.unknown_on_safe(), 0);
    assert_eq!(tally.precision(), Some(0.8));
    assert_eq!(tally.recall(), Some(0.8));
}

#[test]
fn custom_sanitizer_is_a_visible_false_positive_not_a_silent_pass() {
    let (verdict, _, _) = analyse(&case("java-custom-sanitizer"));
    assert_eq!(verdict, SastVerdict::Vulnerable { findings: 1 });
}

// ---------------------------------------------------------------------------
// AC4 — one flow logic, per-language adapters
// ---------------------------------------------------------------------------

#[test]
fn every_language_reuses_the_same_detector_and_vocabulary() {
    let catalog = SemanticModelCatalog::builtin();
    let rule = cwe89_sql_injection(AnalysisReach::Intraprocedural);
    let digest = rule.logic_digest();

    for language in [
        SourceLanguage::Rust,
        SourceLanguage::Java,
        SourceLanguage::Go,
    ] {
        let adapter = catalog
            .adapter(language)
            .unwrap_or_else(|| panic!("{language:?} adapter missing"));
        assert!(adapter.models().any(|m| m.is_source()));
        assert!(adapter.models().any(|m| m.sinks(Weakness::SqlInjection)));
        assert!(
            adapter
                .models()
                .any(|m| m.sanitizes(Weakness::SqlInjection))
        );
        // The rule is built once and carries no language: its logic identity
        // does not depend on the adapter.
        assert_eq!(
            cwe89_sql_injection(AnalysisReach::Intraprocedural).logic_digest(),
            digest
        );
    }
    assert!(
        catalog.adapter(SourceLanguage::Python).is_none(),
        "no adapter means Unsupported, not an empty model"
    );
}
