# mcp-multimodal-tools Specification (OBSOLETE — 2026-08-10)

## Purpose

> **Status: OBSOLETE** — `cargo test --features multimodal` does not compile on
> the default toolchain (compile debt in `crates/cognicode-runtime/src/ladybug/lib.rs:1158`).
> The 2 MCP tools (`docs_ingest`, `graph_search`) described here exist behind
> `#[cfg(feature = "multimodal")]` and cannot be exercised by standard CI,
> so the spec evidence cannot be re-validated. Marked OBSOLETE pending
> restoration of the `multimodal` feature.

Adds 2 new MCP tools to the explorer group: `docs_ingest` (triggers documentation ingestion) and `graph_search` (multimodal-aware search). Both tools sit alongside the 28 existing MCP tools and are feature-gated behind the `multimodal` Cargo feature. Tool input/output schemas follow the existing JSON-Schema style. The `multimodal` feature disabled ⇒ the tools are not registered (regression gate: clients that probe for them get a clear "tool not found" error rather than a panic).

> **Note on classification**: The proposal lists this capability as "Modified" because it adds to the existing MCP tool group, but no main spec exists for `mcp-multimodal-tools`. This spec is written as a NEW spec in the change folder; on archive it becomes the main spec.

## Requirements

### Requirement: docs_ingest Tool Registration

The `docs_ingest` tool MUST be registered in `crates/cognicode-explorer/src/mcp.rs` under the explorer tool group. Its input schema, output schema, and behavior are described in the `docs-source-adapter` spec. The MCP server MUST advertise the tool in its `tools/list` response when the `multimodal` feature is enabled.

#### Scenario: Tool is listed when feature is enabled
- GIVEN `cargo run -p cognicode-mcp --features multimodal`
- WHEN a client sends `tools/list`
- THEN the response includes `docs_ingest` with its full JSON Schema

#### Scenario: Tool is hidden when feature is disabled
- GIVEN `cargo run -p cognicode-mcp` (no features)
- WHEN a client sends `tools/list`
- THEN `docs_ingest` MUST NOT appear in the response (regression gate)

#### Scenario: Calling hidden tool returns error
- GIVEN the `multimodal` feature is disabled
- WHEN a client sends `tools/call` with `name: "docs_ingest"`
- THEN the server MUST return a JSON-RPC error with code `-32601` (Method not found) and message `tool 'docs_ingest' is not available in this build`

### Requirement: graph_search Tool

The `graph_search` tool MUST be a multimodal-aware search across the generic graph (`graph_nodes` + `graph_edges`). Input schema:

```json
{
  "type": "object",
  "properties": {
    "query": { "type": "string", "description": "Free-text query (supports FTS5 syntax)" },
    "kinds": {
      "type": "array",
      "items": { "enum": ["symbol", "decision", "doc", "issue", "evidence"] },
      "description": "Filter by node kinds; default: all"
    },
    "limit": { "type": "integer", "default": 20, "minimum": 1, "maximum": 200 }
  },
  "required": ["query"]
}
```

Output schema:

```json
{
  "type": "object",
  "properties": {
    "results": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "node_id": { "type": "string" },
          "kind": { "enum": ["symbol", "decision", "doc", "issue", "evidence"] },
          "label": { "type": "string" },
          "score": { "type": "number" },
          "snippet": { "type": "string" }
        }
      }
    },
    "total": { "type": "integer" }
  }
}
```

The tool MUST combine FTS5 search on `label` + `metadata` with a kind filter. Results are sorted by `score` descending, capped at `limit`.

#### Scenario: Search returns multimodal results
- GIVEN `graph_nodes` contains 1 Decision (label: "ADR-0001") and 1 Doc (label: "Auth Guide")
- WHEN `graph_search` is called with `{ "query": "auth" }`
- THEN the response includes the Doc node (matches `auth` in label) and the Decision node (matches `auth` in metadata)
- AND `score` is a normalized f64 in [0.0, 1.0]

#### Scenario: Kind filter narrows results
- GIVEN the same 2 nodes
- WHEN `graph_search` is called with `{ "query": "auth", "kinds": ["decision"] }`
- THEN only the Decision node is in `results`
- AND `total == 1`

#### Scenario: Limit caps result count
- GIVEN 50 Doc nodes matching the query
- WHEN `graph_search` is called with `{ "query": "guide", "limit": 10 }`
- THEN `results.length == 10`
- AND `total == 50` (the total is not capped)

#### Scenario: Empty query rejected
- GIVEN `graph_search` is called with `{ "query": "" }`
- WHEN validated
- THEN a schema error is returned mentioning `query must be non-empty`

#### Scenario: Empty graph returns empty results
- GIVEN `graph_nodes` is empty
- WHEN `graph_search` is called
- THEN `results == []` and `total == 0`

### Requirement: Multimodal-Aware Search Semantics

`graph_search` MUST rank results by a combined score: `0.6 * fts5_score + 0.4 * kind_match_bonus`. The `kind_match_bonus` is `0.2` for exact label match, `0.1` for partial match, `0.0` otherwise. Symbol nodes MUST NOT be ranked above multimodal nodes when the query contains keywords that match multimodal metadata (e.g., `status`, `date`, `section`).

