# Spec: EntryPointResolver Default ViewKind Mapping (Roadmap — Phase 5)

## Purpose

Define a typed `EntryPointResolver` that converts any user input
(HTTP route, CLI command, event name, use case name, symbol id,
ViewSpec id, search result, ADR / decision / doc / issue /
evidence id) into a `ResolvedEntryPoint` and selects a default
`ViewKind` for it. The default is the user's best first view but
is overridable from the Explorer. **This spec is the roadmap;
implementation is deferred to Phase 5.** Phase 0/1/3 ship without
any entry-point resolution changes.

## Domain

`entry-point-resolver` — NEW capability. No existing spec to delta
against; this is a full spec.

**Phase**: 5 (DEFERRED). Do not implement during the first safe
slice.

---

## ADDED Requirements

### Requirement: 1. `EntryPoint` enum

The system MUST define a `EntryPoint` enum that captures every
starting point the Explorer accepts (per CONTEXT.md §Entry
Points). Variants:

| Variant | Carries | Example |
|---------|---------|---------|
| `HttpRoute { method, path }` | verb + path | `POST /api/users` |
| `CliCommand { name }` | command string | `cognicode analyze` |
| `Event { name }` | event name | `UserCreated` |
| `UseCase { name }` | use case | `CreateUser` |
| `Symbol { id }` | canonical id | `symbol:src/foo.rs:bar:42` |
| `File { path }` | file path | `src/foo.rs` |
| `Scope { path }` | scope path | `src/foo` |
| `SearchResult { ids, query }` | ranked ids + original query | (search hit) |
| `SavedExploration { id }` | exploration id | `exp-…` |
| `ViewSpec { id }` | view-spec id | `vs-…` |
| `Decision { id }` | ADR / decision id | `ADR-008` |
| `Doc { id }` | doc id | `doc-…` |
| `Issue { id }` | issue id | `iss-…` |
| `Evidence { id }` | evidence id | `ev-…` |

`EntryPoint` is a string-typed input. The resolver parses the
input and returns the most specific variant or a `None` if it
cannot resolve. Parsing rules are layered: longest match wins
(e.g. `POST /api/users` → `HttpRoute`; `/api/users` alone →
`File` if the path is on disk).

#### Scenario: HTTP route resolves

- GIVEN input `"POST /api/users"`
- WHEN the resolver runs
- THEN the result is `EntryPoint::HttpRoute { method: "POST",
  path: "/api/users" }`

#### Scenario: Symbol id resolves

- GIVEN input `"symbol:src/foo.rs:bar:42"`
- WHEN the resolver runs
- THEN the result is `EntryPoint::Symbol { id: "symbol:src/foo.rs:bar:42" }`

#### Scenario: Ambiguous input returns None

- GIVEN input `"foo"` that matches no `Symbol`, no `File`, no
  `UseCase`
- WHEN the resolver runs
- THEN the result is `Err(EntryPointError::NotResolved)`

### Requirement: 2. Default ViewKind per entry point

The resolver MUST map each `EntryPoint` variant to a default
`ViewKind`. The mapping (from CONTEXT.md §Entry Points) is
authoritative:

| Entry point | Default ViewKind |
|-------------|------------------|
| `HttpRoute` | `vertical_slice` |
| `CliCommand` | `vertical_slice` |
| `Event` | `data_flow` |
| `UseCase` | `vertical_slice` |
| `Symbol` | `call_graph` |
| `File` | `overview` (built-in `file-overview` view) |
| `Scope` | `overview` (built-in `scope-overview` view) |
| `SearchResult` | `semantic_search_results` |
| `SavedExploration` | `composed_narrative` |
| `ViewSpec` | the ViewSpec's own `view_kind` |
| `Decision` | `architecture_rationale` |
| `Doc` | `doc_code_alignment` |
| `Issue` | `dead_code_candidates` (or specialised `issue_view` if available) |
| `Evidence` | `evidence_view` |

