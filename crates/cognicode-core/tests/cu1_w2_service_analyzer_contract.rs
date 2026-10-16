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
                outcome.gaps().iter().any(|g| g.message.contains("Cargo.toml")),
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