#### Scenario: Multimodal metadata matches rank higher
- GIVEN a Doc node with `metadata.section == "Auth"` and a Symbol node labeled `auth_fn`
- WHEN `graph_search({ "query": "auth" })` is called
- THEN the Doc node has a higher score than the Symbol node (metadata match > label match for `auth` queries)

#### Scenario: Symbol-only query
- GIVEN only Symbol nodes exist
- WHEN `graph_search({ "query": "render" })` is called
- THEN results are symbol nodes ranked by FTS5 score

### Requirement: Tool Result Limits and Pagination

The `graph_search` tool MUST cap results at `limit` (default 20, max 200). For result sets larger than 200, the response includes a `next_cursor: Option<String>` field with a base64-encoded offset; clients pass it back via `cursor` to fetch the next page.

#### Scenario: Cursor-based pagination
- GIVEN 500 matching nodes
- WHEN `graph_search({ "query": "x", "limit": 50 })` is called
- THEN the response includes 50 results AND `next_cursor: Some("...")`
- WHEN the client calls again with `{ "query": "x", "limit": 50, "cursor": "<cursor>" }`
- THEN the next 50 results are returned

#### Scenario: No more pages
- GIVEN 25 matching nodes and `limit: 50`
- WHEN `graph_search` is called
- THEN the response includes all 25 results and `next_cursor: null`

### Requirement: Backward Compatibility with Existing Tools

The 28 existing MCP tools MUST continue to work unchanged. No existing input schema, output schema, or behavior is modified. The `graph_search` tool MUST be additive only.

#### Scenario: Existing tool list still works
- GIVEN the multimodal feature is enabled
- WHEN `tools/list` is called
- THEN the response includes all 28 existing tools PLUS the 2 new tools (30 total)

#### Scenario: Existing tool calls are unchanged
- GIVEN a pre-existing client calls `find_symbols` (any of the 28 tools)
- WHEN the call is made
- THEN the response is byte-for-byte identical to the pre-change behavior

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| FTS5 query contains a special character (e.g., `+`, `-`, `*`) | Sanitize the query by wrapping each term in double quotes; reject unbalanced parentheses with a clear error |
| `kinds` array contains an unknown kind (e.g., `["wibble"]`) | Schema validation rejects the call before reaching the executor |
| `graph_nodes` table does not exist (DB not migrated) | `graph_search` returns an empty result set with a warning log; no panic. The CLI/MCP start-up MUST warn if multimodal is enabled but the migration is missing |
| Concurrent `docs_ingest` and `graph_search` calls | Both run in parallel; ingest uses `INSERT ... ON CONFLICT DO UPDATE`; search sees the latest committed state (read-committed isolation) |
| `graph_search` is called with a query longer than 1024 chars | Schema validation rejects with `query length exceeds 1024 characters` |
| Result set is empty AND `limit` is 0 | Schema validation rejects with `limit must be >= 1` |
| Tool is called with `kinds: null` | Defaults to all kinds (no filter) |
| `next_cursor` is malformed (client tampering) | Tool returns a `cursor` error and resets the cursor to `None` |
| MCP client sends an empty request body | Returns JSON-RPC `-32600` (Invalid Request) |

## Out of Scope

- Federated search across multiple repos
- Vector embeddings / semantic search (FTS5 only in this change)
- Saved searches / search history
- Search-result grouping or faceting
- Re-ranking via user feedback
- Streaming results (the response is a single JSON payload, capped at 200 results per page)
- Multi-language tokenization (FTS5 unicode61 tokenizer only)

## TDD RED Gate

Before any implementation, the following tests MUST exist and be RED:

1. `tools/list` snapshot — 28 tools (feature off) and 30 tools (feature on)
2. `docs_ingest` tool call — schema validation: valid, empty paths, missing paths, paths not a string array (4 tests)
3. `graph_search` — 7 cases: success, kind filter, limit, empty query, empty graph, multimodal rank, symbol-only rank
4. `graph_search` pagination — 2 cases: first page, last page
5. `graph_search` cursor tampering — 1 case
6. Backward compatibility — 1 regression test that calls each of the 28 existing tools and checks the response shape
7. Feature-gate test — `cargo run -p cognicode-mcp --no-default-features` does NOT register `docs_ingest` or `graph_search`

## Dependencies

- `generic-graph-model` (provides `GenericGraphRepository`, `NodeKind`, `GraphNode`)
- `docs-source-adapter` (provides `docs_ingest` execution backend; same code path as the CLI)
- `mcp-edge-metadata` (the existing MCP tool registry pattern; no changes to the registry mechanism)
- Existing `tools/list` / `tools/call` JSON-RPC dispatch (no protocol changes)
- `pg_trgm` or built-in FTS5 for the search index (Postgres only in this change; SQLite users get `graph_search` returning empty results with a startup warning)

## Risks

- The FTS5 query sanitizer MUST be reviewed for SQL injection — a fuzz test against the sanitizer is recommended.
- The kind filter enum is duplicated between the frontend (Zod) and backend (Rust enum). Drift between them would silently break filtering. Mitigation: generate the Rust enum from the same source-of-truth list used by the frontend (out of scope for this change; flagged for follow-up).
- `next_cursor` is opaque (base64) — clients should not depend on its format. Documented in the tool description.
