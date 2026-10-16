//! CU-1.3 — Presenter bridge MCP: traduce `AnalysisOutcome` al shape
//! existente `BuildGraphOutput` SIN que el handler orqueste de nuevo.
//!
//! La caché/manifests del handler `handle_build_graph` sigue siendo
//! suya (es política de transporte, no del caso de uso); este módulo
//! existe para que el MISMO outcome que consume el SDK/CLI pueda
//! presentarse por MCP con la misma semántica de estado: "complete" /
//! "partial" / "unknown", y `skipped_files: Some(..)` sólo cuando hubo
//! walk con omisiones.

use crate::application::analysis_basis::{AnalysisOutcome, AnalysisValue, OutcomeState};
use crate::interface::mcp::handlers::{BuildGraphOutput, SkippedFileDto};

/// Presenta un outcome de `graph.full` como `BuildGraphOutput`.
///
/// `loaded_from_cache` reproduce la semántica "unknown" del handler
/// actual: servido de caché sin walk en ESTA llamada.
pub fn present_build_graph(
    outcome: &AnalysisOutcome<AnalysisValue>,
    loaded_from_cache: bool,
) -> BuildGraphOutput {
    let (symbols, relationships) = match outcome.value() {
        Some(AnalysisValue::Graph(crate::application::analysis_basis::GraphValue::Full {
            symbols,
            dependencies,
            ..
        })) => (*symbols, *dependencies),
        _ => (0, 0),
    };

    let (status, skipped) = match outcome.state() {
        OutcomeState::Partial => (
            "partial".to_string(),
            Some(
                outcome
                    .gaps()
                    .iter()
                    .filter_map(|gap| {
                        gap.message.split_once(": ").map(|(p, r)| SkippedFileDto {
                            path: p.to_string(),
                            reason_kind: "gap",
                            reason: r.to_string(),
                        })
                    })
                    .collect::<Vec<_>>(),
            ),
        ),
        OutcomeState::Complete if loaded_from_cache => ("unknown".to_string(), None),
        OutcomeState::Complete => ("complete".to_string(), Some(Vec::new())),
        _ => ("unknown".to_string(), None),
    };

    BuildGraphOutput {
        success: !matches!(outcome.state(), OutcomeState::Failed),
        status,
        symbols_found: symbols,
        relationships_found: relationships,
        edges: Vec::new(),
        message: String::new(),
        skipped_files: skipped,
        basis: None,
    }
}
