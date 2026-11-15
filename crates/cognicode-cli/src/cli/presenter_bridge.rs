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

/// El tope de bytes del volcado de detalle por stderr.
///
/// El volcado es una muestra con su recuento, no una lista. MEDIDO el
/// 2026-11-15 sobre este repositorio: 44 905 referencias sin declarar
/// son 4,27 MB de stderr que nadie lee y que tapa el error que sí
/// importa.
///
/// 32 KiB es el número que sale de esa medición: los 135 ficheros que no
/// se pudieron leer son 24 KB con sus motivos —la línea de cada fichero,
/// que es la que un consumidor cuenta, una por fichero—, y el resto del
/// tope es la muestra de las referencias sin declarar. Un tope más corto
/// recortaría los ficheros, que son el contrato; uno más largo dejaría
/// pasar el volumen.
pub const MAX_DETAIL_BYTES: usize = 32 * 1024;

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
///
/// El volcado tiene un tope de bytes y un recuento exacto de lo que
/// queda fuera: [`report_graph_detail`] es su dueño y explica como.
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
        let mut err = std::io::stderr();
        let _ = report_graph_detail(skipped_files, pending_references, &mut err);
    }
}

/// Vuelca el detalle de una construcción parcial: los ficheros que no se
/// pudieron leer y las referencias que nadie declaró.
///
/// El volcado vive aparte de [`report_graph_full`] porque es lo que
/// crece: son una línea por fichero y una por referencia, y su tamaño
/// depende del árbol, no de la orden. Escribe en el escritor que recibe,
/// que es lo que lo hace comprobable.
///
/// ## Las dos reglas
///
/// La primera: las líneas van como antes, una por fichero y una por
/// referencia, para que el grep que las busca las encuentre.
///
/// La segunda: el volcado no crece sin tope —cabe en [`MAX_DETAIL_BYTES`]—
/// y lo que no cabe se cuenta y se dice, con el número exacto y el sitio
/// donde está todo. Un volcado que se corta en silencio miente: el que lo
/// lee tiene que saber que hay más de lo que ve.
///
/// Las líneas de resumen no llevan el prefijo de las que cuentan: un
/// consumidor que cuenta líneas `Skipped:` tiene que contar ficheros, no
/// ficheros más un resumen.
pub fn report_graph_detail(
    skipped_files: &[SkippedFile],
    pending_references: &[PendingReferenceInfo],
    w: &mut dyn std::io::Write,
) -> std::io::Result<()> {
    // Lo que se reserva para las líneas de resumen: el recorte tiene que
    // poder decir lo que dejó fuera, y lo que dice no puede quedar fuera.
    const RESERVED_FOR_SUMMARY: usize = 512;

    let mut written = 0usize;
    let mut skipped_listed = 0usize;
    let mut pending_listed = 0usize;

    // Los ficheros que no se pudieron leer van primero: su línea es la
    // que un consumidor cuenta —una por fichero—, y las referencias sin
    // declarar son el volumen.
    for s in skipped_files {
        let line = format!("  Skipped: {}: {}\n", s.path, s.reason);
        if written + line.len() > MAX_DETAIL_BYTES - RESERVED_FOR_SUMMARY {
            break;
        }
        w.write_all(line.as_bytes())?;
        written += line.len();
        skipped_listed += 1;
    }

    for reference in pending_references {
        // Una referencia sin linea no esta en la linea cero: esta en
        // una linea que el lector no conocia, y decir "0" nombraria un
        // sitio que no existe.
        let origin = match reference.line {
            Some(line) => format!("{}:{}", reference.file, line),
            None => reference.file.clone(),
        };
        let line = format!(
            "  Pending: {} -> {} ({:?})\n",
            origin, reference.target, reference.reason
        );
        if written + line.len() > MAX_DETAIL_BYTES - RESERVED_FOR_SUMMARY {
            break;
        }
        w.write_all(line.as_bytes())?;
        written += line.len();
        pending_listed += 1;
    }

    // Lo que quedó fuera, contado. El documento JSON lleva las listas
    // completas —`skipped_files`, `pending_references` y
    // `rejected_relations`—, así que el detalle no se pierde: cambia de
    // sitio.
    let skipped_omitted = skipped_files.len() - skipped_listed;
    if skipped_omitted > 0 {
        writeln!(
            w,
            "  Omitted: {skipped_omitted} of {} skipped files were not listed \
             (--format json has them all)",
            skipped_files.len()
        )?;
    }
    let pending_omitted = pending_references.len() - pending_listed;
    if pending_omitted > 0 {
        writeln!(
            w,
            "  Omitted: {pending_omitted} of {} pending references were not listed \
             (--format json has them all)",
            pending_references.len()
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::interface::coverage::PendingReason;

    /// A reference nobody declared: it says where it comes from and what
    /// it points at, like the ones the graph leaves loose.
    fn pending_reference(index: usize) -> PendingReferenceInfo {
        PendingReferenceInfo {
            file: format!("src/file{index}.rs"),
            line: Some(index as u32 + 1),
            target: format!("symbol{index}"),
            kind: "call".to_string(),
            reason: PendingReason::Unresolved,
            candidates: None,
        }
    }

    /// A file that could not be read.
    fn skipped_file(index: usize) -> SkippedFile {
        SkippedFile {
            path: format!("src/unreadable{index}.rs"),
            reason: "read failed".to_string(),
        }
    }

    /// How many of a kind the dump says it left out.
    ///
    /// `None` when the dump says nothing about that kind, which is what
    /// happens when it left none out.
    fn omitted_count(text: &str, kind: &str) -> Option<usize> {
        text.lines()
            .find(|line| line.trim_start().starts_with("Omitted: ") && line.contains(kind))
            .and_then(|line| line.trim_start().strip_prefix("Omitted: "))
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|number| number.parse().ok())
    }

    /// How many lines of a kind the dump wrote.
    fn listed_count(text: &str, prefix: &str) -> usize {
        text.lines()
            .filter(|line| line.trim_start().starts_with(prefix))
            .count()
    }

    /// The dump does not grow without a bound, and it loses nothing in
    /// silence.
    ///
    /// MEASURED on 2026-11-14 over this repository: `graph full` wrote
    /// 4 285 666 bytes of stderr, one line per reference nobody declared
    /// —44 642 of them—. A dump that is cut in silence lies: whoever
    /// reads it has to know that there is more than what they see.
    #[test]
    fn the_detail_dump_is_bounded_and_says_how_many_it_left_out() {
        let skipped: Vec<SkippedFile> = (0..3).map(skipped_file).collect();
        let pending: Vec<PendingReferenceInfo> = (0..5_000).map(pending_reference).collect();

        let mut out: Vec<u8> = Vec::new();
        report_graph_detail(&skipped, &pending, &mut out).expect("the dump is written");
        let text = String::from_utf8(out).expect("the dump is text");

        assert!(
            text.len() <= MAX_DETAIL_BYTES,
            "the dump wrote {} bytes, over the cap of {MAX_DETAIL_BYTES}; a dump without a \
             bound buries the error that does matter",
            text.len()
        );

        let pending_listed = listed_count(&text, "Pending: ");
        let pending_omitted = omitted_count(&text, "pending references").unwrap_or_else(|| {
            panic!("the dump does not say how many references it left out:\n{text}")
        });
        assert_eq!(
            pending_listed + pending_omitted,
            pending.len(),
            "the dump listed {pending_listed} references and counted {pending_omitted} left \
             out, and the build had {}: a reference was lost without anyone saying so",
            pending.len()
        );

        let skipped_listed = listed_count(&text, "Skipped: ");
        let skipped_omitted = omitted_count(&text, "skipped files").unwrap_or(0);
        assert_eq!(
            skipped_listed + skipped_omitted,
            skipped.len(),
            "the dump listed {skipped_listed} files and counted {skipped_omitted} left out, \
             and the build had {}",
            skipped.len()
        );
    }

    /// The files that could not be read come first: their line is the one
    /// a consumer counts —one per file— and the undeclared references are
    /// the volume.
    #[test]
    fn the_files_that_could_not_be_read_come_first() {
        let skipped: Vec<SkippedFile> = (0..3).map(skipped_file).collect();
        let pending: Vec<PendingReferenceInfo> = (0..5_000).map(pending_reference).collect();

        let mut out: Vec<u8> = Vec::new();
        report_graph_detail(&skipped, &pending, &mut out).expect("the dump is written");
        let text = String::from_utf8(out).expect("the dump is text");

        for index in 0..3 {
            assert!(
                text.contains(&format!("  Skipped: src/unreadable{index}.rs: read failed")),
                "the dump did not name file {index}, which could not be read:\n{text}"
            );
        }
        assert_eq!(
            listed_count(&text, "Skipped: "),
            3,
            "every file that could not be read has to be listed: its line is the one a \
             consumer counts"
        );
    }

    /// A detail that fits is written whole and without noise: the small
    /// case does not change, and nobody reads a summary of nothing having
    /// been left out.
    #[test]
    fn a_detail_that_fits_is_written_whole() {
        let skipped: Vec<SkippedFile> = (0..2).map(skipped_file).collect();
        let pending: Vec<PendingReferenceInfo> = (0..2).map(pending_reference).collect();

        let mut out: Vec<u8> = Vec::new();
        report_graph_detail(&skipped, &pending, &mut out).expect("the dump is written");
        let text = String::from_utf8(out).expect("the dump is text");

        assert_eq!(
            listed_count(&text, "Skipped: "),
            2,
            "the files that could not be read have to be listed:\n{text}"
        );
        assert_eq!(
            listed_count(&text, "Pending: "),
            2,
            "the undeclared references have to be listed:\n{text}"
        );
        assert!(
            !text.contains("Omitted:"),
            "nothing was left out and the dump says so:\n{text}"
        );
    }
}
