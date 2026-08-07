# Spotter Search

## Purpose

Universal discovery surface for CogniCode Explorer. Returns ranked hits across eleven families — **Symbol**, **File**, **ViewSpec**, **SavedExploration**, **QualityIssue**, **Rule**, **Investigation**, **Scope**, **Doc**, **Decision**, **Evidence** — sourced from already-persisted data. Wave 1: no schema migrations. Narratives (Wave 3) are out of scope.

## Requirements

### Requirement: Multi-family ranked response

The Spotter MUST return a discriminated union of hits tagged by `kind` (`symbol`, `file`, `viewspec`, `saved_exploration`, `quality_issue`, `rule`, `investigation`, `scope`). Hits within each family MUST be ranked by score descending. Each hit MUST expose the family kind so the frontend can render a distinguishing glyph.

#### Scenario: Single query returns hits from every matching family

- GIVEN a workspace with symbols, files, ViewSpecs, saved explorations, quality issues, rules, investigations, and scopes
- WHEN the user submits a query that matches at least one item in every family
- THEN the response contains at least one hit of each `kind`
- AND every hit carries its `kind` tag

### Requirement: Symbol and File hits

The Spotter MUST surface Symbol hits for symbols matching the query and MUST derive a File hit from each matching symbol's `file_path`. File hits MUST be deduplicated by path so each distinct path appears at most once.

#### Scenario: Symbol hit identifies kind, file, and line

- GIVEN a symbol `create_user` at `src/users.rs:42`
- WHEN the user queries `create_user`
- THEN a symbol hit appears whose subtitle identifies the kind, file, and line

#### Scenario: Same file from many symbols collapses to one File hit

- GIVEN 3 matching symbols all in `src/handlers.rs`
- WHEN the query matches all three
- THEN the response contains at most one `file` hit for `src/handlers.rs`

### Requirement: ViewSpec hits

The Spotter MUST surface ViewSpec hits when the query matches a stored ViewSpec's title or view kind. ViewSpec hits MUST only appear when a workspace context is supplied.

#### Scenario: Title match surfaces a ViewSpec summary

- GIVEN a stored ViewSpec titled `"call graph hotspots"`
- WHEN the user queries `call graph`
- THEN a `viewspec` hit appears with that title

#### Scenario: Missing workspace context yields no ViewSpec hits

- GIVEN the Spotter request omits `workspace_id`
- WHEN the query runs
- THEN the response contains no `viewspec` hits and no error

### Requirement: SavedExploration hits

The Spotter MUST surface SavedExploration hits when the query matches a persisted exploration's title or description. The hit MUST expose enough metadata to reopen the exploration.

#### Scenario: Title match surfaces a SavedExploration hit

- GIVEN a persisted exploration titled `"auth flow exploration"`
- WHEN the user queries `auth`
- THEN a `saved_exploration` hit appears with that title

### Requirement: QualityIssue and Rule hits

The Spotter MUST surface QualityIssue hits when the query matches an issue's message or rule id. The Spotter MUST surface Rule hits when the query matches a rule id or description.

#### Scenario: Issue message surfaces a QualityIssue hit

- GIVEN an open issue with message `"memory leak in cache eviction"`
- WHEN the user queries `memory leak`
- THEN a `quality_issue` hit appears identifying that issue

#### Scenario: Rule id surfaces a Rule hit

- GIVEN a registered rule `"rust:S100"`
- WHEN the user queries `S100`
- THEN a `rule` hit appears for that rule

### Requirement: Investigation hits

The Spotter MUST surface Investigation hits when the query matches a persisted investigation's title. The hit MUST expose the investigation's evidence count as its subtitle.

#### Scenario: Title match surfaces an Investigation hit

- GIVEN a persisted investigation titled `"auth flow analysis"`
- WHEN the user queries `auth`
- THEN an `investigation` hit appears with that title
- AND the subtitle shows the evidence count (e.g., `3 evidence items`)

#### Scenario: Investigation subtitle uses evidence count

- GIVEN an investigation `I` with `evidence.len() = 5`
- WHEN the user queries a term matching `I`'s title
- THEN the hit subtitle reads `"5 evidence items"`

### Requirement: Scope hits

The Spotter MUST surface Scope hits when the query matches a symbol's file path. Hits MUST be grouped by parent directory; each group represents a scope bucket. Root-level files (files with no parent directory) are excluded.

#### Scenario: File path match surfaces a Scope hit

- GIVEN a symbol in `src/users/service.rs` whose path matches the query
- WHEN the user queries `service`
- THEN a `scope` hit appears for `src/users/` (the parent directory)
- AND the hit subtitle shows the matching file count (e.g., `2 symbols in 1 file`)

#### Scenario: Scope groups by parent directory

- GIVEN 3 matching symbols under `src/auth/` and 2 under `src/users/`
- WHEN the user submits a query matching all 5 symbols
- THEN at most one `scope` hit per unique parent directory appears
- AND no root-level file matches are returned as scope hits

### Requirement: Result limits

The Spotter MUST cap the total response at 20 hits by default. Each family MUST have its own per-family cap so that one family cannot crowd out the others.

#### Scenario: Default cap returns at most 20 hits

- GIVEN a query matching 50 symbols, 30 ViewSpecs, 20 rules, 15 investigations, and 10 scopes
- WHEN the response is assembled
- THEN the total number of hits is `≤ 20`

#### Scenario: Per-family caps prevent single-family dominance

- GIVEN a query matching 100 symbols and 3 rules
- WHEN the response is assembled
- THEN no single family exhausts the entire 20-hit budget

### Requirement: Deduplication across families

The same underlying object MUST NOT appear as two hits, even when it matches in two families. Deduplication MUST be by stable object identity, not by display label.

