//! CU-1.3 — Presenter bridge CLI: traduce `AnalysisOutcome` al shape
//! N−1 `GraphResult::Full` SIN que el presentador orqueste servicios.
//!
//! El caso de uso (facade CU-1.2) produce el outcome; este módulo solo
//! traduce. El shape externo (`status`, `skipped_files`) se conserva
//! byte-compatible con lo que `CommandExecutor::execute_graph`
//! producía a mano (T1-5.5): ningún consumidor N−1 cambia.

use crate::application::analysis_basis::{AnalysisOutcome, AnalysisValue, OutcomeState};
use crate::interface::cli::result::{GraphResult, SkippedFile};
use crate::interface::coverage::PendingReferenceInfo;

/// Presenta un outcome de `graph.full` como `GraphResult::Full` N−1.
///
/// Reglas de traducción (paridad de semántica, CU-1.3):
/// - `Complete` → `status: "complete"`, `skipped_files: []`.
/// - `Partial`  → `status: "partial"`, gaps → `skipped_files`
///   (message con formato `"path: reason"`; el split lo hace el
///   presentador, no el consumidor).
/// - `Failed`   → el caller maneja el error ANTES de llamar aquí;
///   este presentador solo recibe outcomes con valor.
///
/// Las dos coberturas se publican en su sitio: los ficheros que no se
/// pudieron leer van en `skipped_files`, y las referencias que no se
/// pudieron atar a una declaración van en `pending_references`. Un
/// consumidor que pregunta "lo sabes todo?" recibe las dos respuestas.
pub fn present_graph_full(
    outcome: &AnalysisOutcome<AnalysisValue>,
    path: &str,
    rebuild: bool,
) -> GraphResult {
    let (symbols, dependencies, elapsed_ms, pending) = match outcome.value() {
        Some(AnalysisValue::Graph(crate::application::analysis_basis::GraphValue::Full {
            symbols,
            dependencies,
            elapsed_ms,
            pending_references,
            ..
        })) => (
            *symbols,
            *dependencies,
            *elapsed_ms,
            pending_references.clone(),
        ),
        _ => (0, 0, 0, Vec::new()),
    };

    let (status, skipped_files) = match outcome.state() {
        OutcomeState::Partial => (
            "partial".to_string(),
            outcome
                .gaps()
                .iter()
                // Cada gap dice la cobertura que falta. Los de ficheros
                // saltados son los únicos que describen un fichero; el de
                // referencias pendientes describe otra cosa, y leerlo como
                // un fichero saltado sería inventar un hueco que no está.
                .filter(|gap| {
                    gap.capability.as_deref()
                        == Some(crate::application::analysis_basis::SKIPPED_FILE_CAPABILITY)
                })
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
        OutcomeState::Complete => ("complete".to_string(), Vec::new()),
        // Un veredicto que no es un veredicto no es "complete". El
        // consumidor tiene que poder distinguir "lo sé todo" de "no he
        // podido responder", y el MCP usa la misma palabra para las dos
        // cosas que no son un veredicto.
        _ => ("unknown".to_string(), Vec::new()),
    };

    GraphResult::Full {
        path: path.to_string(),
        rebuild,
        elapsed_ms: elapsed_ms.max(outcome.provenance().elapsed_ms),
        symbols,
        dependencies,
        status,
        skipped_files,
        pending_references: pending
            .iter()
            .map(PendingReferenceInfo::from_reference)
            .collect(),
    }
}
