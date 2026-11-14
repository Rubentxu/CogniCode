//! Un goal que ha leído el árbol dice qué cubrió y de cuándo es la respuesta.
//!
//! ## El defecto
//!
//! `impact` y `trace` cargan el grafo del workspace y descartan lo que la
//! carga les dice: la cobertura que se midió y el manifiesto que la carga
//! conservó. El sobre sale con `state: unknown` y con `basis.coverage` y
//! `basis.freshness` nulos, aunque el árbol se haya leído entero. Una
//! respuesta que no dice qué cubre se lee como una respuesta que no cubre
//! nada, y una respuesta que no se puede fechar no se puede comparar con
//! otra.
//!
//! `overview` publicaba la cobertura de la lectura pero no la frescura: su
//! respuesta no se podía fechar.
//!
//! ## Lo que se mide
//!
//! El sobre, que es lo que ve el consumidor: su `state`, su `basis.coverage`
//! y su `basis.freshness`.
//!
//! ## Lo que tiene que seguir siendo verdad
//!
//! Un árbol con un hueco se publica como `partial` y nombra el fichero: una
//! cobertura que no dice lo que dejó fuera es una cobertura que hay que
//! poder leer como una medida.

#![cfg(unix)]

use cognicode_core::interface::cursor::CursorContext;
use cognicode_core::interface::goals::{context, impact, overview, status, trace};
use cognicode_core::interface::hateoas::envelope_for;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// El corpus: un símbolo y el que lo llama.
const LIB_RS: &str = "\
/// The symbol whose impact and trace are asked for.
pub fn helper() -> u32 {
    1
}

/// The symbol that calls it.
pub fn caller() -> u32 {
    helper()
}
";

/// Escribe un fichero, creando su directorio.
fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("directory");
    std::fs::write(path, contents).expect("the file is written");
}

/// Un árbol que se lee entero.
fn readable_tree() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(&root.join("src/lib.rs"), LIB_RS);
    (dir, root)
}

/// Un árbol con un hueco: un enlace que sale del workspace.
fn tree_with_a_hole() -> (tempfile::TempDir, PathBuf) {
    let (dir, root) = readable_tree();
    let outside = tempfile::tempdir().expect("tempdir");
    let target = outside.path().join("elsewhere.rs");
    write(&target, "pub fn elsewhere() {}\n");
    std::os::unix::fs::symlink(&target, root.join("src/linked.rs")).expect("symlink");
    (dir, root)
}

/// Un árbol con una llamada que el proyecto no puede atribuir.
///
/// El nombre está declarado dos veces, y el grafo no puede probar cuál de
/// las dos declaraciones es la que la llamada nombra: la relación se queda
/// suelta, y un grafo al que le falta una relación no describe el árbol
/// entero.
fn tree_with_an_ambiguous_call() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(
        &root.join("src/one.rs"),
        "pub fn helper() -> u32 {\n    1\n}\n",
    );
    write(
        &root.join("src/two.rs"),
        "pub fn helper() -> u32 {\n    2\n}\n",
    );
    write(
        &root.join("src/lib.rs"),
        "pub fn caller() -> u32 {\n    helper()\n}\n",
    );
    (dir, root)
}

/// El sobre de una respuesta, como lo ve el consumidor.
fn envelope_of(
    tool: &str,
    answer: &Value,
    workspace: &Path,
) -> cognicode_core::interface::hateoas::Envelope {
    let ctx = CursorContext {
        workspace: workspace.to_string_lossy().into_owned(),
        snapshot: String::new(),
    };
    envelope_for(tool, &Map::new(), answer, &ctx)
}

/// `impact` publica la cobertura y la frescura de lo que leyó.
#[test]
fn impact_publishes_what_it_covered() {
    let (_tmp, root) = readable_tree();

    let answer = impact::impact(&root, "helper").expect("the goal answers");
    let answer = serde_json::to_value(answer).expect("the answer is JSON");
    let envelope = envelope_of("impact", &answer, &root);

    assert_eq!(
        envelope.state, "complete",
        "un árbol que se lee entero se publica como completo, no como `unknown`: {envelope:?}"
    );
    assert!(
        envelope.basis.coverage.is_some(),
        "la cobertura viaja con la respuesta: {envelope:?}"
    );
    assert!(
        envelope.basis.freshness.is_some(),
        "la respuesta se puede fechar: {envelope:?}"
    );
}

