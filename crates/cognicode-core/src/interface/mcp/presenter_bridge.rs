//! CU-1.3 — Presenter bridge MCP: traduce `AnalysisOutcome` al shape
//! existente `BuildGraphOutput` SIN que el handler orqueste de nuevo.
//!
//! La caché/manifests del handler `handle_build_graph` sigue siendo
//! suya (es política de transporte, no del caso de uso); este módulo
//! existe para que el MISMO outcome que consume el SDK/CLI pueda
//! presentarse por MCP con la misma semántica de estado: la palabra la
//! nombra el estado (`OutcomeState::published_label`), y
//! `skipped_files: Some(..)` sólo cuando hubo walk con omisiones.

use crate::application::analysis_basis::{AnalysisOutcome, AnalysisValue, OutcomeState};
use crate::interface::mcp::handlers::{BuildGraphOutput, SkippedFileDto};

/// Presenta un outcome de `graph.full` como `BuildGraphOutput`.
///
/// La palabra la nombra el estado y sólo el estado: el presentador no
/// decide. Una respuesta servida de la caché es la MISMA respuesta que la
/// construida en frío —la cobertura se comprobó contra el árbol de ahora—,
/// así que se publica con la misma palabra; el handler vivo ya la publicaba
/// así, y este módulo era la segunda respuesta al mismo caso.
pub fn present_build_graph(outcome: &AnalysisOutcome<AnalysisValue>) -> BuildGraphOutput {
    let (symbols, relationships, pending, rejected) = match outcome.value() {
        Some(AnalysisValue::Graph(crate::application::analysis_basis::GraphValue::Full {
            symbols,
            dependencies,
            pending_references,
            rejected_relations,
            ..
        })) => (
            *symbols,
            *dependencies,
            pending_references
                .iter()
                .map(crate::interface::coverage::PendingReferenceInfo::from_reference)
                .collect(),
            rejected_relations
                .iter()
                .map(crate::interface::coverage::RejectedRelationInfo::from_rejection)
                .collect(),
        ),
        _ => (0, 0, Vec::new(), Vec::new()),
    };

    let (status, skipped) = match outcome.state() {
        OutcomeState::Partial => (
            OutcomeState::Partial.published_label().to_string(),
            Some(
                outcome
                    .gaps()
                    .iter()
                    // Only the gaps about files describe a file. The gap
                    // about pending references describes the other
                    // coverage, and reading it as a skipped file would
                    // invent a gap that is not there.
                    .filter(|gap| {
                        gap.capability.as_deref()
                            == Some(crate::application::analysis_basis::SKIPPED_FILE_CAPABILITY)
                    })
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
        OutcomeState::Complete => (
            OutcomeState::Complete.published_label().to_string(),
            Some(Vec::new()),
        ),
        // La palabra la nombra el estado: un árbol sin nada que leer es
        // "unsupported" aquí y en el handler, y no "unknown" en uno de
        // los dos.
        other => (other.published_label().to_string(), None),
    };

    BuildGraphOutput {
        success: !matches!(outcome.state(), OutcomeState::Failed),
        status,
        symbols_found: symbols,
        relationships_found: relationships,
        edges: Vec::new(),
        message: String::new(),
        skipped_files: skipped,
        pending_references: Some(pending),
        rejected_relationships: Some(rejected),
        basis: None,
    }
}
