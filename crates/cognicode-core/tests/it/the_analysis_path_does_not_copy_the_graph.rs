//! El camino de análisis no copia el grafo.
//!
//! ## El defecto
//!
//! Las herramientas de análisis del grafo piden el grafo con
//! `load_graph()`, que devuelve una copia: el almacén que tiene el grafo
//! en memoria clona el grafo entero —el propio comentario decía
//! «5-10 MB for 29K symbols»— de forma síncrona, en el hilo del runtime
//! que sirve la llamada. El servidor MCP no atiende ninguna otra
//! llamada mientras copia, y el plazo de despacho no puede cortarlo: el
//! sondeo del futuro está dentro de la copia.
//!
//! ## Lo que mide este fichero
//!
//! Un almacén que sabe cuánto se copia: el acceso que devuelve el grafo
//! en propiedad cuenta una copia, y el acceso que devuelve el puntero
//! que el almacén ya tiene no cuenta ninguna. Se llaman las
//! herramientas de análisis y se pregunta cuántas copias se pagaron. La
//! respuesta es cero.
//!
//! El almacén es el doble —el borde—, el resto es de verdad: los
//! handlers, el grafo y los algoritmos que trabajan sobre él.

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::traits::graph_store::{GraphStore, StoreError};
use cognicode_core::domain::value_objects::file_manifest::FileManifest;
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::interface::mcp::handlers::consolidated_handlers::{
    CodebaseMapInput, MapFormat, handle_codebase_map,
};
use cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_pagerank;
use cognicode_core::interface::mcp::handlers::graph_query_handlers::{
    GraphQueryInput, handle_graph_query,
};
use cognicode_core::interface::mcp::handlers::{
    HandlerContext, HandlerContextBuilder, handle_solid_audit,
};
use cognicode_core::interface::mcp::schemas::GraphPageRankInput;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// El doble: un almacén que sabe cuánto se copia.
// ---------------------------------------------------------------------------

/// Un almacén que cuenta las copias del grafo.
///
/// El grafo vive detrás de un `Arc`, como vive en el almacén de
/// producción. El acceso en propiedad lo clona —eso es una copia— y lo
/// cuenta; el acceso compartido devuelve el mismo puntero y no cuenta
/// nada.
struct CountingStore {
    graph: Arc<CallGraph>,
    copies: Arc<AtomicUsize>,
}

impl GraphStore for CountingStore {
    fn save_graph(&self, _graph: &CallGraph) -> Result<(), StoreError> {
        Ok(())
    }

    /// El acceso en propiedad: el grafo entero, clonado.
    fn load_graph(&self) -> Result<Option<CallGraph>, StoreError> {
        self.copies.fetch_add(1, Ordering::SeqCst);
        Ok(Some((*self.graph).clone()))
    }

    /// El acceso compartido: el puntero que el almacén ya tiene.
    fn load_graph_shared(&self) -> Result<Option<Arc<CallGraph>>, StoreError> {
        Ok(Some(self.graph.clone()))
    }

    fn save_manifest(&self, _manifest: &FileManifest) -> Result<(), StoreError> {
        Ok(())
    }

    fn load_manifest(&self) -> Result<Option<FileManifest>, StoreError> {
        Ok(None)
    }

    fn clear(&self) -> Result<(), StoreError> {
        Ok(())
    }

    fn exists(&self) -> Result<bool, StoreError> {
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// El fixture.
// ---------------------------------------------------------------------------

/// Un grafo con dos símbolos y una llamada entre ellos.
fn graph() -> CallGraph {
    let mut graph = CallGraph::new();
    let alpha = graph.add_symbol(Symbol::new(
        "alpha",
        SymbolKind::Function,
        Location::new("lib.rs", 1, 0),
    ));
    let beta = graph.add_symbol(Symbol::new(
        "beta",
        SymbolKind::Function,
        Location::new("lib.rs", 2, 0),
    ));
    graph
        .add_dependency(&alpha, &beta, DependencyType::Calls)
        .expect("the dependency is between two symbols of the graph");
    graph
}

/// El contexto que sirve las llamadas, con el almacén que cuenta.
fn context() -> (HandlerContext, Arc<AtomicUsize>) {
    let copies = Arc::new(AtomicUsize::new(0));
    let store = CountingStore {
        graph: Arc::new(graph()),
        copies: copies.clone(),
    };
    let ctx = HandlerContextBuilder::new()
        .with_graph_store_arc(Arc::new(store))
        .build();
    (ctx, copies)
}

/// Ninguna copia se pagó para servir la llamada.
///
/// El contador se pone a cero antes de cada herramienta, así que el
/// fallo dice QUIÉN copió, no sólo que alguien copió.
fn assert_no_copy(copies: &AtomicUsize, tool: &str) {
    assert_eq!(
        copies.swap(0, Ordering::SeqCst),
        0,
        "the tool `{tool}` copied the whole graph to serve the call: the copy is paid by the \
         thread that serves the runtime, and the graph is already in memory behind a pointer"
    );
}

// ---------------------------------------------------------------------------
// La medición.
// ---------------------------------------------------------------------------

/// Las herramientas de análisis no clonan el grafo para servir la llamada.
///
/// Cada familia de herramientas pregunta por su cuenta: el análisis
/// (`graph_pagerank`), la consulta al grafo (`graph_query`), el mapa del
/// proyecto (`codebase_map`) y la auditoría SOLID (`solid_audit`).
#[tokio::test]
async fn the_analysis_tools_serve_the_graph_without_copying_it() {
    let (ctx, copies) = context();

    handle_graph_pagerank(
        &ctx,
        GraphPageRankInput {
            alpha: 0.85,
            max_iterations: 10,
        },
    )
    .await
    .expect("the pagerank tool answers");
    assert_no_copy(&copies, "graph_pagerank");

    handle_graph_query(
        &ctx,
        GraphQueryInput {
            question: "who calls alpha".to_string(),
            max_depth: 2,
            budget: 50,
            continuation_token: None,
        },
    )
    .await
    .expect("the graph query tool answers");
    assert_no_copy(&copies, "graph_query");

    handle_codebase_map(
        &ctx,
        CodebaseMapInput {
            format: MapFormat::Compact,
        },
    )
    .await
    .expect("the codebase map answers");
    assert_no_copy(&copies, "codebase_map");

    handle_solid_audit(&ctx)
        .await
        .expect("the solid audit answers");
    assert_no_copy(&copies, "solid_audit");
}
