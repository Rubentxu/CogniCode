# issues-ingest-mcp Specification (MODIFIED — delta from `mcp-multimodal-tools`)

> **Delta spec**: this file contains only the **ADDED** requirements introduced by `issue-tracker-adapter`. The existing main spec at `openspec/specs/mcp-multimodal-tools/spec.md` is unchanged. On archive, the ADDED section is merged into the main spec.

## ADDED Requirements

### Requirement: issues_ingest Tool Registration

The `issues_ingest` tool MUST be registered in `crates/cognicode-explorer/src/mcp.rs` immediately after `docs_ingest` (tool #31). It MUST be gated behind the `multimodal` Cargo feature (constant + dispatch arm + schema entry all `#[cfg(feature = "multimodal")]`-gated). The tool constant MUST be `pub const TOOL_ISSUES_INGEST: &str = "issues_ingest";` and appended to `TOOL_NAMES` inside the same `#[cfg(feature = "multimodal")]` block.

#### Scenario: Tool is listed when feature is enabled
- GIVEN `cargo run -p cognicode-mcp --features multimodal`
- WHEN a client sends `tools/list`
- THEN the response includes `issues_ingest` with its full JSON Schema
- AND the total tool count is 31 (was 30 before this change)

#### Scenario: Tool is hidden when feature is disabled
- GIVEN `cargo run -p cognicode-mcp` (no features)
- WHEN a client sends `tools/list`
- THEN `issues_ingest` MUST NOT appear (regression gate, mirrors `docs_ingest`)

#### Scenario: Calling hidden tool returns Method Not Found
- GIVEN the `multimodal` feature is disabled
- WHEN a client sends `tools/call` with `name: "issues_ingest"`
- THEN the server returns a JSON-RPC error with code `-32601` and message `tool 'issues_ingest' is not available in this build`

### Requirement: issues_ingest Input Schema

The input schema MUST be:

```json
{
  "type": "object",
  "properties": {
    "source": {
      "type": "string",
      "description": "Either a `https://github.com/{owner}/{repo}` URL (triggers the GitHub Issues REST API via octocrab) or a local filesystem path to a git repository (triggers `git log` parsing). Required, non-empty."
    },
    "mode": {
      "type": "string",
      "enum": ["github", "git_log", "both"],
      "description": "Ingestion strategy. `github` = GitHub Issues API only. `git_log` = local commit-message references only. `both` = fetch issues AND parse the local log (requires a matching remote; commits referencing issues are merged). Default: `github` when the source is a URL, `git_log` when it is a path."
    },
    "max_issues": {
      "type": "integer",
      "description": "Hard cap on issues fetched from the GitHub API per call. Defaults to 1000. Values > 5000 are silently capped to 5000."
    }
  },
  "required": ["source"]
}
```

#### Scenario: Empty source rejected
- GIVEN `issues_ingest` called with `{ "source": "" }`
- WHEN validated
- THEN the tool returns `error_code: "invalid_input"` and message `missing required arg 'source' (must be a non-empty string)`

#### Scenario: Mode auto-detected from source shape
- GIVEN a URL source `https://github.com/acme/widgets`
- WHEN `issues_ingest` is called without `mode`
- THEN the tool defaults to `mode = "github"`
- AND a path source `/repo/.git` defaults to `mode = "git_log"`

### Requirement: issues_ingest Output Schema

The tool MUST return an `McpResultEnvelope` whose payload is:

```json
{
  "type": "object",
  "properties": {
    "nodes_created": { "type": "integer", "description": "New `NodeKind::Issue` rows inserted (excludes re-ingested ids)." },
    "edges_created": { "type": "integer", "description": "New `Resolves` / `References` edges inserted (excludes re-ingested triples)." },
    "issues_skipped": { "type": "integer", "description": "Issues filtered by `max_issues` cap or by GitHub API pagination cut-off." },
    "errors": { "type": "array", "items": { "type": "string" }, "description": "Per-issue parse / API errors (truncated to the first 20)." }
  }
}
```

#### Scenario: First ingest reports positive counts
- GIVEN a mocked `GitHubClient` returning 5 issues
- WHEN `issues_ingest({ "source": "https://github.com/acme/widgets" })` is called
- THEN the response includes `nodes_created == 5` and `edges_created >= 0`

#### Scenario: Re-ingest reports zero
- GIVEN the same 5 issues were ingested in a previous call
- WHEN `issues_ingest` runs again on the same source
- THEN `nodes_created == 0` and `edges_created == 0` (idempotent upsert)

### Requirement: issues_ingest Dispatch

