//! Domain port for emitting cross-service protocol route edges.
//!
//! Cycle e15.5 introduces spec-based ingestion of HTTP / GraphQL /
//! gRPC / tRPC routes. Each ingestion handler parses a spec and emits
//! `Route` nodes plus `http_calls` / `graphql_calls` / `grpc_calls` /
//! `trpc_calls` edges via this port. The schema lives in PostgreSQL
//! (`m0012_route_nodes_protocol_edges.sql`); this port abstracts the
//! persistence layer so handlers stay pure and the adapter can be
//! swapped (e.g. for tests or in-memory mode).
//!
//! ## Idempotency
//!
//! Both `upsert_route` and `emit_edge` use ON CONFLICT DO UPDATE
//! semantics keyed by the canonical id (`route:{protocol}:{method}:{path}`)
//! and the composite PK `(source_route_id, target_symbol_id, edge_kind)`.
//! Re-ingesting the same spec is safe and cheap — the operation is
//! idempotent at the DB level.
//!
//! ## Spec hash as the dedup key
//!
//! The `spec_hash` column (SHA256 of the spec bytes) lets the handler
//! short-circuit a re-ingest: if a row with the same `spec_hash`
//! already exists, return the existing routes without re-parsing.
//! See `cognicode_ingest_openapi` for the usage pattern.
//!
//! ## Edge kinds
//!
//! Edge kinds are constrained at the SQL layer by
//! `chk_api_route_edges_kind`. Adding a new protocol edge kind
//! requires both this port and the migration's CHECK constraint to
//! be updated together.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::ExplorerResult;

/// A single API route, as emitted by an ingestion handler.
///
/// Stable id form:
/// - HTTP: `route:HTTP:{METHOD}:{path}` (e.g. `route:HTTP:POST:/api/users`)
/// - GraphQL: `route:GraphQL:{type}.{field}` (e.g. `route:GraphQL:Query.users`)
/// - gRPC: `route:gRPC:{service}.{rpc}` (e.g. `route:gRPC:UserService.GetUser`)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiRoute {
    /// Canonical id — see struct doc for the format per protocol.
    pub id: String,
    /// `'http'` | `'graphql'` | `'grpc'` | `'trpc'`.
    pub protocol: String,
    /// HTTP method (e.g. `GET`) or RPC type (`query`, `mutation`, `unary`).
    pub method: String,
    /// URL path (HTTP) or `Type.field` (GraphQL) or `Service.Rpc` (gRPC).
    pub path: String,
    /// Soft FK to the `graph_nodes` row that implements this route.
    /// `None` when the resolver could not match a handler — the route
    /// is still emitted so the user can fix the spec/handler naming
    /// and re-ingest.
    pub handler_symbol: Option<String>,
    /// Absolute file path or URL of the spec the route came from.
    pub spec_source: String,
    /// SHA256 of the spec — idempotent re-ingest key.
    pub spec_hash: String,
    /// Framework hint (e.g. `axum`, `actix-web`, `express`).
    /// Optional; helps agents disambiguate when multiple frameworks
    /// contribute to the same workspace.
    pub framework: Option<String>,
    /// Confidence in the route→handler mapping (0.0–1.0).
    pub confidence: f32,
    /// Open key/value bag for protocol-specific properties
    /// (e.g. HTTP `parameters`, GraphQL `args`, gRPC `request_type`).
    pub properties: Value,
}

/// Edge from a route to its handler implementation.
///
/// The composite PK `(source_route_id, target_symbol_id, edge_kind)` is
/// unique by construction — re-emitting the same edge is a no-op.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiRouteEdge {
    pub source_route_id: String,
    pub target_symbol_id: String,
    /// `'http_calls'` | `'graphql_calls'` | `'grpc_calls'` | `'trpc_calls'`.
    pub edge_kind: String,
    pub confidence: f32,
    /// Per-edge provenance (e.g. `{"operation_id": "createUser", "tier": 1}`).
    pub metadata: Value,
}

/// Stable edge-kind constants. Re-exported by handlers so the wire
/// format is centralised — the SQL CHECK constraint is the source of
/// truth at the storage layer; these constants ensure handler
/// emitters do not typo a string.
pub const EDGE_KIND_HTTP_CALLS: &str = "http_calls";
pub const EDGE_KIND_GRAPHQL_CALLS: &str = "graphql_calls";
pub const EDGE_KIND_GRPC_CALLS: &str = "grpc_calls";
pub const EDGE_KIND_TRPC_CALLS: &str = "trpc_calls";

/// Stable protocol constants. Used by both emitters (to set
/// `ApiRoute.protocol`) and the SQL CHECK constraint.
pub const PROTOCOL_HTTP: &str = "http";
pub const PROTOCOL_GRAPHQL: &str = "graphql";
pub const PROTOCOL_GRPC: &str = "grpc";
pub const PROTOCOL_TRPC: &str = "trpc";

/// Read+write port for the API routes persistence layer.
///
/// Implementations are responsible for the SQL ↔ Rust translation; the
/// port surface stays free of any database types so handlers stay
/// testable.
#[async_trait]
pub trait EdgeEmitter: Send + Sync {
    /// Upsert a single route by its canonical id.
    ///
    /// Returns `Ok(true)` if a new row was created, `Ok(false)` if an
    /// existing row was updated. Errors propagate via `ExplorerError`.
    async fn upsert_route(&self, route: &ApiRoute) -> ExplorerResult<bool>;

