//! H26 — Un lote se escribe entero o no se escribe.
//!
//! `RouteStore::save_many` promete en su documentación un lote «all-or-nothing»:
//! *«if any row fails, the whole batch is rolled back»*. Su implementación por
//! defecto escribe fila a fila, sin transacción, y lo confiesa en la línea
//! siguiente: *«atomicity is the caller's responsibility»*.
//!
//! El defecto medido no es una frase: es lo que pasa cuando alguien hereda ese
//! método. Un adaptador que no lo sobrescriba escribe el lote a medias en
//! cuanto una fila falla, y las filas anteriores quedan escritas. El informe
//! externo lo formula bien: *«el riesgo está en que nuevos adaptadores
//! hereden un contrato falso»*.
//!
//! Este fichero se escribió **contra la API anterior al arreglo**, y sus
//! comprobaciones fallaban. La que discrimina es la primera: después del
//! arreglo, el lote heredado **falla cerrado** —no escribe nada y explica por
//! qué— en vez de dejar el almacén a medias.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use cognicode_explorer::error::{ExplorerError, ExplorerResult};
use cognicode_explorer::ports::{
    ApiRoute, ApiRouteEdge, BatchStats, EDGE_KIND_HTTP_CALLS, PROTOCOL_HTTP, RouteStore,
};
use serde_json::json;

/// A route row with the given canonical id.
fn route(id: &str) -> ApiRoute {
    ApiRoute {
        id: id.to_string(),
        protocol: PROTOCOL_HTTP.to_string(),
        method: "GET".to_string(),
        path: format!("/{id}"),
        handler_symbol: None,
        spec_source: "/tmp/openapi.json".to_string(),
        spec_hash: "spec-hash".to_string(),
        framework: None,
        confidence: 1.0,
        properties: json!({}),
    }
}

/// An edge row pointing at the given route.
fn edge(source_route_id: &str) -> ApiRouteEdge {
    ApiRouteEdge {
        source_route_id: source_route_id.to_string(),
        target_symbol_id: "symbol:handler".to_string(),
        edge_kind: EDGE_KIND_HTTP_CALLS.to_string(),
        confidence: 1.0,
        metadata: json!({}),
    }
}

/// A store that inherits the port's default `save_many` — the shape a new
/// adapter has before anyone writes the transactional version.
///
/// It writes every route it is given, one at a time, and can be told to fail
/// on the N-th write.
struct InheritingStore {
    written: Mutex<Vec<String>>,
    calls: AtomicUsize,
    fail_on: Option<usize>,
}

impl InheritingStore {
    fn new() -> Self {
        Self {
            written: Mutex::new(Vec::new()),
            calls: AtomicUsize::new(0),
            fail_on: None,
        }
    }

    /// Fail on the N-th write (1-based), leaving the previous ones written.
    fn failing_on(call: usize) -> Self {
        Self {
            fail_on: Some(call),
            ..Self::new()
        }
    }

    fn written(&self) -> Vec<String> {
        self.written.lock().expect("store mutex").clone()
    }
}

#[async_trait]
impl RouteStore for InheritingStore {
    async fn upsert_route(&self, route: &ApiRoute) -> ExplorerResult<bool> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if self.fail_on == Some(n) {
            return Err(ExplorerError::Conflict(format!(
                "write failed for {}",
                route.id
            )));
        }
        self.written
            .lock()
            .expect("store mutex")
            .push(route.id.clone());
        Ok(true)
    }

    async fn save_edge(&self, _edge: &ApiRouteEdge) -> ExplorerResult<bool> {
        Ok(true)
    }

    async fn find_route_by_method_path(
        &self,
        _method: &str,
        _path: &str,
    ) -> ExplorerResult<Option<ApiRoute>> {
        Ok(None)
    }

    async fn find_routes_by_spec_hash(&self, _spec_hash: &str) -> ExplorerResult<Vec<ApiRoute>> {
        Ok(Vec::new())
    }

    async fn find_routes_by_handler(&self, _handler_symbol: &str) -> ExplorerResult<Vec<ApiRoute>> {
        Ok(Vec::new())
    }
}

/// A store that provides its own all-or-nothing batch: it stages the rows and
/// commits them only if every one of them can be written.
struct TransactionalStore {
    written: Mutex<Vec<String>>,
    fail_on: Option<usize>,
}

impl TransactionalStore {
    fn new() -> Self {
        Self {
            written: Mutex::new(Vec::new()),
            fail_on: None,
        }
    }

    fn failing_on(call: usize) -> Self {
        Self {
            fail_on: Some(call),
            ..Self::new()
        }
    }

