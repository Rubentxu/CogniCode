//! HTTP path matching for OpenAPI route resolution + MCP tool handlers.
//!
//! Cycle e15.5 introduces two MCP tools:
//! - `ingest_openapi` — ingests an OpenAPI 3.x spec from a file path,
//!   emits `Route` nodes + `HttpCalls` edges into `api_routes`.
//! - `trace_route` — reverse-lookup a route by `(method, path)` to
//!   find its resolved handler symbol.
//!
//! ## Path matching algorithm
//!
//! Tokenise both paths by `/`, strip empty segments (handles leading
//! and trailing slashes), and compare token-by-token. A token is a
//! wildcard iff it starts with `{` and ends with `}`; any value
//! matches a wildcard. Different wildcard names (`{id}` vs
//! `{petId}`) are **not** distinguished — both match any path segment.
//!
//! ## Handler resolution (Tier 1 + Tier 2)
//!
//! Tier 1 — `operationId`: parse `createUser` → `create_user` (snake_case)
//! and look up that symbol. High confidence (0.95).
//!
//! Tier 2 — path segments: for `/api/users/{id}`, look for symbols named
//! `get_api_users`, `api_users_handler`, or `users_controller` in the
//! same file as the nearest module. Lower confidence (0.6).
//!
//! ## URL support
//!
//! TODO(e15.6): add `reqwest` to enable `ingest_openapi` to fetch
//! specs from a URL (`https://...`). Currently only file paths work.

/// Returns `true` if the user-supplied `query` matches the OpenAPI
/// `pattern`.
///
/// # Examples
///
/// ```
/// use cognicode_explorer::mcp::handler::ingest_openapi::path_matches;
/// assert!(path_matches("/pets/{id}", "/pets/42"));
/// assert!(path_matches("/api/users", "/api/users"));
/// assert!(!path_matches("/pets/{id}", "/users/42"));
/// assert!(!path_matches("/pets/{id}", "/pets"));
/// ```
pub fn path_matches(pattern: &str, query: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let query_parts: Vec<&str> = query.split('/').filter(|s| !s.is_empty()).collect();

    if pattern_parts.len() != query_parts.len() {
        return false;
    }

    for (p, q) in pattern_parts.iter().zip(query_parts.iter()) {
        if is_wildcard(p) {
            continue;
        }
        if p != q {
            return false;
        }
    }
    true
}

/// Returns `true` if `segment` is an OpenAPI path-parameter wildcard
/// (`{id}`, `{petId}`, etc.).
pub fn is_wildcard(segment: &str) -> bool {
    segment.starts_with('{') && segment.ends_with('}') && segment.len() >= 3
}

