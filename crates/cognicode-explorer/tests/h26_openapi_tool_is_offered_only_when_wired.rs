#![cfg(feature = "multimodal")]
//! H26 — Una capacidad que no está conectada no se ofrece.
//!
//! Las dos herramientas de ingesta de OpenAPI (`ingest_openapi` y
//! `trace_route`) se registraban **sin condición**: el registro miraba la
//! feature, no el cableado. Con la feature puesta y sin `RouteStore` detrás,
//! el servidor anunciaba dos herramientas que no podían hacer nada: la única
//! respuesta posible era «feature_disabled».
//!
//! Un cliente MCP ve la lista de herramientas antes de llamarlas, y una
//! herramienta que no puede funcionar no pertenece a la lista. El catálogo
//! publicado (`product/tools.json`) no declara ninguna de las dos, así que la
//! superficie del runtime y el catálogo tienen que decir lo mismo.
//!
//! Este fichero se escribió **contra la API anterior al arreglo**: la
//! comprobación que discrimina fallaba, y la guarda —con almacén cableado—
//! pasaba.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use async_trait::async_trait;
use cognicode_core::domain::aggregates::CallGraph;
use cognicode_explorer::adapters::{CallGraphRepository, FsSourceReader};
use cognicode_explorer::error::{ExplorerError, ExplorerResult};
use cognicode_explorer::mcp::explorer::{ExplorerMcpHandler, McpHandlerPorts};
use cognicode_explorer::ports::{
    ApiRoute, ApiRouteEdge, BatchStats, EDGE_KIND_HTTP_CALLS, PROTOCOL_HTTP, RouteStore,
};
use serde_json::json;

const TOOL_INGEST_OPENAPI: &str = "ingest_openapi";
const TOOL_TRACE_ROUTE: &str = "trace_route";

/// A store that answers everything, used to check that the capability is
/// offered when it can actually work.
struct StubStore;

#[async_trait]
impl RouteStore for StubStore {
    async fn upsert_route(&self, _route: &ApiRoute) -> ExplorerResult<bool> {
        Ok(true)
    }

    async fn save_edge(&self, _edge: &ApiRouteEdge) -> ExplorerResult<bool> {
        Ok(true)
    }

    async fn save_many(
        &self,
        routes: &[ApiRoute],
        edges: &[ApiRouteEdge],
    ) -> ExplorerResult<BatchStats> {
        let mut stats = BatchStats::default();
        stats.routes_created = routes.len();
        stats.edges_created = edges.len();
        Ok(stats)
    }

    async fn find_route_by_method_path(
        &self,
        _method: &str,
        _path: &str,
    ) -> ExplorerResult<Option<ApiRoute>> {
        Err(ExplorerError::NotFound("no route".to_string()))
    }

    async fn find_routes_by_spec_hash(&self, _spec_hash: &str) -> ExplorerResult<Vec<ApiRoute>> {
        Ok(Vec::new())
    }

    async fn find_routes_by_handler(&self, _handler_symbol: &str) -> ExplorerResult<Vec<ApiRoute>> {
        Ok(Vec::new())
    }
}

/// The handler ports, with the route store wired or not.
fn ports(route_store: Option<Arc<dyn RouteStore>>) -> McpHandlerPorts {
    let graph = Arc::new(CallGraph::default());
    McpHandlerPorts {
        symbol_repo: Arc::new(CallGraphRepository::new(graph.clone())),
        source_reader: Arc::new(FsSourceReader::new(std::env::temp_dir())),
        view_registry: Arc::new(cognicode_explorer::registry::ViewRegistry::new(None)),
        lens_registry: cognicode_explorer::domain::lens::default_registry(),
        cwd: std::env::temp_dir(),
        graph: Some(graph),
        quality_store: None,
        quality_write: None,
        revision_tracker: Arc::new(AtomicU64::new(0)),
        route_store,
        analytics_registry: None,
        analytics_lineage_store: None,
    }
}

/// The names of the tools the handler offers — the list a client MCP sees.
fn tool_names(handler: &ExplorerMcpHandler) -> Vec<String> {
    handler
        .tools_list()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect()
}

/// Without a store behind it, the capability cannot work, so it is not
/// offered.
///
/// Before the fix, both tools were in the list and every call answered
/// «feature_disabled».
#[test]
fn the_openapi_tools_are_not_offered_without_a_store() {
    let handler = ExplorerMcpHandler::with_graph(ports(None));
    let names = tool_names(&handler);

    assert!(
        !names.iter().any(|n| n == TOOL_INGEST_OPENAPI),
        "a tool that cannot work must not be offered, but the list has it: {names:?}"
    );
    assert!(
        !names.iter().any(|n| n == TOOL_TRACE_ROUTE),
        "a tool that cannot work must not be offered, but the list has it: {names:?}"
    );
}

/// The guard: with a store behind it, the capability is offered. The fix must
/// not "solve" the problem by hiding a capability that works.
#[test]
fn the_openapi_tools_are_offered_when_a_store_is_wired() {
    let handler = ExplorerMcpHandler::with_graph(ports(Some(Arc::new(StubStore))));
    let names = tool_names(&handler);

    assert!(
        names.iter().any(|n| n == TOOL_INGEST_OPENAPI),
        "a working capability must be offered, but the list does not have it: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == TOOL_TRACE_ROUTE),
        "a working capability must be offered, but the list does not have it: {names:?}"
    );
}

/// The rest of the multimodal surface keeps working: the fix only touches the
/// capability that depends on the route store.
#[test]
fn the_ingest_tools_that_do_not_need_a_store_keep_being_offered() {
    let handler = ExplorerMcpHandler::with_graph(ports(None));
    let names = tool_names(&handler);

    assert!(
        names.iter().any(|n| n == "docs_ingest"),
        "the docs ingestion does not depend on the route store: {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "graph_search"),
        "the graph search does not depend on the route store: {names:?}"
    );
}

/// The port keeps its vocabulary: the edge kinds and protocols the store
/// writes are the ones the catalogue names.
#[tokio::test]
async fn the_stub_store_writes_the_kinds_the_port_declares() {
    let store = StubStore;
    let route = ApiRoute {
        id: "route:http:GET:/a".to_string(),
        protocol: PROTOCOL_HTTP.to_string(),
        method: "GET".to_string(),
        path: "/a".to_string(),
        handler_symbol: None,
        spec_source: "/tmp/openapi.json".to_string(),
        spec_hash: "spec-hash".to_string(),
        framework: None,
        confidence: 1.0,
        properties: json!({}),
    };
    let edge = ApiRouteEdge {
        source_route_id: route.id.clone(),
        target_symbol_id: "symbol:handler".to_string(),
        edge_kind: EDGE_KIND_HTTP_CALLS.to_string(),
        confidence: 1.0,
        metadata: json!({}),
    };

    let stats = store.save_many(&[route], &[edge]).await.expect("batch");
    assert_eq!(stats.routes_created, 1);
    assert_eq!(stats.edges_created, 1);
}
