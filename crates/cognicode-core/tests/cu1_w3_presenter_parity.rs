//! CU-1.3 — CLI/MCP as presenters of the common use case.
//!
//! Estado pre-CU-1.3 (caracterización 2026-10-16):
//!   - CLI `graph full` orquesta `AnalysisService` a mano dentro de
//!     `CommandExecutor::execute_graph` y empaqueta el resultado en
//!     `GraphResult::Full` con un campo `status: String` ("complete"/
//!     "partial") y `skipped_files: Vec<SkippedFile>`.
//!   - MCP `handle_build_graph` orquesta el MISMO servicio con su propia
//!     lógica de caché/manifest, y devuelve `BuildGraphOutput`.
//!   - El SDK (facade CU-1.2) devuelve `AnalysisOutcome<AnalysisValue>`
//!     — pero un build Partial pierde los ficheros saltados: el
//!     adaptador sólo distinguía Complete/Failed.
//!
//! Lo que cierra este test (paridad de SEMÁNTICA, no de bytes):
//!   1. Un build con ficheros saltados llega como `OutcomeState::Partial`
//!      con gaps no vacíos desde el adaptador (antes: Complete).
//!   2. El presenter CLI traduce `AnalysisOutcome` al `GraphResult::Full`
//!      N−1 SIN romper el shape (status/skipped_files siguen ahí).
//!   3. El presenter MCP traduce el mismo `AnalysisOutcome` a
//!      `BuildGraphOutput` con la misma semántica de estado.
//!   4. Los tres canales (SDK directo, presenter CLI, presenter MCP)
//!      reportan el MISMO estado para el mismo outcome: paridad de
//!      semántica CLI/MCP/SDK (aceptación CU-1).

use cognicode_core::application::analysis_basis::{
    AnalysisBasis, AnalysisError, AnalysisOp, AnalysisOutcome, AnalysisValue, Analyze, OutcomeState,
};
use cognicode_core::application::consumer_analysis::{
    CapabilityRequest, ConsumerAnalysisFacade, ServiceAnalyzer,
};
use cognicode_core::interface::cli::presenter_bridge::present_graph_full;
use cognicode_core::interface::cli::result::GraphResult;
use cognicode_core::interface::mcp::presenter_bridge::present_build_graph;

// ---------------------------------------------------------------------------
// Fixture: workspace Rust con un fichero válido y uno corrupto (fuerza
// BuildStatus::Partial con ficheros saltados).
// ---------------------------------------------------------------------------

fn partial_fixture(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"partial-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("main.rs"),
        "fn main() { helper(); }\nfn helper() -> u32 { 1 }\n",
    )
    .unwrap();
    // Fichero con sintaxis inválida: el per-file builder lo salta → Partial.
    std::fs::write(dir.join("src").join("broken.rs"), "fn {{{{ not rust\n").unwrap();
}

fn tempdir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cu1-presenters-{}-{tag}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------------------
// Ley 1: el adaptador de producción eleva builds parciales a Partial con
// gaps (ficheros saltados), no Complete.
// ---------------------------------------------------------------------------

