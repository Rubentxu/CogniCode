-- Migration `m0012_route_nodes_protocol_edges` — API route ingestion tables.
--
-- Cycle e15.5 introduces cross-service protocol edge ingestion
-- (HTTP, GraphQL, gRPC, tRPC). Routes are emitted by spec-based
-- ingestion handlers (`cognicode_ingest_openapi`,
-- `cognicode_ingest_graphql_schema`, `cognicode_ingest_grpc_proto`)
-- and become first-class graph nodes via two new tables:
--
-- 1. `api_routes` — one row per HTTP/gRPC/GraphQL/tRPC route.
--    Stable id form: `route:{protocol}:{method}:{path}` for HTTP,
--    `route:GraphQL:{type}.{field}` for GraphQL,
--    `route:gRPC:{service}.{rpc}` for gRPC. The `handler_symbol`
--    column is a soft FK to the `graph_nodes` row that implements
--    the handler (nullable: unresolved handlers are kept as routes
--    with `handler_symbol = NULL` so the user can fix the spec or
--    handler naming and re-ingest).
--
-- 2. `api_route_edges` — edges from routes to their handlers
--    (one row per route × handler × edge_kind). The composite PK
--    matches `(source_route_id, target_symbol_id, edge_kind)`. The
--    `metadata` JSONB column carries per-edge provenance
--    (e.g. `{"operation_id": "createUser", "tier": 1}`).
--
-- Why dedicated tables vs reusing `graph_nodes`:
-- - `graph_nodes` requires `(kind, label, source_path, properties)`
--   and routes need first-class indexed columns (`method`, `path`,
--   `handler_symbol`) for efficient "find route by path" queries.
-- - Routes can be queried without the `multimodal` Cargo feature
--   (the routes table is always available); `graph_nodes.route`
--   rows would require the feature flag.
-- - Trade-off: an extra table, but reads are cheap and the
--   alternative is a fat JSONB column with functional indexes.
--
-- The DDL is `IF NOT EXISTS`-idempotent and additive — no ALTER on
-- existing tables. Re-running migrations on a populated DB is a
-- no-op.

-- =============================================================================
-- api_routes
-- =============================================================================

CREATE TABLE IF NOT EXISTS api_routes (
    id              TEXT PRIMARY KEY,
    protocol        TEXT NOT NULL,           -- 'http' | 'graphql' | 'grpc' | 'trpc'
    method          TEXT NOT NULL,           -- 'GET' | 'POST' | 'query' | 'mutation' | 'unary' | ...
    path            TEXT NOT NULL,           -- '/api/users' | 'Query.users' | 'UserService.GetUser'
    handler_symbol  TEXT,                    -- FK to graph_nodes.id (nullable; resolver is best-effort)
    spec_source     TEXT NOT NULL,           -- absolute file path or URL of the spec
    spec_hash       TEXT NOT NULL,           -- SHA256 of the spec — idempotent re-ingest key
    framework       TEXT,                    -- 'axum' | 'actix-web' | 'express' | 'fastify' | 'hono' | 'nestjs' | 'tsoa' | ...
    confidence      REAL NOT NULL DEFAULT 1.0,
    properties      JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT chk_api_routes_protocol
        CHECK (protocol IN ('http', 'graphql', 'grpc', 'trpc')),
    CONSTRAINT chk_api_routes_confidence
        CHECK (confidence >= 0.0 AND confidence <= 1.0)
);

-- Fast lookup: "which handler implements this route?" — covers
-- `cognicode_trace_route` and trace-route E2E specs.
CREATE INDEX IF NOT EXISTS idx_api_routes_handler_symbol
    ON api_routes(handler_symbol)
    WHERE handler_symbol IS NOT NULL;

-- Fast filter: "show me all HTTP routes from this spec."
CREATE INDEX IF NOT EXISTS idx_api_routes_protocol
    ON api_routes(protocol);

-- Idempotent re-ingest key: when `spec_hash` is unchanged, the
-- ingestion handler returns early with the existing routes.
CREATE INDEX IF NOT EXISTS idx_api_routes_spec_hash
    ON api_routes(spec_hash);

-- Path-based lookup: `WHERE method = 'POST' AND path = '/api/users'`.
CREATE INDEX IF NOT EXISTS idx_api_routes_method_path
    ON api_routes(method, path);

-- =============================================================================
-- api_route_edges
-- =============================================================================

CREATE TABLE IF NOT EXISTS api_route_edges (
    source_route_id  TEXT NOT NULL REFERENCES api_routes(id) ON DELETE CASCADE,
    target_symbol_id TEXT NOT NULL,
    edge_kind        TEXT NOT NULL,           -- 'http_calls' | 'graphql_calls' | 'grpc_calls' | 'trpc_calls'
    confidence       REAL NOT NULL DEFAULT 1.0,
    metadata         JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (source_route_id, target_symbol_id, edge_kind),
    CONSTRAINT chk_api_route_edges_kind
        CHECK (edge_kind IN ('http_calls', 'graphql_calls', 'grpc_calls', 'trpc_calls')),
    CONSTRAINT chk_api_route_edges_confidence
        CHECK (confidence >= 0.0 AND confidence <= 1.0)
);

-- Reverse lookup: "what routes does this symbol implement?" — used
-- by `cognicode_trace_route` to find handler→route relationships.
CREATE INDEX IF NOT EXISTS idx_api_route_edges_target
    ON api_route_edges(target_symbol_id);