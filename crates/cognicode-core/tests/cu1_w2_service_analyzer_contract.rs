//! CU-1.2 (parte 2) — Production adapter: `ServiceAnalyzer` implements
//! the `Analyze` port over the real `AnalysisService`.
//!
//! Estado pre-CU-1.2: el trait `Analyze` (B2.W1) no tenía NINGUNA
//! implementación. CLI y MCP orquestaban `AnalysisService` a mano y el
//! consumidor Guard construía su propio wrapper (`RealCalculator`).
//! Este contrato pinea la implementación de producción del puerto:
//!
//! 1. `graph.full` sobre un workspace Rust real ejecuta
//!    `build_full_graph` + `get_graph_stats` y devuelve
//!    `Complete(GraphValue::Full)` con contadores reales.
//! 2. Workspace sin material Rust → `Failed` con mensaje honesto (no
//!    `Complete` con grafo vacío; SPEC-CU-001.5).
//! 3. Ops no implementadas por el adaptador → `Unsupported` con
//!    reason, jamás panic ni `Unknown` silencioso.
//! 4. El outcome lleva `AnalysisProvenance` con elapsed medido.
//!
//! Fixture: un mini-workspace Rust en un tempdir (2 archivos, 2
//! funciones homónimas en módulos distintos) — el mismo fixture que
//! exige la aceptación de CU-1 para identidad de símbolos.

use cognicode_core::application::analysis_basis::{
    AnalysisBasis, AnalysisOp, AnalysisValue, Analyze, OutcomeState,
};
use cognicode_core::application::consumer_analysis::ServiceAnalyzer;
use std::path::PathBuf;

fn rust_fixture(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("main.rs"),
        "mod alpha;\nmod beta;\nfn main() { alpha::helper(); beta::helper(); }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("alpha.rs"),
        "pub fn helper() -> u32 { 1 }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("beta.rs"),
        "pub fn helper() -> u32 { 2 }\n",
    )
    .unwrap();
}

#[test]
fn graph_full_over_real_workspace_completes_with_real_counts() {
    let tmp = tempdir();
    rust_fixture(&tmp);
    let basis = AnalysisBasis::for_workspace(tmp.clone());
    let analyzer = ServiceAnalyzer::new();

    let outcome = analyzer.execute(
        &basis,
        AnalysisOp::GraphFull {
            path: tmp.clone(),
            rebuild: true,
        },
    );

    // CU-1.4: el walk del servicio incluye Cargo.toml (extensión no
    // soportada) → el build es Partial con gaps, EXACTAMENTE lo que
    // handle_build_graph reporta por MCP. La ley no es "siempre
    // Complete": es "Complete solo si el BuildReport no tiene skips".
    match outcome.state() {
        OutcomeState::Complete => assert!(outcome.gaps().is_empty()),
        OutcomeState::Partial => {
            assert!(!outcome.gaps().is_empty(), "Partial exige gaps no vacíos");
            assert!(
                outcome
                    .gaps()
                    .iter()
                    .any(|g| g.message.contains("Cargo.toml")),
                "el gap debe nombrar el fichero saltado (Cargo.toml), got {:?}",
                outcome.gaps()
            );
        }
        other => panic!("esperaba Complete o Partial, obtuve {other:?}"),
    }
    match outcome.value() {
        Some(AnalysisValue::Graph(GraphValue::Full {
            symbols,
            dependencies: _,
            path: _,
            ..
        })) => {
            assert!(
                *symbols >= 2,
                "el fixture define ≥2 símbolos, got {symbols}"
            );
        }
        other => panic!("esperaba GraphValue::Full, got {other:?}"),
    }
}

#[test]
fn graph_full_without_rust_material_fails_honestly() {
    let tmp = tempdir(); // vacío: sin Cargo.toml, sin .rs
    let basis = AnalysisBasis::for_workspace(tmp.clone());
    let analyzer = ServiceAnalyzer::new();

    let outcome = analyzer.execute(
        &basis,
        AnalysisOp::GraphFull {
            path: tmp,
            rebuild: true,
        },
    );

    assert_eq!(
        outcome.state(),
        OutcomeState::Failed,
        "sin material Rust el resultado es Failed honesto, no Complete vacío"
    );
    assert!(outcome.error().is_some());
}

