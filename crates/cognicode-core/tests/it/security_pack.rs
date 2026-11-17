//! The first security pack, end to end: Rust, Java, Kotlin and Go over
//! SQL injection, command injection, path traversal, hardcoded secrets and
//! `unsafe`.
//!
//! Every taint family has, per language, a **vulnerable**, a **safe** and an
//! **undecidable** fixture. Structural families add unsupported-construct
//! fixtures. The tests pin what blocks and what does not:
//!
//! - a finding blocks only with a promoted detector, grounded evidence of the
//!   family's declared class and a risk at the family's declared level;
//! - `unsafe` is a risk and never blocks;
//! - a negative is scoped (`NotObserved`) or undecided (`Unknown`), never clean.

use cognicode_core::application::findings::{PackRun, RunAuthority, RunContext, SecurityPack};
use cognicode_core::domain::execution::CorrelationId;
use cognicode_core::domain::findings::semantic_model::{
    BlockingLevel, CallArg, CapabilityMatrix, FindingNature, JvmContext, Observation,
    ObservationKind, ObservedCall, ObservedFunction, ObservedUnit, PackVerdict, SecurityFamily,
    SourceLanguage, StructuralFact, StructuralFamily, StructuralObservation, Support,
    UnsafeConstruct, Weakness,
};
use cognicode_core::domain::findings::{
    AnalysisScope, EvidenceClass, FindingGate, FindingSeverity, FindingVerifier, GroundingRef,
    RiskLevel,
};
use cognicode_core::domain::kernel_ids::{EntityId, ExecutionId, FactId, SnapshotId};
use cognicode_core::infrastructure::findings::in_memory_evidence::InMemoryEvidenceStore;

use SourceLanguage::{Go, Java, Kotlin, Rust};

// ---------------------------------------------------------------------------
// Vocabulary
// ---------------------------------------------------------------------------

/// The callees a language uses for one taint family.
struct Apis {
    source: &'static str,
    sink: &'static str,
    sanitizer: &'static str,
}

fn apis(language: SourceLanguage, weakness: Weakness) -> Apis {
    let (source, sink, sanitizer) = match (language, weakness) {
        (Rust, Weakness::SqlInjection) => (
            "actix_web::HttpRequest::query_string",
            "rusqlite::Connection::execute",
            "i64::from_str",
        ),
        (Rust, Weakness::CommandInjection) => (
            "actix_web::HttpRequest::query_string",
            "std::process::Command::new",
            "i64::from_str",
        ),
        (Rust, Weakness::PathTraversal) => (
            "actix_web::HttpRequest::query_string",
            "std::fs::read_to_string",
            "std::path::Path::file_name",
        ),
        (Java | Kotlin, Weakness::SqlInjection) => (
            "javax.servlet.http.HttpServletRequest.getParameter",
            "java.sql.Statement.executeQuery",
            "java.lang.Integer.parseInt",
        ),
        (Java | Kotlin, Weakness::CommandInjection) => (
            "javax.servlet.http.HttpServletRequest.getParameter",
            "java.lang.Runtime.exec",
            "java.lang.Integer.parseInt",
        ),
        (Java | Kotlin, Weakness::PathTraversal) => (
            "javax.servlet.http.HttpServletRequest.getParameter",
            "java.nio.file.Files.readString",
            "java.nio.file.Path.getFileName",
        ),
        (Go, Weakness::SqlInjection) => (
            "net/http.Request.FormValue",
            "database/sql.DB.Query",
            "strconv.Atoi",
        ),
        (Go, Weakness::CommandInjection) => (
            "net/http.Request.FormValue",
            "os/exec.Command",
            "strconv.Atoi",
        ),
        (Go, Weakness::PathTraversal) => (
            "net/http.Request.FormValue",
            "os.ReadFile",
            "path/filepath.Base",
        ),
        (language, weakness) => panic!("no fixture vocabulary for {language:?} {weakness:?}"),
    };
    Apis {
        source,
        sink,
        sanitizer,
    }
}