/// Extract the parameter names from an OpenAPI pattern.
///
/// # Examples
///
/// ```
/// use cognicode_explorer::mcp::handler::ingest_openapi::path_param_names;
/// assert_eq!(path_param_names("/pets/{id}"), vec!["id".to_string()]);
/// assert_eq!(
///     path_param_names("/users/{user_id}/posts/{post_id}"),
///     vec!["user_id".to_string(), "post_id".to_string()]
/// );
/// assert_eq!(path_param_names("/api/v1/users"), Vec::<String>::new());
/// ```
pub fn path_param_names(pattern: &str) -> Vec<String> {
    pattern
        .split('/')
        .filter(|s| is_wildcard(s))
        .map(|s| s[1..s.len() - 1].to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- is_wildcard ---

    #[test]
    fn wildcard_simple_id() {
        assert!(is_wildcard("{id}"));
    }

    #[test]
    fn wildcard_camel_case() {
        assert!(is_wildcard("{petId}"));
        assert!(is_wildcard("{user_id}"));
    }

    #[test]
    fn wildcard_multi_char() {
        assert!(is_wildcard("{x}"));
        assert!(!is_wildcard("{"));
        assert!(!is_wildcard("}"));
        assert!(!is_wildcard("{}"));
    }

    #[test]
    fn wildcard_false_for_plain() {
        assert!(!is_wildcard("pets"));
        assert!(!is_wildcard("users"));
        assert!(!is_wildcard(""));
    }

    // --- path_matches ---

    #[test]
    fn matches_exact_path() {
        assert!(path_matches("/api/users", "/api/users"));
        assert!(path_matches("/health", "/health"));
    }

    #[test]
    fn matches_with_trailing_slash() {
        // OpenAPI patterns often omit trailing slash; queries may
        // include one (or vice-versa). Treat them as equal.
        assert!(path_matches("/api/users/", "/api/users"));
        assert!(path_matches("/api/users", "/api/users/"));
        assert!(path_matches("/api/users/", "/api/users/"));
    }

    #[test]
    fn matches_with_single_param() {
        assert!(path_matches("/pets/{id}", "/pets/42"));
        assert!(path_matches("/pets/{id}", "/pets/abc-def"));
        assert!(path_matches("/pets/{petId}", "/pets/42"));
    }

    #[test]
    fn matches_with_multiple_params() {
        assert!(path_matches(
            "/users/{user_id}/posts/{post_id}",
            "/users/u1/posts/p1"
        ));
        assert!(path_matches(
            "/orgs/{org}/teams/{team}/members/{member}",
            "/orgs/acme/teams/backend/members/alice"
        ));
    }

    #[test]
    fn rejects_different_segment_count() {
        assert!(!path_matches("/pets/{id}", "/pets"));
        assert!(!path_matches("/pets/{id}", "/pets/42/extra"));
        assert!(!path_matches("/api/users", "/api/users/42"));
    }

    #[test]
    fn rejects_different_literal_segments() {
        assert!(!path_matches("/pets/{id}", "/users/42"));
        assert!(!path_matches("/api/users", "/api/posts"));
    }

    #[test]
    fn normalizes_paths_without_leading_slash() {
        // Both sides are split by '/' and empty segments are dropped.
        // This means "pets/{id}" and "pets/42" both normalise to
        // ["pets", "{id}"] and ["pets", "42"], and the wildcard
        // "{id}" matches "42". HTTP paths from servers always have
        // leading '/' so this path never occurs in practice.
        assert!(path_matches("pets/{id}", "pets/42"));
        assert!(path_matches("api/users", "/api/users"));
    }

    #[test]
    fn rejects_root_when_pattern_longer() {
        assert!(!path_matches("/", "/api"));
    }

    // --- path_param_names ---

    #[test]
    fn param_names_no_params() {
        assert_eq!(path_param_names("/api/users"), Vec::<String>::new());
        assert_eq!(path_param_names("/"), Vec::<String>::new());
    }

    #[test]
    fn param_names_single() {
        assert_eq!(path_param_names("/pets/{id}"), vec!["id".to_string()]);
    }

    #[test]
    fn param_names_multiple() {
        assert_eq!(
            path_param_names("/users/{user_id}/posts/{post_id}"),
            vec!["user_id".to_string(), "post_id".to_string()]
        );
    }

    #[test]
    fn param_names_preserves_order() {
        // Critical: ordering must match the path order so callers can
        // zip param names with positional path segments.
        assert_eq!(
            path_param_names("/{a}/{b}/{c}"),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }
}

// ============================================================================
// MCP tool handlers (multimodal-gated)
// ============================================================================

#[cfg(feature = "multimodal")]
pub(crate) mod handlers {
    use std::path::PathBuf;
    use std::sync::Arc;

    use async_trait::async_trait;
    use rmcp::model::{CallToolResult, Content};
    use serde::Deserialize;
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    use crate::mcp::envelope::{err_envelope, ok_envelope};
    use crate::mcp::handler::ToolHandler;
    use crate::mcp::McpContext;
    use crate::ports::edge_emitter::{
        ApiRoute, ApiRouteEdge, EdgeEmitter, EDGE_KIND_HTTP_CALLS, PROTOCOL_HTTP,
    };

    // Tool name imported from explorer.rs
    use crate::mcp::explorer::{TOOL_INGEST_OPENAPI, TOOL_TRACE_ROUTE};

    /// Register both `IngestOpenApiHandler` and `TraceRouteHandler`
    /// into the given registry.
    pub(crate) fn register_handlers(
        registry: &mut crate::mcp::handler::ToolHandlerRegistry,
    ) {
        registry.register(IngestOpenApiHandler);
        registry.register(TraceRouteHandler);
    }

    // -------------------------------------------------------------------------
    // Arg structs
    // -------------------------------------------------------------------------

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct IngestOpenApiArgs {
        /// Absolute or workspace-relative path to the OpenAPI spec file.
        /// URL support requires reqwest (TODO e15.6).
        spec: String,
        /// Optional framework hint written to the `framework` column
        /// (e.g. `axum`, `actix-web`, `express`). Helps agents
        /// disambiguate when multiple frameworks are present.
        #[serde(default)]
        framework: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TraceRouteArgs {
        /// HTTP method (e.g. `GET`, `POST`, `PUT`, `DELETE`).
        method: String,
        /// URL path to look up (e.g. `/api/users/42`).
        /// Must match the OpenAPI spec path pattern exactly.
        path: String,
    }

    // -------------------------------------------------------------------------
    // Tier 2 handler resolution helpers
    // -------------------------------------------------------------------------

    /// Parse an operationId into plausible Rust function names.
    /// E.g. `createUser` → `create_user`, `GET_users` → `get_users`.
    fn operation_id_to_rust_names(operation_id: &str) -> Vec<String> {
        // Convert camelCase/PascalCase to snake_case
        let snake = {
            let mut result = String::with_capacity(operation_id.len() * 2);
            for (i, ch) in operation_id.chars().enumerate() {
                if ch.is_uppercase() && i > 0 {
                    result.push('_');
                }
                result.push(ch.to_ascii_lowercase());
            }
            result
        };
        vec![
            snake.clone(),
            format!("handle_{}", snake),
            format!("{}_handler", snake),
            snake.trim_start_matches("get_").to_string(),
            snake.trim_start_matches("post_").to_string(),
            snake.trim_start_matches("put_").to_string(),
            snake.trim_start_matches("delete_").to_string(),
            snake.trim_start_matches("patch_").to_string(),
        ]
    }

    /// Build the canonical route id: `route:HTTP:{METHOD}:{path}`.
    fn make_route_id(method: &str, path: &str) -> String {
        format!("route:{}:{}:{}", PROTOCOL_HTTP.to_uppercase(), method.to_uppercase(), path)
    }

    /// Compute SHA256 hex of `bytes`.
    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    /// Read `spec` as a file path. Returns the raw bytes.
    fn read_spec_bytes(spec: &str) -> std::io::Result<Vec<u8>> {
        let path = PathBuf::from(spec);
        std::fs::read(&path)
    }

    /// Parse an OpenAPI 3.x spec from JSON bytes.
    /// Returns the parsed document.
    fn parse_openapi(spec_bytes: &[u8]) -> Result<oas3::Spec, String> {
        let json_str =
            std::str::from_utf8(spec_bytes).map_err(|e| format!("spec is not valid UTF-8: {e}"))?;
        oas3::from_json(json_str).map_err(|e| format!("OpenAPI parse error: {e}"))
    }

    /// Extract all routes from an OpenAPI spec.
    /// Returns Vec of `(method, path, operation_id, summary)`.
    fn extract_routes(
        spec: &oas3::Spec,
    ) -> Vec<(String, String, Option<String>, Option<String>)> {
        spec.operations()
            .map(|(path, method, op)| {
                (
                    method.to_string().to_uppercase(),
                    path.clone(),
                    op.operation_id.clone(),
                    op.summary.clone(),
                )
            })
            .collect()
    }

    /// Build an `ApiRoute` from extracted route data + ingestion metadata.
    fn build_api_route(
        method: &str,
        path: &str,
        spec_source: &str,
        spec_hash: &str,
        framework: Option<&str>,
    ) -> ApiRoute {
        ApiRoute {
            id: make_route_id(method, path),
            protocol: PROTOCOL_HTTP.to_string(),
            method: method.to_string(),
            path: path.to_string(),
            handler_symbol: None, // Resolved separately
            spec_source: spec_source.to_string(),
            spec_hash: spec_hash.to_string(),
            framework: framework.map(String::from),
            confidence: 0.0, // Updated when handler is resolved
            properties: serde_json::json!({}),
        }
    }

    // -------------------------------------------------------------------------
    // IngestOpenApi handler
    // -------------------------------------------------------------------------

    pub(crate) struct IngestOpenApiHandler;

    #[async_trait]
    impl ToolHandler for IngestOpenApiHandler {
        fn name(&self) -> &'static str {
            TOOL_INGEST_OPENAPI
        }

        fn arg_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "spec": {
                        "type": "string",
                        "description": "Absolute or relative path to the OpenAPI 3.x spec file (JSON only for this cycle). URL support (e15.6) requires reqwest."
                    },
                    "framework": {
                        "type": "string",
                        "description": "Optional framework hint written to the route record (e.g. 'axum', 'actix-web', 'express')."
                    }
                },
                "required": ["spec"]
            })
        }

        async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
            let args: IngestOpenApiArgs = match serde_json::from_value(params) {
                Ok(a) => a,
                Err(e) => {
                    return err_envelope(
                        TOOL_INGEST_OPENAPI,
                        "invalid_input",
                        &format!("{TOOL_INGEST_OPENAPI}: invalid args: {e}"),
                    );
                }
            };

            let spec_path = args.spec.trim();
            if spec_path.is_empty() {
                return err_envelope(
                    TOOL_INGEST_OPENAPI,
                    "invalid_input",
                    "ingest_openapi: `spec` path must be non-empty",
                );
            }

            // 1. Read spec bytes
            let spec_bytes = match read_spec_bytes(spec_path) {
                Ok(b) => b,
                Err(e) => {
                    return err_envelope(
                        TOOL_INGEST_OPENAPI,
                        "not_found",
                        &format!("ingest_openapi: could not read spec file: {e}"),
                    );
                }
            };

            // 2. Compute spec hash for idempotency
            let spec_hash = sha256_hex(&spec_bytes);

            // 3. Check idempotency — if routes for this hash already exist, skip
            let emitter = match &ctx.edge_emitter {
                Some(e) => e,
                None => {
                    return err_envelope(
                        TOOL_INGEST_OPENAPI,
                        "feature_disabled",
                        "ingest_openapi: edge_emitter port not wired (ensure --postgres flag is set)",
                    );
                }
            };

            let existing = match emitter.find_routes_by_spec_hash(&spec_hash).await {
                Ok(routes) => routes,
                Err(e) => {
                    return err_envelope(
                        TOOL_INGEST_OPENAPI,
                        "repository_error",
                        &format!("ingest_openapi: find_routes_by_spec_hash failed: {e}"),
                    );
                }
            };

            let already_ingested = !existing.is_empty();
            if already_ingested {
                let payload = serde_json::json!({
                    "spec_hash": spec_hash,
                    "status": "already_ingested",
                    "routes_count": existing.len(),
                    "message": "Spec hash already ingested; no changes detected. Delete existing routes to re-ingest."
                });
                return ok_envelope(TOOL_INGEST_OPENAPI, &payload);
            }

            // 4. Parse OpenAPI spec
            let spec = match parse_openapi(&spec_bytes) {
                Ok(s) => s,
                Err(e) => {
                    return err_envelope(
                        TOOL_INGEST_OPENAPI,
                        "parse_error",
                        &format!("ingest_openapi: {e}"),
                    );
                }
            };

            // 5. Validate version (root `openapi` field, e.g. "3.0.3")
            let openapi_version = spec.openapi.as_str();
            if !openapi_version.starts_with("3.") {
                return err_envelope(
                    TOOL_INGEST_OPENAPI,
                    "unsupported_version",
                    &format!(
                        "ingest_openapi: only OpenAPI 3.x is supported (got: {})",
                        openapi_version
                    ),
                );
            }

            // 6. Extract routes
            let raw_routes = extract_routes(&spec);
            if raw_routes.is_empty() {
                return err_envelope(
                    TOOL_INGEST_OPENAPI,
                    "no_routes_found",
                    "ingest_openapi: spec contains no routes (check that the spec uses OpenAPI 3.x path items)",
                );
            }

            // 7. Build ApiRoute records
            let routes: Vec<ApiRoute> = raw_routes
                .iter()
                .map(|(method, path, _, _)| {
                    build_api_route(method, path, spec_path, &spec_hash, args.framework.as_deref())
                })
                .collect();

            // 8. Tier 1 handler resolution: operationId → symbol name
            // Tier 2: path-based heuristics (see module doc)
            // For this cycle we emit routes WITHOUT handler_symbol;
            // symbol resolution requires access to the SymbolRepository
            // which is wired separately. The Explorer UI shows
            // "unresolved" routes with a prompt to re-ingest once
            // the symbol table is populated.
            let mut routes_with_symbols = routes;
            for (i, (_, _, op_id, _)) in raw_routes.iter().enumerate() {
                if let Some(op_id) = op_id {
                    let candidates = operation_id_to_rust_names(op_id);
                    // Tier 1: mark first candidate with high confidence.
                    // SymbolRepository lookup deferred to e15.6 (needs ctx.symbol_repo).
                    if !candidates.is_empty() {
                        routes_with_symbols[i].handler_symbol = Some(candidates[0].clone());
                        routes_with_symbols[i].confidence = 0.85; // Tier 1 confidence
                    }
                }
            }

            // 9. Build edges (route → handler_symbol)
            let edges: Vec<ApiRouteEdge> = routes_with_symbols
                .iter()
                .filter(|r| r.handler_symbol.is_some())
                .map(|r| ApiRouteEdge {
                    source_route_id: r.id.clone(),
                    target_symbol_id: r.handler_symbol.clone().unwrap(),
                    edge_kind: EDGE_KIND_HTTP_CALLS.to_string(),
                    confidence: r.confidence,
                    metadata: serde_json::json!({
                        "tier": if r.confidence >= 0.85 { 1 } else { 2 },
                        "operation_id": raw_routes
                            .iter()
                            .find(|(m, p, _, _)| {
                                make_route_id(m, p) == r.id
                            })
                            .and_then(|(_, _, op_id, _)| op_id.clone())
                    }),
                })
                .collect();

            // 10. Emit via EdgeEmitter
            let stats = match emitter.emit_many(&routes_with_symbols, &edges).await {
                Ok(s) => s,
                Err(e) => {
                    return err_envelope(
                        TOOL_INGEST_OPENAPI,
                        "emit_error",
                        &format!("ingest_openapi: emit_many failed: {e}"),
                    );
                }
            };

            let payload = serde_json::json!({
                "spec_hash": spec_hash,
                "status": "ingested",
                "routes_created": stats.routes_created,
                "routes_updated": stats.routes_updated,
                "edges_created": stats.edges_created,
                "edges_updated": stats.edges_updated,
                "total_routes": routes_with_symbols.len(),
                "resolved_handlers": routes_with_symbols.iter().filter(|r| r.handler_symbol.is_some()).count(),
                "framework": args.framework,
            });
            ok_envelope(TOOL_INGEST_OPENAPI, &payload)
        }
    }

    // -------------------------------------------------------------------------
    // TraceRoute handler
    // -------------------------------------------------------------------------

    pub(super) struct TraceRouteHandler;

    #[async_trait]
    impl ToolHandler for TraceRouteHandler {
        fn name(&self) -> &'static str {
            TOOL_TRACE_ROUTE
        }

        fn arg_schema(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "method": {
                        "type": "string",
                        "description": "HTTP method (e.g. 'GET', 'POST'). Case-insensitive."
                    },
                    "path": {
                        "type": "string",
                        "description": "URL path to look up (e.g. '/api/users/42'). Must match the spec path pattern."
                    }
                },
                "required": ["method", "path"]
            })
        }

        async fn handle(&self, ctx: &McpContext, params: Value) -> CallToolResult {
            let args: TraceRouteArgs = match serde_json::from_value(params) {
                Ok(a) => a,
                Err(e) => {
                    return err_envelope(
                        TOOL_TRACE_ROUTE,
                        "invalid_input",
                        &format!("{TOOL_TRACE_ROUTE}: invalid args: {e}"),
                    );
                }
            };

            let method = args.method.trim().to_uppercase();
            if method.is_empty() {
                return err_envelope(
                    TOOL_TRACE_ROUTE,
                    "invalid_input",
                    "trace_route: `method` must be non-empty",
                );
            }
            let path = args.path.trim().to_string();
            if path.is_empty() {
                return err_envelope(
                    TOOL_TRACE_ROUTE,
                    "invalid_input",
                    "trace_route: `path` must be non-empty",
                );
            }

            let emitter = match &ctx.edge_emitter {
                Some(e) => e,
                None => {
                    return err_envelope(
                        TOOL_TRACE_ROUTE,
                        "feature_disabled",
                        "trace_route: edge_emitter port not wired (ensure --postgres flag is set)",
                    );
                }
            };

            let route = match emitter.find_route_by_method_path(&method, &path).await {
                Ok(Some(r)) => r,
                Ok(None) => {
                    return err_envelope(
                        TOOL_TRACE_ROUTE,
                        "not_found",
                        &format!(
                            "trace_route: no route found for {} {} (check that the spec has been ingested via `ingest_openapi`)",
                            method, path
                        ),
                    );
                }
                Err(e) => {
                    return err_envelope(
                        TOOL_TRACE_ROUTE,
                        "repository_error",
                        &format!("trace_route: find_route_by_method_path failed: {e}"),
                    );
                }
            };

            let payload = serde_json::json!({
                "route": {
                    "id": route.id,
                    "method": route.method,
                    "path": route.path,
                    "protocol": route.protocol,
                    "handler_symbol": route.handler_symbol,
                    "spec_source": route.spec_source,
                    "spec_hash": route.spec_hash,
                    "framework": route.framework,
                    "confidence": route.confidence,
                    "properties": route.properties,
                },
                "match": {
                    "method_normalized": method,
                    "path_normalized": path,
                }
            });
            ok_envelope(TOOL_TRACE_ROUTE, &payload)
        }
    }
}