#### Scenario: Same ViewSpec referenced from a SavedExploration appears once

- GIVEN a SavedExploration whose notes embed a reference to ViewSpec `V1`
- WHEN the query matches both
- THEN the response contains exactly one hit for `V1`

### Requirement: Empty state and empty query

The Spotter MUST return an empty hit list when the query yields no matches (not an error); the frontend MUST render an empty state referencing the original query. When the query is empty, the Spotter MUST NOT call the backend; the frontend SHOULD show recent or popular items instead.

#### Scenario: No matches returns empty list with rendered empty state

- GIVEN no indexed objects match `zzzqqqxxx`
- WHEN the user submits that query
- THEN the response is an empty list
- AND the frontend renders a message referencing `zzzqqqxxx`

#### Scenario: Empty query does not call the backend

- GIVEN the Spotter is open and the input is empty
- WHEN the user has not typed
- THEN no Spotter request is sent
- AND the UI shows recent or popular entries

### Requirement: Frontend useSpotter wiring

The `useSpotter` hook MUST call the multi-family endpoint, not the legacy symbol-only endpoint. Each rendered hit MUST display a glyph that identifies its family kind. ViewSpec hits MUST NOT be flattened or dropped; consumers switch on `kind` to extract family-specific fields.

#### Scenario: useSpotter targets the multi-family endpoint

- GIVEN the hook is invoked with a non-empty query
- WHEN the request is made
- THEN it targets the multi-family endpoint (Symbol + ViewSpec + File + SavedExploration + QualityIssue + Rule + Investigation + Scope)

#### Scenario: ViewSpec hits are preserved and accessible

- GIVEN a `viewspec` hit with `ViewSpecSummary { id, title, view_kind }`
- WHEN the hit is rendered
- THEN the consumer accesses `kind === 'viewspec'` fields via typed accessor
- AND the ViewSpec summary is fully accessible (not flattened to a generic string)

#### Scenario: route variant is rejected from schema

- GIVEN the frontend `spotterSearchResultSchema` Zod schema
- WHEN the schema is used to validate a response
- THEN no `route` variant is accepted
- AND any response containing a `route` kind fails validation

#### Scenario: Click on a SavedExploration hit opens the inspector

- GIVEN a `saved_exploration` hit for exploration `E`
- WHEN the user clicks it
- THEN the inspector opens the persisted state of `E`

### Requirement: SavedExploration inspect branch

The object-inspection pipeline MUST accept a SavedExploration identity and load the persisted state for it. When the persistence layer fails, the inspector MUST return a recoverable error, not panic.

#### Scenario: Inspecting a SavedExploration resolves through the dedicated branch

- GIVEN a persisted exploration `E`
- WHEN the inspector receives an object id referring to `E`
- THEN it resolves to a summary sourced from the persistence layer

#### Scenario: Persistence failure returns a recoverable error

- GIVEN the persistence layer cannot load exploration `E_missing`
- WHEN the inspector receives an id referring to it
- THEN the response is a recoverable error
- AND no SavedExploration hits from the failing call appear in Spotter

---

## ADDED Requirements — Graph Families (Wave 2)

### Requirement: Three New Spotter Result Variants

The system MUST expose `Doc`, `Decision`, and `Evidence` variants of `SpotterSearchResult`, each carrying the standard affordance pattern (icon, subtitle, available_views, click-to-inspect) shared by the existing eight families. Each hit MUST show its `title`, a kind badge, and `file_path`.

#### Scenario: Doc family renders a hit

- GIVEN the Spotter is open with keyboard focus
- WHEN the user types a query matching a Doc node
- THEN a Doc hit appears in the dropdown
- AND the hit shows the node's `title`, a kind badge `"doc"`, and `file_path`

#### Scenario: Decision family renders a hit

- GIVEN an ADR node is in `graph_nodes`
- WHEN the user searches for it
- THEN a Decision hit appears with title, kind badge `"decision"`, and `file_path`

#### Scenario: Evidence family renders a hit

- GIVEN an evidence node (benchmark or fuzzer finding) is in `graph_nodes`
- WHEN the user searches for it
- THEN an Evidence hit appears with title, kind badge `"evidence"`, and `file_path`

#### Scenario: Activating a hit opens the inspector

- GIVEN a Doc, Decision, or Evidence hit is visible
- WHEN the user clicks it or presses Enter
- THEN the inspector opens with the resolved object as the focus
- AND the default view matches the kind

### Requirement: Inspectable Object Type Coverage

The system MUST classify Doc hits as `InspectableObjectType::Doc`. Decision hits MUST reuse `DecisionArtifact`; Evidence hits MUST reuse `Evidence`. The wire surface MUST remain backward compatible with existing families.

#### Scenario: Doc is inspectable end-to-end

- GIVEN a Doc hit is selected
- WHEN the inspector loads
- THEN `InspectableObjectType::Doc` is used as `applies_to`
- AND the view registry returns at least one applicable view

### Requirement: Graceful Degradation Without Graph Repository

When `SearchServiceImpl` is constructed without a `GraphRepository` (port not wired, or non-multimodal build), the system MUST return empty results for the three new families rather than error. The other eight families MUST behave normally.

#### Scenario: Missing graph repo yields empty families

- GIVEN `SearchServiceImpl` is built with `graph_repo = None`
- WHEN the user searches for a doc-like query
- THEN the Doc, Decision, and Evidence families are empty
- AND the other 8 families behave normally

#### Scenario: Non-multimodal build degrades cleanly

- GIVEN the crate is compiled without the `multimodal` feature
- WHEN the Spotter is opened
- THEN only the 8 pre-existing families are offered
- AND no runtime panic occurs
