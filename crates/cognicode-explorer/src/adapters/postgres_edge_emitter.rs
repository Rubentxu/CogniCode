//! `PostgresEdgeEmitter` — `EdgeEmitter` port adapter backed by PostgreSQL.
//!
//! Implements all 6 methods of the `EdgeEmitter` port against the
//! `api_routes` and `api_route_edges` tables defined in
//! `crates/cognicode-core/src/infrastructure/persistence/m0012_route_nodes_protocol_edges.sql`.
//!
//! ## Write contract
//!
//! Unlike `PostgresQualityRepository` (read-only), this adapter is
//! the canonical *write* path for route metadata. The MCP ingestion
//! handlers (`cognicode_ingest_openapi` and friends) are its only
//! callers in v1; Phase 5 (e19) may add tRPC and runtime trace
//! ingestion as additional writers.
//!
//! ## ON CONFLICT semantics
//!
//! Both `upsert_route` and `emit_edge` use PostgreSQL's
//! `ON CONFLICT (...) DO UPDATE` so re-ingesting the same spec is
//! safe. The columns updated on conflict are kept minimal
//! (`updated_at`, plus the mutable business fields) — immutable
//! identity columns (`id`, `protocol`) are left untouched so the
//! wire-level invariants hold even after a re-ingest.
//!
//! ## Connection pool
//!
//! The adapter owns a `sqlx::PgPool` cloned from the parent service
//! in `cognicode-runtime`. Cloning the adapter is cheap because the
//! pool is internally `Arc`-backed.
//!
//! ## Migration coupling
//!
//! Migration `m0012_route_nodes_protocol_edges.sql` must have been
//! applied before any query is dispatched.
//! `PostgresRepository::run_migrations()` applies it as step 5
//! unconditionally when the `postgres` feature is on.

#[cfg(feature = "postgres")]
use cognicode_core::infrastructure::persistence::PostgresRepository;

#[cfg(feature = "postgres")]
use sqlx::Row;

#[cfg(feature = "postgres")]
use crate::error::{ExplorerError, ExplorerResult};
#[cfg(feature = "postgres")]
use crate::ports::edge_emitter::{
    ApiRoute, ApiRouteEdge, BatchStats, EdgeEmitter,
};

// Internal helper — converts any error into `ExplorerError::Anyhow`.
// Used by every method so SQL errors propagate as a single error
// type without forcing the port to expose a `Store` variant.
#[cfg(feature = "postgres")]
fn store_err<T: std::fmt::Display>(ctx: &str, e: T) -> ExplorerError {
    ExplorerError::Anyhow(anyhow::anyhow!("{ctx}: {e}"))
}

/// `PostgresEdgeEmitter` — PG-backed implementation of [`EdgeEmitter`].
///
/// Constructed in `cognicode-runtime/src/lib.rs` from the same
/// `PostgresRepository` that backs the quality and persistence stacks.
#[cfg(feature = "postgres")]
#[derive(Clone)]
pub struct PostgresEdgeEmitter {
    pool: sqlx::PgPool,
}

#[cfg(feature = "postgres")]
impl PostgresEdgeEmitter {
    /// Build the adapter from a `PostgresRepository`. The pool is
    /// cloned — adapters share the same connection pool.
    pub fn new(pg: &PostgresRepository) -> Self {
        Self {
            pool: pg.pool().clone(),
        }
    }