fn call(line: u32, callee: &str, args: Vec<CallArg>, def: Option<&str>) -> Observation {
    // Every statement is grounded in a canonical fact, as a real projection
    // would record it.
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

fn value(uses: &[&str]) -> CallArg {
    CallArg::Value {
        uses: uses.iter().map(|u| u.to_string()).collect(),
    }
}

fn function(language: SourceLanguage, observations: Vec<Observation>) -> ObservedFunction {
    ObservedFunction {
        id: "handler".to_string(),
        path: "src/handler".to_string(),
        language,
        observations,
        jvm: None,
    }
}

fn vulnerable(language: SourceLanguage, weakness: Weakness) -> ObservedFunction {
    let a = apis(language, weakness);
    function(
        language,
        vec![
            call(10, a.source, vec![CallArg::Literal], Some("input")),
            call(11, a.sink, vec![value(&["input"])], None),
        ],
    )
}

fn sanitized(language: SourceLanguage, weakness: Weakness) -> ObservedFunction {
    let a = apis(language, weakness);
    function(
        language,
        vec![
            call(10, a.source, vec![CallArg::Literal], Some("input")),
            call(11, a.sanitizer, vec![value(&["input"])], Some("clean")),
            call(12, a.sink, vec![value(&["clean"])], None),
        ],
    )
}

fn constant_only(language: SourceLanguage, weakness: Weakness) -> ObservedFunction {
    let a = apis(language, weakness);
    function(
        language,
        vec![
            call(10, a.source, vec![CallArg::Literal], Some("unused")),
            call(11, a.sink, vec![CallArg::Literal], None),
        ],
    )
}

fn hidden_in_helper(language: SourceLanguage, weakness: Weakness) -> ObservedFunction {
    let a = apis(language, weakness);
    function(
        language,
        vec![
            call(10, a.source, vec![CallArg::Literal], Some("input")),
            call(11, "com.example.Helper.run", vec![value(&["input"])], None),
        ],
    )
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

fn scope() -> AnalysisScope {
    AnalysisScope::new(
        cognicode_core::domain::value_objects::WorkspaceId::try_new("workspace").unwrap(),
        SnapshotId::new(1),
    )
}

fn context(authority: RunAuthority) -> RunContext {
    RunContext {
        scope: scope(),
        execution_id: ExecutionId::new(1),
        correlation: CorrelationId::new("security-pack-test").unwrap(),
        authority,
    }
}

fn promoted() -> RunAuthority {
    RunAuthority::Promoted {
        approver: "security-team".to_string(),
    }
}

fn store() -> InMemoryEvidenceStore {
    InMemoryEvidenceStore::with_scope(scope())
}

fn taint(
    weakness: Weakness,
    function: &ObservedFunction,
    authority: RunAuthority,
) -> (PackRun, InMemoryEvidenceStore) {
    let mut store = store();
    let run = SecurityPack::standard()
        .analyse_taint(weakness, function, &context(authority), &mut store)
        .expect("the run must be carried out");
    (run, store)
}

fn structural(
    family: StructuralFamily,
    unit: &ObservedUnit,
    authority: RunAuthority,
) -> (PackRun, InMemoryEvidenceStore) {
    let mut store = store();
    let run = SecurityPack::standard()
        .analyse_structural(family, unit, &context(authority), &mut store)
        .expect("the run must be carried out");
    (run, store)
}

const MODELED: [SourceLanguage; 4] = [Rust, Java, Kotlin, Go];

// ---------------------------------------------------------------------------
// AC1 — safe / vulnerable / unsupported fixtures per language and family
// ---------------------------------------------------------------------------

#[test]
fn vulnerable_fixtures_are_found_with_a_dataflow_path_in_every_language_and_family() {
    for language in MODELED {
        for weakness in Weakness::ALL {
            let (run, _) = taint(
                weakness,
                &vulnerable(language, weakness),
                RunAuthority::Candidate,
            );
            let label = format!("{language:?} {weakness:?}");
            assert_eq!(
                run.verdict,
                PackVerdict::Found {
                    nature: FindingNature::Vulnerability,
                    findings: 1
                },
                "{label}"
            );
            let finding = &run.findings[0];
            assert_eq!(finding.kind.as_str(), weakness.finding_kind(), "{label}");
            assert_eq!(finding.evidence_class, EvidenceClass::B, "{label}");
            assert_eq!(run.backend.as_deref(), Some("m5_dataflow"), "{label}");
        }
    }
}

#[test]
fn safe_fixtures_are_not_reported_and_the_negative_is_scoped() {
    for language in MODELED {
        for weakness in Weakness::ALL {
            for (name, fixture) in [
                ("sanitized", sanitized(language, weakness)),
                ("constant", constant_only(language, weakness)),
            ] {
                let (run, _) = taint(weakness, &fixture, promoted());
                let label = format!("{language:?} {weakness:?} {name}");
                assert!(run.findings.is_empty(), "{label}");
                assert_eq!(run.blocking, 0, "{label}");
                assert_eq!(
                    run.verdict,
                    PackVerdict::NotObserved {
                        within: "intraprocedural flow".to_string()
                    },
                    "{label}: a negative names its reach, it never says clean"
                );
            }
        }
    }
}

#[test]
fn undecidable_fixtures_are_unknown_never_safe() {
    for language in MODELED {
        for weakness in Weakness::ALL {
            let (run, _) = taint(weakness, &hidden_in_helper(language, weakness), promoted());
            assert_eq!(
                run.verdict,
                PackVerdict::Unknown {
                    unresolved: vec!["com.example.Helper.run".to_string()]
                },
                "{language:?} {weakness:?}"
            );
            assert_eq!(run.blocking, 0);
        }
    }
}

#[test]
fn a_language_without_a_model_is_unsupported_for_every_taint_family() {
    for weakness in Weakness::ALL {
        let (run, _) = taint(
            weakness,
            &function(SourceLanguage::Python, vec![]),
            promoted(),
        );
        assert!(
            matches!(run.verdict, PackVerdict::Unsupported { .. }),
            "{weakness:?}"
        );
        assert_eq!(run.backend, None, "nothing ran");
    }
}

fn binding(line: u32, name: &str, text: &str) -> StructuralObservation {
    StructuralObservation {
        line,
        grounding: Some(GroundingRef::entity(
            EntityId::new(u64::from(line)),
            FactId::new(700 + u64::from(line)),
        )),
        fact: StructuralFact::StringBinding {
            name: name.to_string(),
            value: text.to_string(),
        },
    }
}

fn unsafe_fact(line: u32, safety_comment: bool) -> StructuralObservation {
    StructuralObservation {
        line,
        grounding: Some(GroundingRef::entity(
            EntityId::new(u64::from(line)),
            FactId::new(800 + u64::from(line)),
        )),
        fact: StructuralFact::Unsafe {
            construct: UnsafeConstruct::Block,
            safety_comment,
        },
    }
}

fn unit(language: SourceLanguage, facts: Vec<StructuralObservation>) -> ObservedUnit {
    ObservedUnit {
        path: "src/config".to_string(),
        language,
        facts,
    }
}

#[test]
fn hardcoded_secrets_are_found_and_placeholders_are_not_in_every_language() {
    for language in MODELED {
        let (found, _) = structural(
            StructuralFamily::HardcodedSecret,
            &unit(language, vec![binding(4, "dbPassword", "s3cr3t-Value-42")]),
            RunAuthority::Candidate,
        );
        assert_eq!(
            found.verdict,
            PackVerdict::Found {
                nature: FindingNature::Vulnerability,
                findings: 1
            },
            "{language:?}"
        );
        assert_eq!(found.findings[0].evidence_class, EvidenceClass::C);

        let (safe, _) = structural(
            StructuralFamily::HardcodedSecret,
            &unit(
                language,
                vec![
                    binding(4, "dbPassword", "${DB_PASSWORD}"),
                    binding(5, "password_file", "/run/secrets/db-password"),
                ],
            ),
            promoted(),
        );
        assert!(safe.findings.is_empty(), "{language:?}");
        assert!(matches!(safe.verdict, PackVerdict::NotObserved { .. }));
    }
}

#[test]
fn a_secret_value_never_leaks_into_the_finding() {
    let (run, _) = structural(
        StructuralFamily::HardcodedSecret,
        &unit(
            Rust,
            vec![binding(4, "api_key", "AKIA-very-secret-literal")],
        ),
        RunAuthority::Candidate,
    );
    let rendered = format!("{:?}", run.findings);
    assert!(!rendered.contains("AKIA-very-secret-literal"));
}

#[test]
fn unsafe_is_unsupported_outside_rust_and_modeled_in_rust() {
    for language in [Java, Kotlin, Go] {
        let (run, _) = structural(
            StructuralFamily::UnsafeCode,
            &unit(language, vec![unsafe_fact(3, false)]),
            promoted(),
        );
        assert!(
            matches!(run.verdict, PackVerdict::Unsupported { .. }),
            "{language:?}"
        );
        assert!(run.findings.is_empty());
    }
    let (run, _) = structural(
        StructuralFamily::UnsafeCode,
        &unit(Rust, vec![]),
        RunAuthority::Candidate,
    );
    assert!(matches!(run.verdict, PackVerdict::NotObserved { .. }));
}

// ---------------------------------------------------------------------------
// AC2 — block only when the evidence satisfies the declared level
// ---------------------------------------------------------------------------

#[test]
fn a_promoted_detector_with_grounded_dataflow_evidence_blocks_in_every_language_and_family() {
    for language in MODELED {
        for weakness in Weakness::ALL {
            let (run, _) = taint(weakness, &vulnerable(language, weakness), promoted());
            assert_eq!(run.blocking, 1, "{language:?} {weakness:?}");
        }
    }
}

#[test]
fn a_candidate_detector_finds_but_never_blocks() {
    for weakness in Weakness::ALL {
        let (run, _) = taint(weakness, &vulnerable(Go, weakness), RunAuthority::Candidate);
        assert_eq!(run.findings.len(), 1);
        assert_eq!(run.blocking, 0, "{weakness:?}");
    }
}

#[test]
fn ungrounded_evidence_finds_but_does_not_block() {
    for weakness in Weakness::ALL {
        let a = apis(Java, weakness);
        let ungrounded = function(
            Java,
            vec![
                Observation::new(
                    10,
                    ObservationKind::Call(ObservedCall {
                        callee: a.source.to_string(),
                        args: vec![CallArg::Literal],
                        def: Some("input".to_string()),
                    }),
                ),
                Observation::new(
                    11,
                    ObservationKind::Call(ObservedCall {
                        callee: a.sink.to_string(),
                        args: vec![value(&["input"])],
                        def: None,
                    }),
                ),
            ],
        );
        let (run, _) = taint(weakness, &ungrounded, promoted());
        assert_eq!(run.findings.len(), 1, "{weakness:?}: still explained");
        assert_eq!(run.blocking, 0, "{weakness:?}: no canonical fact, no block");
    }
}

#[test]
fn a_taint_finding_does_not_clear_a_stricter_evidence_level_than_it_has() {
    let (run, store) = taint(
        Weakness::SqlInjection,
        &vulnerable(Rust, Weakness::SqlInjection),
        promoted(),
    );
    let verifier = FindingVerifier::new(&store);
    let finding = &run.findings[0];
    assert!(verifier.can_block(
        finding,
        &FindingGate::new(EvidenceClass::B, RiskLevel::High)
    ));
    assert!(
        !verifier.can_block(
            finding,
            &FindingGate::new(EvidenceClass::A, RiskLevel::High)
        ),
        "class B evidence must not satisfy a class A requirement"
    );
}

#[test]
fn a_secret_is_class_c_evidence_and_blocks_only_at_its_declared_class() {
    let (run, store) = structural(
        StructuralFamily::HardcodedSecret,
        &unit(Go, vec![binding(4, "apiToken", "tok_live_0123456789")]),
        promoted(),
    );
    let finding = &run.findings[0];
    assert_eq!(finding.evidence_class, EvidenceClass::C);
    assert_eq!(run.blocking, 1, "declared level is class C");

    let verifier = FindingVerifier::new(&store);
    assert!(
        !verifier.can_block(
            finding,
            &FindingGate::new(EvidenceClass::B, RiskLevel::High)
        ),
        "class C evidence must not satisfy a class B requirement"
    );
}

#[test]
fn a_candidate_secret_finding_does_not_block() {
    let (run, _) = structural(
        StructuralFamily::HardcodedSecret,
        &unit(Java, vec![binding(4, "apiToken", "tok_live_0123456789")]),
        RunAuthority::Candidate,
    );
    assert_eq!(run.findings.len(), 1);
    assert_eq!(run.blocking, 0);
}

#[test]
fn unsafe_rust_is_a_low_risk_marker_that_never_blocks_even_when_promoted_and_grounded() {
    let (run, store) = structural(
        StructuralFamily::UnsafeCode,
        &unit(Rust, vec![unsafe_fact(3, true), unsafe_fact(9, false)]),
        promoted(),
    );
    assert_eq!(
        run.verdict,
        PackVerdict::Found {
            nature: FindingNature::Risk,
            findings: 2
        }
    );
    assert_eq!(run.blocking, 0);
    for finding in &run.findings {
        assert_eq!(finding.kind.as_str(), "security.unsafe_code_risk");
        assert_eq!(finding.severity, FindingSeverity::Info);
        assert_eq!(finding.risk, RiskLevel::Low);
    }

    // Even the weakest gate someone could configure leaves it below the
    // pack's own bar: the risk level is Low, so a Medium gate rejects it.
    let verifier = FindingVerifier::new(&store);
    assert!(!verifier.can_block(
        &run.findings[0],
        &FindingGate::new(EvidenceClass::C, RiskLevel::Medium)
    ));
}

#[test]
fn justified_and_unjustified_unsafe_are_both_reported_at_their_own_lines() {
    let (run, _) = structural(
        StructuralFamily::UnsafeCode,
        &unit(Rust, vec![unsafe_fact(3, true), unsafe_fact(9, false)]),
        RunAuthority::Candidate,
    );
    let messages: Vec<&str> = run.findings.iter().map(|f| f.message.as_str()).collect();
    assert_eq!(messages.len(), 2);
    assert!(messages[0].ends_with("src/config:3"), "{messages:?}");
    assert!(messages[1].ends_with("src/config:9"), "{messages:?}");
}

// ---------------------------------------------------------------------------
// AC3 — JVM semantic context without a JVM scanner
// ---------------------------------------------------------------------------

fn jvm_call_site(language: SourceLanguage, jvm: Option<JvmContext>) -> ObservedFunction {
    ObservedFunction {
        jvm,
        ..function(
            language,
            vec![
                call(
                    10,
                    "javax.servlet.http.HttpServletRequest.getParameter",
                    vec![CallArg::Literal],
                    Some("id"),
                ),
                call(11, "Stmt.executeQuery", vec![value(&["id"])], None),
            ],
        )
    }
}

#[test]
fn jvm_context_turns_an_unresolved_call_site_into_a_blocking_finding() {
    let context = JvmContext::new().with_import("Stmt", "java.sql.Statement");
    for language in [Java, Kotlin] {
        let (without, _) = taint(
            Weakness::SqlInjection,
            &jvm_call_site(language, None),
            promoted(),
        );
        assert_eq!(
            without.verdict,
            PackVerdict::Unknown {
                unresolved: vec!["Stmt.executeQuery".to_string()]
            },
            "{language:?}: no context, no claim"
        );
        assert_eq!(without.blocking, 0);

        let (with, _) = taint(
            Weakness::SqlInjection,
            &jvm_call_site(language, Some(context.clone())),
            promoted(),
        );
        assert_eq!(with.findings.len(), 1, "{language:?}");
        assert_eq!(with.blocking, 1, "{language:?}");
    }
}

#[test]
fn a_pooled_subtype_reaches_the_jdk_sink_through_classpath_supertypes() {
    let context = JvmContext::new()
        .with_import("Stmt", "com.pool.PooledStatement")
        .with_supertype("com.pool.PooledStatement", "java.sql.Statement");
    let (run, _) = taint(
        Weakness::SqlInjection,
        &jvm_call_site(Java, Some(context)),
        promoted(),
    );
    assert_eq!(run.findings.len(), 1);
}

#[test]
fn a_context_that_names_an_unrelated_type_does_not_invent_a_sink() {
    let context = JvmContext::new().with_import("Stmt", "com.example.Dao");
    let (run, _) = taint(
        Weakness::SqlInjection,
        &jvm_call_site(Java, Some(context)),
        promoted(),
    );
    assert!(run.findings.is_empty());
    assert!(matches!(run.verdict, PackVerdict::Unknown { .. }));
}

#[test]
fn kotlin_stdlib_spellings_are_modeled_beside_the_shared_jdk_table() {
    let read = function(
        Kotlin,
        vec![
            call(
                10,
                "javax.servlet.http.HttpServletRequest.getParameter",
                vec![CallArg::Literal],
                Some("name"),
            ),
            call(11, "kotlin.io.readText", vec![value(&["name"])], None),
        ],
    );
    let (run, _) = taint(Weakness::PathTraversal, &read, promoted());
    assert_eq!(run.blocking, 1);

    let numeric = function(
        Kotlin,
        vec![
            call(
                10,
                "javax.servlet.http.HttpServletRequest.getParameter",
                vec![CallArg::Literal],
                Some("raw"),
            ),
            call(11, "kotlin.text.toInt", vec![value(&["raw"])], Some("n")),
            call(
                12,
                "java.sql.Statement.executeQuery",
                vec![value(&["n"])],
                None,
            ),
        ],
    );
    let (run, _) = taint(Weakness::SqlInjection, &numeric, promoted());
    assert!(run.findings.is_empty());
    assert!(matches!(run.verdict, PackVerdict::NotObserved { .. }));
}

// ---------------------------------------------------------------------------
// Family distinctions the model must keep
// ---------------------------------------------------------------------------

#[test]
fn a_sink_of_one_family_is_not_a_sink_of_another() {
    // Tainted data reaching a command sink is not SQL injection.
    let f = vulnerable(Go, Weakness::CommandInjection);
    let (run, _) = taint(Weakness::SqlInjection, &f, promoted());
    assert!(run.findings.is_empty());
    let (run, _) = taint(Weakness::CommandInjection, &f, promoted());
    assert_eq!(run.findings.len(), 1);
}

#[test]
fn a_basename_sanitizer_protects_paths_but_not_commands() {
    let a = apis(Go, Weakness::CommandInjection);
    let f = function(
        Go,
        vec![
            call(10, a.source, vec![CallArg::Literal], Some("input")),
            call(
                11,
                "path/filepath.Base",
                vec![value(&["input"])],
                Some("name"),
            ),
            call(12, a.sink, vec![value(&["name"])], None),
        ],
    );
    let (run, _) = taint(Weakness::CommandInjection, &f, promoted());
    assert_eq!(
        run.findings.len(),
        1,
        "reducing to a basename does not make a command safe"
    );
}

// ---------------------------------------------------------------------------
// AC4 — the capability matrix
// ---------------------------------------------------------------------------

fn matrix() -> CapabilityMatrix {
    SecurityPack::standard().capability_matrix()
}

#[test]
fn the_matrix_covers_every_language_and_family() {
    let m = matrix();
    for language in SourceLanguage::ALL {
        for family in SecurityFamily::ALL {
            assert!(m.get(language, family).is_some(), "{language:?} {family:?}");
        }
    }
}

#[test]
fn the_matrix_states_support_ceiling_and_blocking_per_cell() {
    let m = matrix();
    for language in MODELED {
        for weakness in Weakness::ALL {
            let cell = m
                .get(language, SecurityFamily::of_weakness(weakness))
                .unwrap();
            let Support::Supported {
                backend,
                evidence_ceiling,
                reach,
                can_block,
                models,
                ..
            } = &cell.support
            else {
                panic!("{language:?} {weakness:?} must be supported");
            };
            assert_eq!(backend, "m5_dataflow");
            assert_eq!(*evidence_ceiling, EvidenceClass::B);
            assert!(*can_block);
            assert!(reach.is_some());
            let models = models.expect("taint families publish model counts");
            assert!(models.sources > 0 && models.sinks > 0 && models.sanitizers > 0);
            assert!(!cell.limits.is_empty());
        }
    }
}

#[test]
fn the_matrix_states_what_is_not_supported() {
    let m = matrix();
    for language in [Java, Kotlin, Go] {
        let cell = m.get(language, SecurityFamily::UnsafeCode).unwrap();
        assert!(matches!(cell.support, Support::Unsupported { .. }));
    }
    let rust_unsafe = m.get(Rust, SecurityFamily::UnsafeCode).unwrap();
    assert_eq!(rust_unsafe.nature, FindingNature::Risk);
    assert_eq!(rust_unsafe.blocking, BlockingLevel::AdvisoryOnly);
    match &rust_unsafe.support {
        Support::Supported {
            can_block,
            evidence_ceiling,
            ..
        } => {
            assert!(!can_block);
            assert_eq!(*evidence_ceiling, EvidenceClass::C);
        }
        other => panic!("rust unsafe must be modeled, got {other:?}"),
    }
    for family in SecurityFamily::ALL {
        let cell = m.get(SourceLanguage::Python, family).unwrap();
        assert!(matches!(cell.support, Support::Unsupported { .. }));
    }
}

#[test]
fn jvm_cells_state_that_context_is_optional_and_no_scanner_is_required() {
    let m = matrix();
    for language in [Java, Kotlin] {
        let cell = m.get(language, SecurityFamily::SqlInjection).unwrap();
        assert!(
            cell.limits
                .iter()
                .any(|l| l.contains("optional") && l.contains("No JVM or Detekt scanner")),
            "{language:?}: {:?}",
            cell.limits
        );
    }
    let go = m.get(Go, SecurityFamily::SqlInjection).unwrap();
    assert!(go.limits.iter().all(|l| !l.contains("JVM")));
}

#[test]
fn the_matrix_agrees_with_what_the_runs_actually_do() {
    let m = matrix();
    for language in MODELED {
        for weakness in Weakness::ALL {
            let cell = m
                .get(language, SecurityFamily::of_weakness(weakness))
                .unwrap();
            let Support::Supported {
                backend,
                evidence_ceiling,
                detector_version,
                logic_digest,
                ..
            } = &cell.support
            else {
                panic!("supported");
            };
            let (run, _) = taint(
                weakness,
                &vulnerable(language, weakness),
                RunAuthority::Candidate,
            );
            let finding = &run.findings[0];
            assert_eq!(run.backend.as_deref(), Some(backend.as_str()));
            assert_eq!(finding.evidence_class, *evidence_ceiling);
            assert_eq!(&finding.detector.version, detector_version);
            assert_eq!(
                finding.detector.digests.logic.as_str(),
                logic_digest,
                "a result names the exact rule the matrix published"
            );
        }
    }
}

const INFRASTRUCTURE: [StructuralFamily; 3] = [
    StructuralFamily::InfraPublicExposure,
    StructuralFamily::InfraEncryption,
    StructuralFamily::InfraPermissions,
];

#[test]
fn infrastructure_families_are_modeled_for_terraform_only() {
    let m = matrix();
    for family in INFRASTRUCTURE {
        let family = SecurityFamily::of_structural(family);
        assert!(
            matches!(
                m.get(SourceLanguage::Terraform, family).unwrap().support,
                Support::Supported { .. }
            ),
            "{family:?}"
        );
        for language in MODELED {
            assert!(
                matches!(
                    m.get(language, family).unwrap().support,
                    Support::Unsupported { .. }
                ),
                "{language:?} {family:?} must not claim a model it does not have"
            );
        }
    }
    for weakness in Weakness::ALL {
        assert!(
            matches!(
                m.get(
                    SourceLanguage::Terraform,
                    SecurityFamily::of_weakness(weakness)
                )
                .unwrap()
                .support,
                Support::Unsupported { .. }
            ),
            "{weakness:?}"
        );
    }
}

#[test]
fn an_infrastructure_family_over_a_program_language_is_unsupported_never_clean() {
    for family in INFRASTRUCTURE {
        let (run, _) = structural(family, &unit(Rust, vec![]), promoted());
        assert!(
            matches!(run.verdict, PackVerdict::Unsupported { .. }),
            "{family:?}"
        );
        assert!(run.findings.is_empty());
    }
}

fn misconfiguration(
    line: u32,
    family: StructuralFamily,
    with_grounding: bool,
) -> StructuralObservation {
    StructuralObservation {
        line,
        grounding: with_grounding.then(|| {
            GroundingRef::entity(
                EntityId::new(u64::from(line)),
                FactId::new(600 + u64::from(line)),
            )
        }),
        fact: StructuralFact::Misconfiguration {
            family,
            check: "storage_encrypted".to_string(),
            resource: "aws_db_instance.main".to_string(),
            attribute: "storage_encrypted".to_string(),
            detail: "`storage_encrypted` is explicitly false".to_string(),
        },
    }
}

fn unknown_value(line: u32, family: StructuralFamily) -> StructuralObservation {
    StructuralObservation {
        line,
        grounding: None,
        fact: StructuralFact::UnknownValue {
            family,
            resource: "aws_db_instance.main".to_string(),
            attribute: "storage_encrypted".to_string(),
            causes: vec!["unbound variable var.encrypt".to_string()],
        },
    }
}

#[test]
fn a_verified_terraform_misconfiguration_blocks_only_when_promoted_and_grounded() {
    let family = StructuralFamily::InfraEncryption;
    let unit_of = |grounded| {
        unit(
            SourceLanguage::Terraform,
            vec![misconfiguration(7, family, grounded)],
        )
    };
    let (blocked, store) = structural(family, &unit_of(true), promoted());
    assert_eq!(blocked.blocking, 1);
    assert_eq!(blocked.findings[0].evidence_class, EvidenceClass::C);
    assert!(
        !FindingVerifier::new(&store).can_block(
            &blocked.findings[0],
            &FindingGate::new(EvidenceClass::B, RiskLevel::High)
        ),
        "structural evidence never satisfies a stronger class"
    );

    let (candidate, _) = structural(family, &unit_of(true), RunAuthority::Candidate);
    assert_eq!((candidate.findings.len(), candidate.blocking), (1, 0));
    let (floating, _) = structural(family, &unit_of(false), promoted());
    assert_eq!((floating.findings.len(), floating.blocking), (1, 0));
}

#[test]
fn an_unknown_terraform_value_never_becomes_a_finding_nor_a_clean_verdict() {
    let family = StructuralFamily::InfraEncryption;
    let (run, _) = structural(
        family,
        &unit(SourceLanguage::Terraform, vec![unknown_value(7, family)]),
        promoted(),
    );
    assert!(run.findings.is_empty());
    assert_eq!(run.blocking, 0);
    assert!(matches!(run.verdict, PackVerdict::Unknown { .. }));

    let (other_family, _) = structural(
        StructuralFamily::InfraPermissions,
        &unit(SourceLanguage::Terraform, vec![unknown_value(7, family)]),
        promoted(),
    );
    assert!(
        matches!(other_family.verdict, PackVerdict::NotObserved { .. }),
        "an unknown of one family says nothing about another"
    );
}

#[test]
fn the_matrix_is_publishable_json_and_round_trips() {
    let m = matrix();
    let json = serde_json::to_string(&m).expect("serializes");
    let back: CapabilityMatrix = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(back, m);
    assert!(json.contains("\"pack_version\":\"1.1.0\""));
}
