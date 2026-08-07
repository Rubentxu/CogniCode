# Delta for ask-router

## MODIFIED Requirements

### Requirement: Internal Dispatch (No MCP Chaining)

The router MUST call `ExplorerService` and `CallGraph` methods directly using the same `Arc<ExplorerService>` and `Arc<CallGraph>` held by `ExplorerMcpHandler`. The router MUST NOT call the 17 MCP tools through the MCP protocol (no re-serialization, no recursive `tools/call`). The dispatch layer MUST be a thin async function that invokes service methods and merges their `serde_json::Value` outputs. When invoked from a brain session (`brain_ask`), `dispatch_ask` MUST accept an optional `&BrainSessionService` parameter; when the service is `Some`, the dispatcher MUST consume the session's focus node (already prepended into the enriched `question` by the caller) and MUST NOT add any further entity prefix. The `McpResultEnvelope.provenance.source` for the inner ask envelope MUST remain `"ask-router"` regardless of whether the call came from a brain session — the brain-session layer wraps the outer envelope.
(Previously: dispatch was a thin async function over `(classified, service, graph)`; no session awareness, no focus node.)

#### Scenario: Dispatch signature accepts optional session

- GIVEN `dispatch_ask` is called from a `brain_ask` handler
- WHEN the signature is inspected at compile time
- THEN it MUST accept `session: Option<&BrainSessionService>` (or equivalent `&Option<Arc<BrainSessionService>>`)
- AND it MUST compile and run correctly when `session` is `None` (one-shot `cognicode_ask` path unchanged)

#### Scenario: Inner envelope preserves ask-router provenance

- GIVEN a session with `focus_node = Some("Foo")` calls `brain_ask`
- WHEN the inner ask-router envelope is inspected
- THEN `provenance.source` MUST equal `"ask-router"` (not `"brain-session"`)
- AND the outer brain-session wrapper envelope MUST carry `provenance.source = "brain-session"`

#### Scenario: Dispatch does not double-inject focus node

- GIVEN a session with `focus_node = Some("AuthService")`
- AND the caller has already rewritten the question to `"what does `AuthService` call?"`
- WHEN `dispatch_ask` is called with `session = Some(...)`
- THEN the dispatcher MUST NOT prepend another `AuthService` token
- AND the ask-router MUST see exactly one `AuthService` backtick token in the question

#### Scenario: One-shot cognicode_ask is unchanged

- GIVEN `cognicode_ask` is called directly (no session)
- WHEN `dispatch_ask(classified, service, graph)` is invoked
- THEN the behavior MUST be identical to the pre-change contract
- AND every existing 18-tool test MUST still pass

### Requirement: AskArgs Schema

`AskArgs` for `cognicode_ask` MUST keep `question: string` (required) and `context: object` (optional). The `context` field MUST be tolerant of arbitrary JSON (not strictly typed) and is reserved for future routing hints. Direct calls to `cognicode_ask` (not through `brain_ask`) MUST treat `context` as opaque and ignored. The wire schema MUST NOT change in this change — only the implementation behind it gains session awareness.
(Previously: `context` was "reserved for future use"; same description still applies at the wire level.)

#### Scenario: AskArgs schema is unchanged

- GIVEN the MCP server is started
- WHEN a client calls `tools/list` and inspects the `cognicode_ask` schema
- THEN the schema MUST still declare `question: string` (required) and `context: object` (optional)
- AND no new required field MUST be added

#### Scenario: context is ignored on one-shot path

- GIVEN `cognicode_ask` is called with `context = {"session_id": "S", "focus": "Foo"}`
- WHEN the dispatch runs
- THEN the router MUST NOT look up session `S` or apply any focus
- AND the response MUST be the standard ask-router envelope with no session wrapping