    /// Upsert a single edge by its composite PK.
    ///
    /// Returns `Ok(true)` if a new row was created, `Ok(false)` if an
    /// existing row was updated.
    async fn emit_edge(&self, edge: &ApiRouteEdge) -> ExplorerResult<bool>;

    /// Batch upsert — convenience over `upsert_route` + `emit_edge`
    /// in a single transaction. All-or-nothing: if any row fails, the
    /// whole batch is rolled back.
    ///
    /// Default impl loops over individual calls without a transaction
    /// (atomicity is the caller's responsibility). Implementations
    /// are encouraged to override with a real `BEGIN/COMMIT` block.
    async fn emit_many(
        &self,
        routes: &[ApiRoute],
        edges: &[ApiRouteEdge],
    ) -> ExplorerResult<BatchStats> {
        let mut stats = BatchStats::default();
        for route in routes {
            if self.upsert_route(route).await? {
                stats.routes_created += 1;
            } else {
                stats.routes_updated += 1;
            }
        }
        for edge in edges {
            if self.emit_edge(edge).await? {
                stats.edges_created += 1;
            } else {
                stats.edges_updated += 1;
            }
        }
        Ok(stats)
    }

    /// Look up a route by `(method, path)` — used by
    /// `cognicode_trace_route` and by re-ingestion handlers to
    /// short-circuit when the spec hash is unchanged.
    ///
    /// Returns `None` when no route matches.
    async fn find_route_by_method_path(
        &self,
        method: &str,
        path: &str,
    ) -> ExplorerResult<Option<ApiRoute>>;

    /// Return every route whose `spec_hash` matches — used by the
    /// idempotent re-ingest short-circuit.
    async fn find_routes_by_spec_hash(
        &self,
        spec_hash: &str,
    ) -> ExplorerResult<Vec<ApiRoute>>;

    /// Return every route whose `handler_symbol` matches — used by
    /// the trace-route tool's "reverse" direction (handler → routes).
    async fn find_routes_by_handler(
        &self,
        handler_symbol: &str,
    ) -> ExplorerResult<Vec<ApiRoute>>;
}

/// Counts from a batch upsert operation.
///
/// Returned by `EdgeEmitter::emit_many`. Useful for observability
/// (e.g. surfacing "ingested N routes, updated M" in the tool response).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchStats {
    pub routes_created: usize,
    pub routes_updated: usize,
    pub edges_created: usize,
    pub edges_updated: usize,
}

impl BatchStats {
    /// Total number of rows touched (created + updated) in this batch.
    pub fn total_touched(&self) -> usize {
        self.routes_created
            + self.routes_updated
            + self.edges_created
            + self.edges_updated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The edge-kind and protocol constants must round-trip exactly
    /// against the SQL CHECK constraints in `m0012_route_nodes_protocol_edges.sql`.
    /// If a future contributor renames a constant, they must update
    /// the SQL constraint in lockstep — this test is the canary.
    #[test]
    fn edge_kind_constants_match_sql_check() {
        // SQL: CHECK (edge_kind IN ('http_calls', 'graphql_calls', 'grpc_calls', 'trpc_calls'))
        assert_eq!(EDGE_KIND_HTTP_CALLS, "http_calls");
        assert_eq!(EDGE_KIND_GRAPHQL_CALLS, "graphql_calls");
        assert_eq!(EDGE_KIND_GRPC_CALLS, "grpc_calls");
        assert_eq!(EDGE_KIND_TRPC_CALLS, "trpc_calls");
    }

    #[test]
    fn protocol_constants_match_sql_check() {
        // SQL: CHECK (protocol IN ('http', 'graphql', 'grpc', 'trpc'))
        assert_eq!(PROTOCOL_HTTP, "http");
        assert_eq!(PROTOCOL_GRAPHQL, "graphql");
        assert_eq!(PROTOCOL_GRPC, "grpc");
        assert_eq!(PROTOCOL_TRPC, "trpc");
    }

    #[test]
    fn batch_stats_total_touched() {
        let stats = BatchStats {
            routes_created: 5,
            routes_updated: 2,
            edges_created: 7,
            edges_updated: 1,
        };
        assert_eq!(stats.total_touched(), 15);
    }

    #[test]
    fn api_route_id_format_examples() {
        // HTTP
        let http = ApiRoute {
            id: "route:HTTP:POST:/api/users".to_string(),
            protocol: PROTOCOL_HTTP.to_string(),
            method: "POST".to_string(),
            path: "/api/users".to_string(),
            handler_symbol: Some("symbol:src/api/users.rs:create_user:42".to_string()),
            spec_source: "/abs/openapi.yaml".to_string(),
            spec_hash: "deadbeef".to_string(),
            framework: Some("axum".to_string()),
            confidence: 0.95,
            properties: serde_json::json!({}),
        };
        assert_eq!(http.id, "route:HTTP:POST:/api/users");

        // GraphQL
        let gql = ApiRoute {
            id: "route:GraphQL:Query.users".to_string(),
            protocol: PROTOCOL_GRAPHQL.to_string(),
            method: "query".to_string(),
            path: "Query.users".to_string(),
            ..http.clone()
        };
        assert_eq!(gql.id, "route:GraphQL:Query.users");

        // gRPC
        let grpc = ApiRoute {
            id: "route:gRPC:UserService.GetUser".to_string(),
            protocol: PROTOCOL_GRPC.to_string(),
            method: "unary".to_string(),
            path: "UserService.GetUser".to_string(),
            ..http.clone()
        };
        assert_eq!(grpc.id, "route:gRPC:UserService.GetUser");
    }
}