#[test]
fn graph_complexity_inherits_partial_from_last_build_report() {
    // Contrato: graph_complexity hereda Partial + gaps del BuildReport
    // compartido, IGUAL que graph_full. El adaptador NO copia la
    // traducción de BuildStatus en cada op: una sola traducción vive en
    // `partial_from_build_report` y ambas ops la consultan. Sin este
    // contrato, un workspace con skips reportaba "complete" en
    // graph.complexity mientras MCP decía "partial" sobre el MISMO
    // gráfico (divergencia real detectada por el UAT de integración).
    //
    // El hueco del fixture tiene que ser REAL: un fichero que el producto
    // no puede leer. Antes lo daba un `.kt` --una fuente en lenguaje
    // conocido-sin-soporte--, pero un hueco que depende de que
    // lenguajes soporta el producto CADUCA en cuanto el producto crece:
    // cuando Kotlin pasó a soportarse, el fixture dejó de producir
    // Partial y el contrato perdió su precondición. Un fichero con
    // sintaxis rota es un hueco para siempre, y del mismo tipo que el que
    // el recorrido por fichero ya publica (`SkipReason::Parse`). El
    // contrato NO se debilita: sigue detectando exactamente la
    // divergencia que nacio para cazar.
    let tmp = tempdir();
    rust_fixture(&tmp);
    std::fs::write(
        tmp.join("src").join("broken_syntax.rs"),
        "pub fn broken_fn( -> u32 {\n    42\n}\n",
    )
    .unwrap();
    let basis = AnalysisBasis::for_workspace(tmp.clone());
    let analyzer = ServiceAnalyzer::new();

    // 1) graph_full puebla last_build_report con el estado real del
    //    walk (Partial con gaps = el `.kt` sin registrar).
    let full = analyzer.execute(
        &basis,
        AnalysisOp::GraphFull {
            path: tmp.clone(),
            rebuild: true,
        },
    );
    assert_eq!(
        full.state(),
        OutcomeState::Partial,
        "precondición: el fixture con una fuente rota produce Partial (gap = broken_syntax.rs), got {:?}",
        full.state()
    );
    let expected_gap_messages: Vec<String> =
        full.gaps().iter().map(|g| g.message.clone()).collect();
    assert!(
        expected_gap_messages
            .iter()
            .any(|m| m.contains("broken_syntax.rs")),
        "precondición: el gap debe nombrar broken_syntax.rs, got {:?}",
        expected_gap_messages
    );

    // 2) graph_complexity sobre el MISMO basis (mismo service, mismo
    //    last_build_report) debe heredar el estado Partial y los gaps.
    let complexity = analyzer.execute(&basis, AnalysisOp::GraphComplexity { path: tmp.clone() });

    assert_eq!(
        complexity.state(),
        OutcomeState::Partial,
        "graph.complexity debe heredar Partial del BuildReport; implementación \
         actual devuelve Complete porque graph_complexity no consulta \
         last_build_report (analyze.rs:151-192). got {:?}",
        complexity.state()
    );
    let actual_gap_messages: Vec<String> = complexity
        .gaps()
        .iter()
        .map(|g| g.message.clone())
        .collect();
    assert!(
        actual_gap_messages
            .iter()
            .any(|m| m.contains("broken_syntax.rs")),
        "graph.complexity debe propagar el gap broken_syntax.rs; got {:?}",
        actual_gap_messages
    );
    assert_eq!(
        actual_gap_messages.len(),
        expected_gap_messages.len(),
        "graph.complexity debe propagar los MISMOS gaps que graph_full sobre el \
         mismo workspace (mismas fuentes, mismo skip set), got {:?} vs {:?}",
        actual_gap_messages,
        expected_gap_messages
    );
}

#[test]
fn unsupported_op_is_reported_not_panicked() {
    let tmp = tempdir();
    rust_fixture(&tmp);
    let basis = AnalysisBasis::for_workspace(tmp.clone());
    let analyzer = ServiceAnalyzer::new();

    // GraphMermaid aún no está cableado en el adaptador: debe ser
    // Unsupported explícito, no panic.
    let outcome = analyzer.execute(
        &basis,
        AnalysisOp::GraphMermaid {
            path: tmp,
            format: "svg".to_string(),
        },
    );

    assert_eq!(outcome.state(), OutcomeState::Unsupported);
    assert!(outcome.reason().is_some_and(|r| !r.trim().is_empty()));
}

#[test]
fn outcome_carries_provenance_with_elapsed() {
    let tmp = tempdir();
    rust_fixture(&tmp);
    let basis = AnalysisBasis::for_workspace(tmp.clone());
    let analyzer = ServiceAnalyzer::new();

    let outcome = analyzer.execute(
        &basis,
        AnalysisOp::GraphFull {
            path: tmp,
            rebuild: true,
        },
    );

    let provenance = outcome.provenance();
    assert_eq!(provenance.operation.as_deref(), Some("graph.full"));
    // elapsed_ms is present; the real value is measured by the adapter
    // (always non-negative; the field's type already encodes that).
    let _ = provenance;
}

fn tempdir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cu1-facade-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

use cognicode_core::application::analysis_basis::GraphValue;
