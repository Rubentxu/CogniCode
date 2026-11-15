//! Una construcción parcial conserva su manifiesto: una respuesta con huecos
//! también se puede fechar.
//!
//! ## El defecto
//!
//! `load_or_build_graph` conservaba el manifiesto sólo cuando la construcción
//! era completa: `manifest_to_keep` era `Some(manifest)` únicamente si
//! `build_complete`. Una construcción parcial —una llamada que no se pudo
//! atribuir, un fichero que no se pudo leer— pierde el manifiesto, y con él
//! el digest: `graph_basis` devuelve `None` y el sobre sale con
//! `basis.freshness: null`.
//!
//! La respuesta que más necesita fecharse es la que tiene huecos: el
//! consumidor necesita saber si es de antes o de después del cambio, y una
//! respuesta que no se puede fechar no se puede comparar con otra.
//!
//! ## Lo que se mide
//!
//! El sobre, que es lo que ve el consumidor: la frescura de una respuesta
//! parcial y el estado que la acompaña.
//!
//! ## Lo que tiene que seguir siendo verdad
//!
//! Una construcción parcial no deja un snapshot durable detrás: un grafo
//! parcial servido más tarde por otra sesión publicaría la cobertura de un
//! grafo que no leyó el árbol entero. Y fechar la respuesta no la completa:
//! el estado sigue siendo `partial` y la cobertura sigue nombrando el hueco.

use cognicode_core::application::graph_cache;
use cognicode_core::application::graph_snapshot::load_or_build_graph;
use cognicode_core::application::services::analysis_service::AnalysisService;
use cognicode_core::interface::cursor::CursorContext;
use cognicode_core::interface::goals::overview;
use cognicode_core::interface::hateoas::envelope_for;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// Escribe un fichero, creando su directorio.
fn write(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().expect("a file has a parent")).expect("directory");
    std::fs::write(path, contents).expect("the file is written");
}

/// Un árbol que se lee entero: una función y la que la llama.
fn readable_tree() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    write(
        &root.join("src/lib.rs"),
        "pub fn helper() -> u32 {\n    1\n}\n\npub fn caller() -> u32 {\n    helper()\n}\n",
    );
    (dir, root)
}

/// Un árbol con una llamada que el proyecto no puede atribuir.
///
/// El nombre está declarado dos veces, y el grafo no puede probar cuál de las
/// dos declaraciones es la que la llamada nombra: la relación se queda suelta,
/// y un grafo al que le falta una relación no describe el árbol entero.
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

/// Una respuesta con huecos también se puede fechar.
///
/// El digest del manifiesto describe los ficheros leídos, no la completitud
/// del grafo: un manifiesto que nombra una llamada suelta sigue siendo el
/// manifiesto de un árbol, y su digest sigue diciendo qué árbol es.
#[test]
fn a_partial_answer_can_be_dated() {
    let (_tmp, root) = tree_with_an_ambiguous_call();

    let answer = overview::overview(&root).expect("the goal answers");
    let answer = serde_json::to_value(answer).expect("the answer is JSON");
    let envelope = envelope_of("overview", &answer, &root);

    assert!(
        envelope.basis.freshness.is_some(),
        "una respuesta con huecos también se puede fechar: {envelope:?}"
    );
}

/// Fechar la respuesta no la completa.
///
/// El signo contrario: una respuesta fechada como `complete` sobre un árbol
/// con un hueco es una cobertura que hay que poder leer como una medida.
#[test]
fn a_dated_partial_answer_is_still_partial() {
    let (_tmp, root) = tree_with_an_ambiguous_call();

    let answer = overview::overview(&root).expect("the goal answers");
    let answer = serde_json::to_value(answer).expect("the answer is JSON");
    let envelope = envelope_of("overview", &answer, &root);

    assert_eq!(
        envelope.state, "partial",
        "fechar la respuesta no la completa: {envelope:?}"
    );
    let coverage = envelope
        .basis
        .coverage
        .clone()
        .expect("la cobertura de un árbol con un hueco existe");
    assert!(
        coverage.pending_references > 0,
        "la cobertura sigue nombrando la referencia que quedó suelta: {envelope:?}"
    );
}

/// Una construcción parcial no deja un snapshot detrás.
///
/// El snapshot durable es lo que otra sesión sirve como el grafo del árbol, y
/// un grafo parcial servido por otra sesión publicaría la cobertura de un
/// grafo que no leyó el árbol entero: el snapshot es la evidencia de una
/// lectura completa, y una lectura que no fue completa no deja esa evidencia.
#[test]
fn a_partial_build_leaves_no_snapshot_behind() {
    let (tmp, root) = tree_with_an_ambiguous_call();
    let cache = tmp.path().join("cache");
    let service = AnalysisService::new();

    load_or_build_graph(&service, &root, Some(&cache), None, false).expect("the graph is built");

    let snapshot = graph_cache::snapshot_path(&cache, &root);
    assert!(
        !snapshot.exists(),
        "una construcción parcial no deja un snapshot que otra sesión sirva como completo: {}",
        snapshot.display()
    );
}

/// El control: una construcción completa sí deja un snapshot.
///
/// Sin este control, la comprobación anterior podría pasar porque el
/// snapshot no se escribe nunca.
#[test]
fn a_complete_build_leaves_a_snapshot_behind() {
    let (tmp, root) = readable_tree();
    let cache = tmp.path().join("cache");
    let service = AnalysisService::new();

    load_or_build_graph(&service, &root, Some(&cache), None, false).expect("the graph is built");

    let snapshot = graph_cache::snapshot_path(&cache, &root);
    assert!(
        snapshot.exists(),
        "una construcción completa deja el snapshot que otra sesión sirve: {}",
        snapshot.display()
    );
}

/// El digest de una respuesta parcial sigue a sus fuentes.
///
/// La invalidación por contenido no necesita el grafo completo: compara los
/// ficheros del disco con el manifiesto, y el manifiesto de una construcción
/// parcial nombra los ficheros que se leyeron. Un fichero editado cambia el
/// digest, y una respuesta con huecos se puede comparar con otra.
#[test]
fn the_digest_of_a_partial_answer_follows_its_sources() {
    let (_tmp, root) = tree_with_an_ambiguous_call();

    let before = digest_of(&root);
    write(
        &root.join("src/one.rs"),
        "pub fn helper() -> u32 {\n    3\n}\n",
    );
    let after = digest_of(&root);

    assert!(
        before.is_some(),
        "una respuesta con huecos se puede fechar: {before:?}"
    );
    assert_ne!(
        before, after,
        "un fichero editado cambia el digest: {before:?} y {after:?}"
    );
}

/// El digest que la respuesta de `overview` publica.
fn digest_of(root: &Path) -> Option<String> {
    let answer = overview::overview(root).expect("the goal answers");
    let answer = serde_json::to_value(answer).expect("the answer is JSON");
    let envelope = envelope_of("overview", &answer, root);
    envelope.basis.freshness.map(|freshness| freshness.snapshot)
}