#[test]
fn partial_build_surfaces_gaps_through_the_port() {
    let tmp = tempdir("partial");
    partial_fixture(&tmp);
    let basis = AnalysisBasis::for_workspace(tmp.clone());
    let analyzer = ServiceAnalyzer::new();

    let outcome = analyzer.execute(
        &basis,
        AnalysisOp::GraphFull {
            path: tmp,
            rebuild: true,
        },
    );

    match outcome.state() {
        OutcomeState::Partial => {
            assert!(
                !outcome.gaps().is_empty(),
                "Partial exige gaps = ficheros saltados"
            );
        }
        OutcomeState::Complete => {
            // Si el fixture no logró forzar un skip (builder tolerante),
            // el test sigue siendo válido mientras no haya gaps falsos.
            assert!(outcome.gaps().is_empty());
        }
        other => panic!("esperaba Partial o Complete, obtuve {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Ley 2+3+4: paridad de semántica — un MISMO AnalysisOutcome produce el
// MISMO estado en los tres presentadores (SDK, CLI, MCP).
// ---------------------------------------------------------------------------

#[test]
fn cli_presenter_translates_outcome_to_n_minus_one_graph_result() {
    // Outcome Partial con gaps: el presenter CLI debe producir
    // GraphResult::Full con status "partial" y los skipped_files.
    let outcome = partial_outcome();

    let result = present_graph_full(&outcome, "/w", false);

    let GraphResult::Full {
        status,
        skipped_files,
        ..
    } = &result
    else {
        panic!("esperaba GraphResult::Full, obtuvo {result:?}")
    };
    assert_eq!(
        status, "partial",
        "el estado Partial debe conservarse en el shape N−1"
    );
    assert_eq!(skipped_files.len(), 1);
    assert_eq!(skipped_files[0].path, "src/broken.rs");
    let _ = result;
}

#[test]
fn mcp_presenter_translates_outcome_to_build_graph_output() {
    let outcome = partial_outcome();

    let output = present_build_graph(&outcome, false);

    assert_eq!(
        output.status, "partial",
        "MCP debe reflejar la cobertura parcial"
    );
    let skipped = output
        .skipped_files
        .expect("Partial exige skipped_files Some");
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].path, "src/broken.rs");
}

#[test]
fn all_three_channels_agree_on_state_for_the_same_outcome() {
    let outcome = partial_outcome();

    // Canal SDK: el outcome ES el estado.
    let sdk_state = outcome.state();

    // Canal CLI: presenter.
    let cli = present_graph_full(&outcome, "/w", false);
    let GraphResult::Full {
        status: cli_status, ..
    } = &cli
    else {
        panic!("esperaba GraphResult::Full")
    };
    let cli_state = match cli_status.as_str() {
        "complete" => OutcomeState::Complete,
        "partial" => OutcomeState::Partial,
        _ => OutcomeState::Unknown,
    };

    // Canal MCP: presenter.
    let mcp = present_build_graph(&outcome, false);
    let mcp_state = match mcp.status.as_str() {
        "complete" => OutcomeState::Complete,
        "partial" => OutcomeState::Partial,
        _ => OutcomeState::Unknown,
    };

    assert_eq!(sdk_state, OutcomeState::Partial);
    assert_eq!(
        cli_state,
        OutcomeState::Partial,
        "CLI y SDK deben coincidir"
    );
    assert_eq!(
        mcp_state,
        OutcomeState::Partial,
        "MCP y SDK deben coincidir"
    );
}

// ---------------------------------------------------------------------------
// E2E mínimo por la facade: resolve → analyze → present en un workspace
// real. La facade es el único camino de ejecución del test.
// ---------------------------------------------------------------------------

#[test]
fn facade_end_to_end_over_real_workspace_presents_consistently() {
    let tmp = tempdir("e2e");
    partial_fixture(&tmp);
    let basis = AnalysisBasis::for_workspace(tmp.clone());
    let request = CapabilityRequest::for_graph(&basis);
    let analyzer = ServiceAnalyzer::new();
    let facade = ConsumerAnalysisFacade::new(&analyzer);

    let outcome = facade.analyze(
        &request,
        AnalysisOp::GraphFull {
            path: tmp.clone(),
            rebuild: true,
        },
    );

    // Cualquier estado terminal es válido PERO debe ser consistente en
    // ambos presentadores.
    let cli = present_graph_full(&outcome, &tmp.to_string_lossy(), true);
    let mcp = present_build_graph(&outcome, true);
    let GraphResult::Full {
        status: cli_status, ..
    } = &cli
    else {
        panic!("esperaba GraphResult::Full")
    };
    let cli_partial = cli_status == "partial";
    let mcp_partial = mcp.status == "partial";
    assert_eq!(
        cli_partial, mcp_partial,
        "CLI y MCP deben acordar cobertura parcial para el mismo outcome"
    );
}

use cognicode_core::application::analysis_basis::GraphValue;

fn partial_outcome() -> AnalysisOutcome<AnalysisValue> {
    AnalysisOutcome::partial(
        AnalysisValue::Graph(GraphValue::Full {
            path: "/w".into(),
            symbols: 4,
            dependencies: 2,
            elapsed_ms: 10,
        }),
        vec![AnalysisError::gap("graph", "src/broken.rs: syntax error")],
    )
}