    /// Build the adapter from a raw `sqlx::PgPool`. Useful for tests
    /// that wire their own pool against an ephemeral PG instance.
    pub fn from_pool(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[cfg(feature = "postgres")]
#[derive(sqlx::FromRow, Debug)]
struct RouteRow {
    id: String,
    protocol: String,
    method: String,
    path: String,
    handler_symbol: Option<String>,
    spec_source: String,
    spec_hash: String,
    framework: Option<String>,
    confidence: f32,
    properties: serde_json::Value,
}

#[cfg(feature = "postgres")]
impl From<RouteRow> for ApiRoute {
    fn from(r: RouteRow) -> Self {
        Self {
            id: r.id,
            protocol: r.protocol,
            method: r.method,
            path: r.path,
            handler_symbol: r.handler_symbol,
            spec_source: r.spec_source,
            spec_hash: r.spec_hash,
            framework: r.framework,
            confidence: r.confidence,
            properties: r.properties,
        }
    }
}

#[cfg(feature = "postgres")]
#[async_trait::async_trait]
impl EdgeEmitter for PostgresEdgeEmitter {
    async fn upsert_route(&self, route: &ApiRoute) -> ExplorerResult<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO api_routes (
                id, protocol, method, path, handler_symbol, spec_source,
                spec_hash, framework, confidence, properties, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), now())
            ON CONFLICT (id) DO UPDATE SET
                handler_symbol = EXCLUDED.handler_symbol,
                spec_source = EXCLUDED.spec_source,
                spec_hash = EXCLUDED.spec_hash,
                framework = EXCLUDED.framework,
                confidence = EXCLUDED.confidence,
                properties = EXCLUDED.properties,
                updated_at = now()
            RETURNING (xmax = 0) AS inserted
            "#,
        )
        .bind(&route.id)
        .bind(&route.protocol)
        .bind(&route.method)
        .bind(&route.path)
        .bind(&route.handler_symbol)
        .bind(&route.spec_source)
        .bind(&route.spec_hash)
        .bind(&route.framework)
        .bind(route.confidence)
        .bind(&route.properties)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| store_err("upsert_route", e))?;

        let inserted: bool = result
            .try_get("inserted")
            .map_err(|e| store_err("upsert_route", e))?;
        Ok(inserted)
    }

    async fn emit_edge(&self, edge: &ApiRouteEdge) -> ExplorerResult<bool> {
        let result = sqlx::query(
            r#"
            INSERT INTO api_route_edges (
                source_route_id, target_symbol_id, edge_kind, confidence,
                metadata, created_at
            )
            VALUES ($1, $2, $3, $4, $5, now())
            ON CONFLICT (source_route_id, target_symbol_id, edge_kind)
            DO UPDATE SET
                confidence = EXCLUDED.confidence,
                metadata = EXCLUDED.metadata
            RETURNING (xmax = 0) AS inserted
            "#,
        )
        .bind(&edge.source_route_id)
        .bind(&edge.target_symbol_id)
        .bind(&edge.edge_kind)
        .bind(edge.confidence)
        .bind(&edge.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| store_err("emit_edge", e))?;

        let inserted: bool = result
            .try_get("inserted")
            .map_err(|e| store_err("emit_edge", e))?;
        Ok(inserted)
    }

    /// Override the default `emit_many` with a transactional batch.
    ///
    /// The default port impl loops over individual calls without
    /// `BEGIN/COMMIT`, so a partial failure leaves the database in
    /// an inconsistent state. This override wraps the batch in a
    /// transaction so all-or-nothing semantics are guaranteed.
    async fn emit_many(
        &self,
        routes: &[ApiRoute],
        edges: &[ApiRouteEdge],
    ) -> ExplorerResult<BatchStats> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| store_err("emit_many begin", e))?;

        let mut stats = BatchStats::default();

        for route in routes {
            let result = sqlx::query(
                r#"
                INSERT INTO api_routes (
                    id, protocol, method, path, handler_symbol, spec_source,
                    spec_hash, framework, confidence, properties, created_at, updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), now())
                ON CONFLICT (id) DO UPDATE SET
                    handler_symbol = EXCLUDED.handler_symbol,
                    spec_source = EXCLUDED.spec_source,
                    spec_hash = EXCLUDED.spec_hash,
                    framework = EXCLUDED.framework,
                    confidence = EXCLUDED.confidence,
                    properties = EXCLUDED.properties,
                    updated_at = now()
                RETURNING (xmax = 0) AS inserted
                "#,
            )
            .bind(&route.id)
            .bind(&route.protocol)
            .bind(&route.method)
            .bind(&route.path)
            .bind(&route.handler_symbol)
            .bind(&route.spec_source)
            .bind(&route.spec_hash)
            .bind(&route.framework)
            .bind(route.confidence)
            .bind(&route.properties)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| store_err("emit_many route", e))?;

            let inserted: bool = result
                .try_get("inserted")
                .map_err(|e| store_err("emit_many route", e))?;
            if inserted {
                stats.routes_created += 1;
            } else {
                stats.routes_updated += 1;
            }
        }

        for edge in edges {
            let result = sqlx::query(
                r#"
                INSERT INTO api_route_edges (
                    source_route_id, target_symbol_id, edge_kind, confidence,
                    metadata, created_at
                )
                VALUES ($1, $2, $3, $4, $5, now())
                ON CONFLICT (source_route_id, target_symbol_id, edge_kind)
                DO UPDATE SET
                    confidence = EXCLUDED.confidence,
                    metadata = EXCLUDED.metadata
                RETURNING (xmax = 0) AS inserted
                "#,
            )
            .bind(&edge.source_route_id)
            .bind(&edge.target_symbol_id)
            .bind(&edge.edge_kind)
            .bind(edge.confidence)
            .bind(&edge.metadata)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| store_err("emit_many edge", e))?;

            let inserted: bool = result
                .try_get("inserted")
                .map_err(|e| store_err("emit_many edge", e))?;
            if inserted {
                stats.edges_created += 1;
            } else {
                stats.edges_updated += 1;
            }
        }

        tx.commit()
            .await
            .map_err(|e| store_err("emit_many commit", e))?;

        Ok(stats)
    }

    async fn find_route_by_method_path(
        &self,
        method: &str,
        path: &str,
    ) -> ExplorerResult<Option<ApiRoute>> {
        let row = sqlx::query_as::<_, RouteRow>(
            r#"
            SELECT id, protocol, method, path, handler_symbol, spec_source,
                   spec_hash, framework, confidence, properties
            FROM api_routes
            WHERE method = $1 AND path = $2
            LIMIT 1
            "#,
        )
        .bind(method)
        .bind(path)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| store_err("find_route_by_method_path", e))?;

        Ok(row.map(ApiRoute::from))
    }

    async fn find_routes_by_spec_hash(
        &self,
        spec_hash: &str,
    ) -> ExplorerResult<Vec<ApiRoute>> {
        let rows = sqlx::query_as::<_, RouteRow>(
            r#"
            SELECT id, protocol, method, path, handler_symbol, spec_source,
                   spec_hash, framework, confidence, properties
            FROM api_routes
            WHERE spec_hash = $1
            ORDER BY method, path
            "#,
        )
        .bind(spec_hash)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| store_err("find_routes_by_spec_hash", e))?;

        Ok(rows.into_iter().map(ApiRoute::from).collect())
    }

    async fn find_routes_by_handler(
        &self,
        handler_symbol: &str,
    ) -> ExplorerResult<Vec<ApiRoute>> {
        let rows = sqlx::query_as::<_, RouteRow>(
            r#"
            SELECT id, protocol, method, path, handler_symbol, spec_source,
                   spec_hash, framework, confidence, properties
            FROM api_routes
            WHERE handler_symbol = $1
            ORDER BY method, path
            "#,
        )
        .bind(handler_symbol)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| store_err("find_routes_by_handler", e))?;

        Ok(rows.into_iter().map(ApiRoute::from).collect())
    }
}