    fn written(&self) -> Vec<String> {
        self.written.lock().expect("store mutex").clone()
    }
}

#[async_trait]
impl RouteStore for TransactionalStore {
    async fn upsert_route(&self, route: &ApiRoute) -> ExplorerResult<bool> {
        self.written
            .lock()
            .expect("store mutex")
            .push(route.id.clone());
        Ok(true)
    }

    async fn save_edge(&self, _edge: &ApiRouteEdge) -> ExplorerResult<bool> {
        Ok(true)
    }

    /// The whole batch or nothing: the rows are staged first and committed
    /// only when every one of them could be written.
    async fn save_many(
        &self,
        routes: &[ApiRoute],
        edges: &[ApiRouteEdge],
    ) -> ExplorerResult<BatchStats> {
        let mut staged: Vec<String> = Vec::with_capacity(routes.len());
        let mut stats = BatchStats::default();
        for (i, route) in routes.iter().enumerate() {
            if self.fail_on == Some(i + 1) {
                return Err(ExplorerError::Conflict(format!(
                    "write failed for {}",
                    route.id
                )));
            }
            staged.push(route.id.clone());
            stats.routes_created += 1;
        }
        stats.edges_created = edges.len();
        self.written.lock().expect("store mutex").extend(staged);
        Ok(stats)
    }

    async fn find_route_by_method_path(
        &self,
        _method: &str,
        _path: &str,
    ) -> ExplorerResult<Option<ApiRoute>> {
        Ok(None)
    }

    async fn find_routes_by_spec_hash(&self, _spec_hash: &str) -> ExplorerResult<Vec<ApiRoute>> {
        Ok(Vec::new())
    }

    async fn find_routes_by_handler(&self, _handler_symbol: &str) -> ExplorerResult<Vec<ApiRoute>> {
        Ok(Vec::new())
    }
}

/// A batch that inherits the port's default must not leave rows behind when
/// one of them fails.
///
/// Before the fix, the default wrote the first route and then failed on the
/// second: the store kept the first row. The contract says the batch is
/// rolled back.
#[tokio::test]
async fn a_batch_that_fails_in_the_middle_leaves_nothing_written() {
    let store = InheritingStore::failing_on(2);
    let result = store.save_many(&[route("a"), route("b")], &[]).await;

    assert!(
        result.is_err(),
        "the batch must fail when one of its rows fails"
    );
    assert!(
        store.written().is_empty(),
        "a failed batch must not leave rows behind, but the store kept: {:?}",
        store.written()
    );
}

/// The refusal must say what is missing. An adapter that does not implement
/// the transactional batch has to learn what it has to implement, not that
/// something went wrong.
#[tokio::test]
async fn the_refusal_names_the_obligation() {
    let store = InheritingStore::failing_on(2);
    let result = store.save_many(&[route("a"), route("b")], &[]).await;

    let message = match result {
        Ok(_) => panic!("the inherited batch must not succeed"),
        Err(e) => e.to_string(),
    };
    assert!(
        message.contains("transactional"),
        "the refusal must name the obligation it cannot meet, but it said: {message}"
    );
}

/// Without a transactional batch, nothing is written at all — not even a batch
/// that would have succeeded.
///
/// This is the difference between a port that refuses and a port that lies.
#[tokio::test]
async fn an_inherited_batch_is_never_written_at_all() {
    let store = InheritingStore::new();
    let result = store.save_many(&[route("a")], &[edge("a")]).await;

    assert!(
        result.is_err(),
        "a store without a transactional batch must not claim to have written one"
    );
    assert!(
        store.written().is_empty(),
        "nothing may be written by a batch the store cannot make atomic, but the store kept: {:?}",
        store.written()
    );
}

/// The guard: a store that *does* write the batch atomically is not punished
/// by the new contract. The port stays usable, and the fix does not "solve"
/// the problem by disabling the capability.
#[tokio::test]
async fn a_store_that_writes_the_batch_transactionally_is_not_punished() {
    let failing = TransactionalStore::failing_on(2);
    let result = failing.save_many(&[route("a"), route("b")], &[]).await;
    assert!(result.is_err(), "the batch must fail when a row fails");
    assert!(
        failing.written().is_empty(),
        "a transactional batch leaves nothing behind, but the store kept: {:?}",
        failing.written()
    );

    let store = TransactionalStore::new();
    let stats = store
        .save_many(&[route("a"), route("b")], &[edge("a")])
        .await
        .expect("a transactional batch must succeed");
    assert_eq!(stats.routes_created, 2);
    assert_eq!(stats.edges_created, 1);
    assert_eq!(store.written().len(), 2, "both routes must be written");
}