The default is a hint, not a hard rule. The Explorer can switch
views from the ViewTabs after the first render.

#### Scenario: HTTP route opens vertical slice

- GIVEN `EntryPoint::HttpRoute { method: "POST", path: "/api/users" }`
- WHEN `resolve(ep).default_view_kind()` is called
- THEN the result is `ViewKind::VerticalSlice`

#### Scenario: Symbol opens call graph

- GIVEN `EntryPoint::Symbol { id: "symbol:src/foo.rs:bar:42" }`
- WHEN `resolve(ep).default_view_kind()` is called
- THEN the result is `ViewKind::CallGraph`

### Requirement: 3. Resolver pipeline

The system MUST implement a single `resolve(input: &str) ->
Result<ResolvedEntryPoint>` that the Spotter, the URL bar, the
ADR link, and the MCP `entrypoint_resolve` tool all use. The
pipeline:

```
input string
  ↓
EntryPoint::parse(input)   ← structural parse
  ↓
object resolution (when needed: Symbol/File/Scope/Decision/...)
  ↓
ResolvedEntryPoint { ep: EntryPoint, target: ResolvedObject }
  ↓
default_view_kind()         ← per the table above
```

`ResolvedEntryPoint` carries the parsed `EntryPoint` plus the
`ResolvedObject` (the actual `InspectableObjectSummary` it
resolved to), so callers don't re-resolve.

#### Scenario: Spotter uses the resolver

- GIVEN the user types `"POST /api/users"` in the Spotter
- WHEN they press Enter
- THEN the resolver returns
  `ResolvedEntryPoint { ep: HttpRoute, target: ResolvedObject, default_view_kind: VerticalSlice }`
- AND the Explorer opens the `vertical_slice` view for the
  resolved object

#### Scenario: MCP exposes the resolver

- GIVEN an MCP client
- WHEN it calls `entrypoint_resolve { input: "UserCreated" }`
- THEN the tool returns the same `ResolvedEntryPoint` shape the
  Explorer uses, plus a `mcp` flag that tells the caller
  "browser-only renderers will not work — show JSON"

### Requirement: 4. Search results are not a flat list

`EntryPoint::SearchResult` MUST NOT resolve to a single object.
Instead, the resolver returns
`ResolvedEntryPoint::SearchResults { items, default_view_kind:
SemanticSearchResults }`. The Explorer then renders the search
hit set as a moldable `semantic_search_results` view, not as a
flat list. The user can save the result set as a `ViewSpec` from
inside the view.

#### Scenario: Search opens a moldable view

- GIVEN a Spotter query `"create_user"` returning 4 hits
- WHEN the user submits
- THEN the Explorer opens the `semantic_search_results` view
  showing the 4 hits as filterable, groupable rows
- AND a "Save as ViewSpec" action is visible in the toolbar

#### Scenario: Save-as ViewSpec persists

- GIVEN the user is in the `semantic_search_results` view
- WHEN they click "Save as ViewSpec"
- THEN a new `ViewSpec` is created with
  `view_kind = SemanticSearchResults`,
  `data_source.query = the original Spotter query`,
  `renderer_kind = Composite`, and the user's title

## Out of Scope (Phase 5 — explicit non-requirements)

- Auto-detecting the entry-point kind from a free-form natural
  language query (LLM-based classification) — out of v1
- A separate `EntryPointResolver` UI surface (the Spotter is the
  only entry point in v1)
- Multi-step entry points (chain of entry points that build on
  each other) — out of v1
- Sharing a `ResolvedEntryPoint` URL across users — Phase 6+ if
  needed

## Coverage

- **Happy paths**: covered (parse each variant, default
  ViewKind per table, pipeline stages, search is a moldable
  view)
- **Edge cases**: covered (ambiguous input → NotResolved, search
  result is not flattened, MCP degrades renderer)
- **Error states**: covered (resolver errors don't crash Spotter;
  Explorer falls back to a "no view available" pane)