/// `trace` publica la cobertura y la frescura de lo que leyó.
#[test]
fn trace_publishes_what_it_covered() {
    let (_tmp, root) = readable_tree();

    let answer = trace::trace(&root, "helper", "both", 1).expect("the goal answers");
    let answer = serde_json::to_value(answer).expect("the answer is JSON");
    let envelope = envelope_of("trace", &answer, &root);

    assert_eq!(
        envelope.state, "complete",
        "un árbol que se lee entero se publica como completo, no como `unknown`: {envelope:?}"
    );
    assert!(
        envelope.basis.coverage.is_some(),
        "la cobertura viaja con la respuesta: {envelope:?}"
    );
    assert!(
        envelope.basis.freshness.is_some(),
        "la respuesta se puede fechar: {envelope:?}"
    );
}

/// `overview` publica la frescura de lo que leyó.
#[test]
fn overview_publishes_when_its_answer_was_computed() {
    let (_tmp, root) = readable_tree();

    let answer = overview::overview(&root).expect("the goal answers");
    let answer = serde_json::to_value(answer).expect("the answer is JSON");
    let envelope = envelope_of("overview", &answer, &root);

    assert!(
        envelope.basis.freshness.is_some(),
        "la respuesta se puede fechar: {envelope:?}"
    );
}

/// `overview` publica la cobertura del grafo que cuenta.
///
/// Los números de la respuesta son los del grafo: si la cobertura que los
/// acompaña venga de otra lectura, la respuesta hablaría de dos árboles a
/// la vez, y el consumidor no podría saber cuál de los dos es el que cuenta.
#[test]
fn overview_publishes_the_coverage_of_the_graph_it_counts() {
    let (_tmp, root) = tree_with_an_ambiguous_call();

    let answer = overview::overview(&root).expect("the goal answers");
    let answer = serde_json::to_value(answer).expect("the answer is JSON");

    let coverage = &answer["coverage"];
    assert_eq!(
        coverage["status"], "partial",
        "una llamada que no se pudo atribuir deja el grafo incompleto: {coverage:?}"
    );
    assert!(
        !coverage["pending_references"]
            .as_array()
            .expect("the pending references are a list")
            .is_empty(),
        "la cobertura nombra la referencia que quedó suelta: {coverage:?}"
    );
}

/// Un goal que no carga el grafo no publica una frescura que nadie midió.
///
/// La frescura es el digest del manifiesto que conserva la carga: un goal
/// que no carga nada no tiene manifiesto que digerir, así que su sobre va
/// sin digest. Un digest de mentira sería una identidad que nadie midió, y
/// el `null` del sobre lo dice sin mentir.
#[test]
fn a_goal_that_does_not_load_the_graph_publishes_no_freshness() {
    let (_tmp, root) = readable_tree();

    let answer = status::status(
        &status::StatusInput {
            include_goals: false,
        },
        &root,
    );
    let answer = serde_json::to_value(answer).expect("the answer is JSON");
    let envelope = envelope_of("status", &answer, &root);
    assert!(
        envelope.basis.freshness.is_none(),
        "un goal que no carga el grafo no publica digest: {envelope:?}"
    );

    let answer = context::context(
        &context::ContextInput {
            include_coverage: true,
        },
        &root,
    );
    let answer = serde_json::to_value(answer).expect("the answer is JSON");
    let envelope = envelope_of("context", &answer, &root);
    assert!(
        envelope.basis.freshness.is_none(),
        "un goal que no carga el grafo no publica digest: {envelope:?}"
    );
}

/// Un árbol con un hueco se publica como `partial`, y nombra el fichero.
///
/// El signo contrario: una cobertura que no dice lo que dejó fuera no es una
/// medida.
#[test]
fn a_tree_with_a_hole_is_published_as_partial() {
    let (_tmp, root) = tree_with_a_hole();

    let answer = impact::impact(&root, "helper").expect("the goal answers");
    let answer = serde_json::to_value(answer).expect("the answer is JSON");
    let envelope = envelope_of("impact", &answer, &root);

    assert_eq!(
        envelope.state, "partial",
        "un árbol con un hueco no se publica como completo: {envelope:?}"
    );
    let coverage = envelope
        .basis
        .coverage
        .clone()
        .expect("la cobertura de un árbol con un hueco existe");
    assert_eq!(
        coverage.skipped_files, 1,
        "el hueco se cuenta: {envelope:?}"
    );
    let named = envelope
        .result
        .get("coverage")
        .and_then(|coverage| coverage.get("skipped_files"))
        .and_then(Value::as_array)
        .expect("la cobertura nombra los ficheros que dejó fuera");
    assert!(
        named[0]
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| path.ends_with("src/linked.rs")),
        "el hueco nombra el fichero: {envelope:?}"
    );
}
