//! El snapshot se lleva lo que el lector leyó.
//!
//! ## El hueco
//!
//! El snapshot durable se escribe con bincode, que no se autodescribe: los
//! bytes de un símbolo son los de sus campos, en orden, y el que los lee
//! espera los mismos campos. Un símbolo que declara un test lleva ese dato
//! —el lector es el único que puede saberlo—, así que un campo que se
//! escribe sólo cuando es verdad es un campo que el lector del snapshot
//! espera y no encuentra.
//!
//! Lo que se rompe no es un detalle: el snapshot deja de decodificar, el
//! producto reconstruye el grafo en cada run, y el distintivo de test se
//! pierde por el camino. El consumidor que pregunta qué tests alcanzan un
//! cambio recibe una lista vacía de un grafo que sí los sabía.
//!
//! ## Lo que se mide
//!
//! Un símbolo marcado como test viaja al snapshot y vuelve diciendo lo que
//! decía, y un símbolo que no lo es vuelve sin serlo. El viaje de ida y
//! vuelta es el del bincode, el mismo que usa el producto.

use cognicode_core::application::graph_snapshot::{load_durable_snapshot, save_durable_snapshot};
use cognicode_core::domain::aggregates::Symbol;
use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::value_objects::{
    AnalyzerBuild, FileManifest, Location, SNAPSHOT_SCHEMA, SnapshotKey, SymbolKind,
};

/// El símbolo que el lector marcó como test, y el que no.
fn graph() -> (
    CallGraph,
    cognicode_core::domain::aggregates::call_graph::SymbolId,
    cognicode_core::domain::aggregates::call_graph::SymbolId,
) {
    let mut graph = CallGraph::new();
    let test = graph.add_symbol(
        Symbol::new(
            "the_test_that_watches_the_change",
            SymbolKind::Function,
            Location::new("src/a.rs", 10, 0),
        )
        .mark_as_test(),
    );
    let helper = graph.add_symbol(Symbol::new(
        "helper",
        SymbolKind::Function,
        Location::new("src/a.rs", 1, 0),
    ));
    (graph, test, helper)
}

/// La clave que nombra este análisis: el workspace, el analizador de ahora
/// y el esquema con el que se escribe.
fn key(workspace: &str) -> SnapshotKey {
    SnapshotKey::new(workspace, AnalyzerBuild::current(), SNAPSHOT_SCHEMA)
}

#[test]
fn the_snapshot_carries_the_distinctive_the_reader_gave() {
    let root = tempfile::tempdir().expect("a temporal root");
    let path = root.path().join("snapshot");
    let (graph, test, _helper) = graph();

    save_durable_snapshot(
        &path,
        &key("/tmp/ws"),
        &graph,
        &FileManifest::new(root.path().to_path_buf()),
    )
    .expect("the snapshot is written");

    let (_key, loaded, _manifest) = load_durable_snapshot(&path)
        .expect("the snapshot decodes: the shape it was written with is the shape it is read with");

    let loaded_test = loaded
        .get_symbol(&test)
        .expect("the test is in the graph that came back");
    assert!(
        loaded_test.is_test(),
        "the snapshot carries what the reader read: a symbol that declares a \
         test comes back declaring it. Symbol: {loaded_test:?}"
    );
}

#[test]
fn a_symbol_that_is_not_a_test_comes_back_not_being_one() {
    let root = tempfile::tempdir().expect("a temporary root");
    let path = root.path().join("snapshot");
    let (graph, _test, helper) = graph();

    save_durable_snapshot(
        &path,
        &key("/tmp/ws"),
        &graph,
        &FileManifest::new(root.path().to_path_buf()),
    )
    .expect("the snapshot is written");

    let (_key, loaded, _manifest) = load_durable_snapshot(&path)
        .expect("the snapshot decodes: the shape it was written with is the shape it is read with");

    let loaded_helper = loaded
        .get_symbol(&helper)
        .expect("the helper is in the graph that came back");
    assert!(
        !loaded_helper.is_test(),
        "a helper is not a test, and the snapshot does not invent one: \
         {loaded_helper:?}"
    );
}
