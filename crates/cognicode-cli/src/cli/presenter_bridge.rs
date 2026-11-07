//! CU-1.3 — Presenter bridge CLI: traduce `AnalysisOutcome` al shape
//! N−1 `GraphResult::Full` SIN que el presentador orqueste servicios.
//!
//! El caso de uso (facade CU-1.2) produce el outcome; este módulo solo
//! traduce. El shape externo (`status`, `skipped_files`) se conserva
//! byte-compatible con lo que `CommandExecutor::execute_graph`
//! producía a mano (T1-5.5): ningún consumidor N−1 cambia.

use crate::cli::result::{GraphResult, SkippedFile};
use cognicode_core::application::analysis_basis::{
    AnalysisOutcome, AnalysisValue, GraphSource, OutcomeState,
};
use cognicode_core::interface::coverage::{PendingReferenceInfo, RejectedRelationInfo};

/// Presenta un outcome de `graph.full` como `GraphResult::Full` N−1.
///
/// Reglas de traducción (paridad de semántica, CU-1.3):
/// - `Complete` → `status: "complete"`, `skipped_files: []`.
/// - `Partial`  → `status: "partial"`, gaps → `skipped_files`
///   (message con formato `"path: reason"`; el split lo hace el
///   presentador, no el consumidor).
/// - `Failed`   → el caller maneja el error ANTES de llamar aquí;
///   este presentador solo recibe outcomes con valor.
/// - `Unsupported` → `status: "unsupported"`: el walk corrió y no
///   encontró nada que leer. La palabra la nombra el estado y no este
///   módulo, para que las tres superficies publiquen el mismo
///   vocabulario sobre el mismo árbol.
///
/// Las tres coberturas se publican en su sitio: los ficheros que no se
/// pudieron leer van en `skipped_files`, las referencias que no se pudieron
/// atar a una declaración van en `pending_references`, y las relaciones
/// que el almacén rechazó van en `rejected_relations`. Un consumidor que
/// pregunta "lo sabes todo?" recibe las tres respuestas.
pub fn present_graph_full(
    outcome: &AnalysisOutcome<AnalysisValue>,
    path: &str,
    rebuild: bool,
) -> GraphResult {
    let (symbols, dependencies, elapsed_ms, pending, rejected) = match outcome.value() {
        Some(AnalysisValue::Graph(
            cognicode_core::application::analysis_basis::GraphValue::Full {
                symbols,
                dependencies,
                elapsed_ms,
                pending_references,
                rejected_relations,
                ..
            },
        )) => (
            *symbols,
            *dependencies,
            *elapsed_ms,
            pending_references.clone(),
            rejected_relations.clone(),
        ),
        _ => (0, 0, 0, Vec::new(), Vec::new()),
    };

    let status = outcome.state().published_label().to_string();

    // Los gaps de un Partial son las coberturas que faltan. Los de
    // ficheros saltados son los únicos que describen un fichero; el de
    // referencias pendientes describe otra cosa, y leerlo como un
    // fichero saltado sería inventar un hueco que no está.
    let skipped_files = match outcome.state() {
        OutcomeState::Partial => outcome
            .gaps()
            .iter()
            .filter(|gap| {
                gap.capability.as_deref()
                    == Some(cognicode_core::application::analysis_basis::SKIPPED_FILE_CAPABILITY)
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
        _ => Vec::new(),
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
        rejected_relations: rejected
            .iter()
            .map(RejectedRelationInfo::from_rejection)
            .collect(),
    }
}

/// Dice, en stderr, de donde salio el grafo y que no cubrio.
///
/// El documento de stdout no cambia: la respuesta es la respuesta de las
/// dos maneras. Lo que cambia es lo que la CLI dice sobre como llego a
/// ella, y eso es observabilidad: va a stderr, donde el que lee una tuberia
/// lo ve y el que consume el documento no se confunde.
///
/// Las lineas de "Skipped" y "Pending" van a stderr (como el eprintln!
/// anterior a la migracion). El presentador de texto del binario las
/// emite tambien via los campos `status` y `skipped_files`; mantenerlas
/// aqui conserva el flujo de stderr historico para las tuberias que
/// grepean "Building ..." o "Skipped:" por separado de stdout.
pub fn report_graph_full(outcome: &AnalysisOutcome<AnalysisValue>, result: &GraphResult) {
    let GraphResult::Full {
        skipped_files,
        pending_references,
        status,
        symbols,
        dependencies,
        ..
    } = result
    else {
        return;
    };

    // La fuente del grafo viaja con la respuesta: un grafo calculado ahora
    // y uno servido del snapshot son la misma respuesta solo cuando el
    // arbol no cambio, y quien consume es quien tiene que saber que el
    // producto lo comprobo —y quien lo comprobo.
    //
    // The phrase is the one the provenance enum publishes: the CLI and the
    // MCP say the same thing about the same source, and a word that means
    // two things is a lie the consumer cannot see.
    match outcome.provenance().graph_source {
        Some(source) if source != GraphSource::Built => eprintln!(
            "Graph loaded from {}: {symbols} symbols, {dependencies} relationships",
            source.published_message()
        ),
        _ => {}
    }

    // El motivo va a stderr, que es donde vive la observabilidad: el
    // documento de stdout lleva el estado y el motivo se explica al
    // que mira la tubería.
    if let Some(reason) = outcome.reason() {
        eprintln!("  {reason}");
    }

    if status == "partial" {
        for s in skipped_files {
            eprintln!("  Skipped: {}: {}", s.path, s.reason);
        }
        for reference in pending_references {
            // Una referencia sin linea no esta en la linea cero: esta en
            // una linea que el lector no conocia, y decir "0" nombraria un
            // sitio que no existe.
            let origin = match reference.line {
                Some(line) => format!("{}:{}", reference.file, line),
                None => reference.file.clone(),
            };
            eprintln!(
                "  Pending: {} -> {} ({:?})",
                origin, reference.target, reference.reason
            );
        }
    }
}
