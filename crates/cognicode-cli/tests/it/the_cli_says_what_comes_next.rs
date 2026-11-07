//! TASK-83 — el CLI dice lo que viene después, y lo dice como una orden.
//!
//! El cable y la terminal son dos transportes del mismo sobre: los
//! enlaces que el MCP publica en `structuredContent` son los mismos que
//! el CLI imprime como órdenes. Este fichero mide el caso en el que la
//! orden importa más: una construcción **parcial** del grafo, que es la
//! respuesta que más deja fuera y la que menos ayuda deja si no dice
//! cómo seguir.
//!
//! ## Lo que NO se comprueba aquí
//!
//! - Que el sobre viaje por el MCP: eso lo mide
//!   `a012_structured_output` (el mismo contrato, visto desde el cable).
//! - Que el grafo se construya de verdad: aquí la respuesta es un
//!   valor, no una construcción.

use cognicode_cli::cli::commands::Format as CliFormat;
use cognicode_cli::cli::result::{CommandOutput, GraphResult, SkippedFile};
use cognicode_cli::presenter::present_output;

/// La respuesta que el CLI imprime, en texto.
fn printed(output: &CommandOutput) -> String {
    let mut buffer = Vec::new();
    present_output(output, CliFormat::Text, &mut buffer).expect("the presenter writes");
    String::from_utf8(buffer).expect("the presenter writes utf-8")
}

/// Una construcción parcial: un fichero que no se pudo leer.
fn partial_build() -> CommandOutput {
    CommandOutput::Graph(GraphResult::Full {
        path: "/repo".to_string(),
        rebuild: false,
        elapsed_ms: 12,
        symbols: 100,
        dependencies: 20,
        status: "partial".to_string(),
        skipped_files: vec![SkippedFile {
            path: "src/a.rs".to_string(),
            reason: "io error".to_string(),
        }],
        pending_references: vec![],
        rejected_relations: vec![],
    })
}

/// Una construcción parcial dice cómo volver a construir.
///
/// El hueco —el fichero que no se pudo leer— es la razón del enlace, y
/// la orden es la que reconstruye el grafo **de ese árbol**, no la del
/// directorio desde el que se teclea.
#[test]
fn a_partial_build_prints_the_rebuild_of_the_tree_it_built() {
    let text = printed(&partial_build());

    assert!(
        text.contains("Siguiente:"),
        "the answer names what comes next: {text}"
    );
    assert!(
        text.contains("cognicode graph full --rebuild /repo"),
        "the next step rebuilds the graph of the tree that was built: {text}"
    );
    assert!(
        text.contains("1 file, 0 references and 0 relations were left out of the answer"),
        "the reason says how much was left out, in words a person reads: {text}"
    );
}

/// Una construcción completa no inventa un siguiente paso.
///
/// El enlace del reindexado existe cuando hay un hueco. Sin huecos,
/// imprimirlo sería pedirle al usuario una reconstrucción que no hace
/// falta.
#[test]
fn a_complete_build_prints_no_next_step() {
    let output = CommandOutput::Graph(GraphResult::Full {
        path: "/repo".to_string(),
        rebuild: false,
        elapsed_ms: 12,
        symbols: 100,
        dependencies: 20,
        status: "complete".to_string(),
        skipped_files: vec![],
        pending_references: vec![],
        rejected_relations: vec![],
    });

    let text = printed(&output);
    assert!(
        !text.contains("Siguiente:"),
        "a complete answer has nothing to offer: {text}"
    );
}