The dispatch helper `dispatch_issues_ingest(service, graph_repo, arguments)` MUST:
1. Parse + validate the args (returning `invalid_input` on schema violations).
2. Construct an `IssuesExtractor` and call `.extract(source)`.
3. On `Ok(nodes)`, call `graph_repo.upsert_nodes(...)` followed by `graph_repo.upsert_edges(...)` and return the counts.
4. On `Err(SourceExtractorError::Internal("github api: token required …"))`, return `error_code: "github_auth_required"`.
5. On `Err(SourceExtractorError::Internal("github api: rate limit exceeded …"))`, return `error_code: "github_rate_limited"`.

The helper MUST be `#[cfg(feature = "multimodal")]`-gated. The `graph_repo` argument is `Option<&Arc<dyn GraphRepository>>` — when `None`, the tool returns `error_code: "issues_ingest_unavailable"`.

#### Scenario: Missing GITHUB_TOKEN
- GIVEN no `GITHUB_TOKEN` env var
- WHEN the GitHub API returns 401
- THEN the response carries `error_code: "github_auth_required"` and message `set GITHUB_TOKEN env var (and rerun); see https://github.com/settings/tokens`

#### Scenario: No graph repository wired
- GIVEN the MCP handler was constructed without a `GraphRepository`
- WHEN `issues_ingest` is called
- THEN the response carries `error_code: "issues_ingest_unavailable"`

### Requirement: Backward Compatibility with 30 Existing Tools

Adding `issues_ingest` MUST NOT modify the input/output schema of any of the 30 existing tools. The 30 tools continue to work byte-for-byte.

#### Scenario: Existing tool list still works
- GIVEN the multimodal feature is enabled
- WHEN `tools/list` is called
- THEN the response includes all 30 existing tools + `issues_ingest` (31 total)

#### Scenario: Existing tool calls are unchanged
- GIVEN a pre-existing client calls `find_symbols` (any of the 30 tools)
- WHEN the call is made
- THEN the response is byte-for-byte identical to the pre-change behavior

## Edge Cases (ADDED)

| Edge Case | Expected Behavior |
|-----------|-------------------|
| `source` is neither a URL nor an existing path | Return `error_code: "invalid_input"` with `source must be a github.com URL or a local git repo path` |
| `mode = "both"` but the local repo has no `origin` remote | Fall back to `git_log` mode with a `tracing::warn!`; still emit commit-issue edges, but `node-id` is `issue:github/unknown/unknown#N` and the frontend flags them as unresolved |
| GitHub API returns 5xx transient error | Retry once with a 500ms backoff; on second failure, return `error_code: "github_api_error"` |
| `max_issues = 0` | Schema validation rejects with `max_issues must be >= 1` |
| `max_issues` larger than the total issue count | No truncation; `nodes_created` equals the total |
| `octocrab` rate-limit header is present but > 0 | Continue; do not warn |
| Concurrent `issues_ingest` calls | Each runs in its own task; `INSERT … ON CONFLICT` serialises on the unique constraint; final state is the union of both calls |
| A commit message has a `#N` reference but no GitHub issue with that number exists in the index | Edge is still created (the `target` node is created lazily on first sight — see `issue-node-properties` spec). The frontend shows the issue node with a "synthetic" badge |
| `source` is a GitHub Enterprise URL (e.g. `https://ghe.acme.com/...`) | Schema validation rejects with `github enterprise not supported in V1 (use https://github.com URLs only)` |

## TDD RED Gate (ADDED)

1. `tools/list` snapshot — 30 tools (feature off) and 31 tools (feature on) — the existing 30-tool snapshot test from `mcp-multimodal-tools` is updated, not duplicated
2. `issues_ingest` schema validation — 5 cases: missing source, empty source, unknown mode, max_issues = 0, GHE URL
3. `issues_ingest` happy path with mocked `GitHubClient` — 1 case: 5 issues, 1 commit, 6 nodes, 1 edge
4. `issues_ingest` error envelopes — 3 cases: `github_auth_required`, `github_rate_limited`, `issues_ingest_unavailable`
5. Re-ingest idempotency — 1 case: second call reports `nodes_created == 0`
6. Feature-gate test: `cargo run -p cognicode-mcp --no-default-features` does NOT register `issues_ingest`
7. Backward-compat regression — 1 test that calls each of the 30 existing tools and checks the response shape (reused from `mcp-multimodal-tools`)

## Dependencies

- `mcp-multimodal-tools` (the main spec; this delta is additive only)
- `docs-source-adapter` (precedent: `dispatch_docs_ingest` shape)
- `issues-extractor` (the adapter invoked by the dispatch)
- `graph-repository-write` (the persistence path; `issues_ingest` is the second caller after `docs_ingest`)
- `issue-node-properties` (the `NodeKind::Issue` schema applied to each row)

## Out of Scope

- New dispatch pattern (e.g. table-driven tool registration) — flagged for a follow-up refactor
- Streaming responses (single JSON payload, like the other 30 tools)
- Authentication beyond `GITHUB_TOKEN` (no OAuth flow)