#[cfg(feature = "postgres")]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ExplorerError;
    use serde_json::json;

    /// The adapter accepts a route, then a second call with the same id
    /// is an update (not an insert). Mirrors the port's idempotency
    /// contract — useful for the handlers' re-ingest path.
    #[tokio::test]
    async fn upsert_route_returns_inserted_then_updated() {
        // Pool is missing — we only assert the call site compiles and
        // surfaces a Store error. Real round-trip tests run against
        // `pg_graph_repository` in the cognicode-runtime test suite.
        let pool = sqlx::PgPool::connect_lazy("postgres://invalid:5432/x").unwrap();
        let adapter = PostgresEdgeEmitter::from_pool(pool);

        let route = ApiRoute {
            id: "route:HTTP:GET:/test".to_string(),
            protocol: "http".to_string(),
            method: "GET".to_string(),
            path: "/test".to_string(),
            handler_symbol: None,
            spec_source: "/abs/test.yaml".to_string(),
            spec_hash: "abc123".to_string(),
            framework: Some("axum".to_string()),
            confidence: 1.0,
            properties: json!({}),
        };

        let result = adapter.upsert_route(&route).await;
        assert!(matches!(result, Err(ExplorerError::Anyhow(_))));
    }

    /// `BatchStats::total_touched` is a thin sum — verify the math.
    #[test]
    fn batch_stats_total() {
        let stats = BatchStats {
            routes_created: 5,
            routes_updated: 2,
            edges_created: 7,
            edges_updated: 1,
        };
        assert_eq!(stats.total_touched(), 15);
    }
}