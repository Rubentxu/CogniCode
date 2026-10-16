//! CU-1.3 — Presenter bridge CLI: traduce `AnalysisOutcome` al shape
//! N−1 `GraphResult::Full` SIN que el presentador orqueste servicios.
//!
//! El caso de uso (facade CU-1.2) produce el outcome; este módulo solo
//! traduce. El shape externo (`status`, `skipped_files`) se conserva
//! byte-compatible con lo que `CommandExecutor::execute_graph`
//! producía a mano (T1-5.5): ningún consumidor N−1 cambia.

use crate::application::analysis_basis::{AnalysisOutcome, AnalysisValue, OutcomeState};
use crate::interface::cli::result::{GraphResult, SkippedFile};

/// Presenta un outcome de `graph.full` como `GraphResult::Full` N−1.
///
/// Reglas de traducción (paridad de semántica, CU-1.3):
/// - `Complete` → `status: "complete"`, `skipped_files: []`.
/// - `Partial`  → `status: "partial"`, gaps → `skipped_files`
///   (message con formato `"path: reason"`; el split lo hace el
///   presentador, no el consumidor).
/// - `Failed`   → el caller maneja el error ANTES de llamar aquí;
///   este presentador solo recibe outcomes con valor.
pub fn present_graph_full(
    outcome: &AnalysisOutcome<AnalysisValue>,
    path: &str,
    rebuild: bool,
) -> GraphResult {
    let (symbols, dependencies, elapsed_ms) = match outcome.value() {
        Some(AnalysisValue::Graph(crate::application::analysis_basis::GraphValue::Full {
            symbols,
            dependencies,
            elapsed_ms,
            ..
        })) => (*symbols, *dependencies, *elapsed_ms),
        _ => (0, 0, 0),
    };

    let (status, skipped_files) = match outcome.state() {
        OutcomeState::Partial => (
            "partial".to_string(),
            outcome
                .gaps()
                .iter()
                .filter_map(|gap| {
                    // El gap message llega como "path: reason" cuando
                    // viene de un fichero saltado; capability es el
                    // nombre de la capability afectada.
                    gap.message.split_once(": ").map(|(p, r)| SkippedFile {
                        path: p.to_string(),
                        reason: r.to_string(),
                    })
                })
                .collect(),
        ),
        _ => ("complete".to_string(), Vec::new()),
    };

    GraphResult::Full {
        path: path.to_string(),
        rebuild,
        elapsed_ms: elapsed_ms.max(outcome.provenance().elapsed_ms),
        symbols,
        dependencies,
        status,
        skipped_files,
    }
}
