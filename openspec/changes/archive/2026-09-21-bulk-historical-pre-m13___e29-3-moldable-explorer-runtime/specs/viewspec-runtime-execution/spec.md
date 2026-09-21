# Spec: ViewSpec Runtime Execution

## Purpose

Wire `PostgresViewSpecStore` through persistence → search → **runtime registration / applicability discovery** → execute → render. Saved MoldQL lowers through `MoldqlAstLowerer` to `MoldPlan`. `MoldPlan::Graph` is pinned and dispatched through `GraphExecutor`; object selection, quality, lens, and view execution use their corresponding typed read-only operations. E29.2 projections then materialise a `ContextualView` carrying `renderer_kind`. This replaces the `execute_view_spec not implemented` stub. `runtime_renderer_dispatch` defaults `on`; setting it `off` is a temporary, explicit emergency rollback action. The normal dispatch path is `renderer_kind` → `rendererRegistry`; an unknown renderer yields `UnsupportedRendererState`, never silent JSON.

> **Domain status:** `viewspec-runtime-execution` is a **new capability**. The premature canonical copy at `openspec/specs/viewspec-runtime-execution/spec.md` is **deleted** in this change; the capability is introduced as a new spec inside the change folder, including runtime registration / applicability discovery requirements absent from the premature copy.

## ADDED Requirements

### Requirement: Execute pipeline pinned to `RevisionId`

The system MUST execute a `ViewSpec` against a specific `RevisionId`. The handler MUST accept `revision_id` and thread it through `MoldqlAstLowerer` into `MoldPlan` dispatch. `MoldPlan::Graph` MUST execute through `GraphExecutor`; other supported variants MUST use their typed read-only operation rather than being coerced into `GraphPlan`. Unsupported variants MUST return 422 (`error = "view_spec_execution_unsupported"`). A missing revision MUST return 404 (`error = "revision_unknown"`); a missing spec MUST return 404 (`error = "view_spec_not_found"`). Re-execution with the same `(spec_id, revision_id)` MUST return a content-equal `ContextualView`. The route MUST NOT return `FeatureDisabled`, and **no changes to `crates/cognicode-core/src/domain/plan/graph_plan.rs` are required**.

#### Scenario: Happy-path execute

- GIVEN a saved spec `S` with `data_source = Moldql { ... }` and `RevisionId R`
- WHEN `POST /api/viewspecs/S/execute { revision_id: R, target: T }` runs
- THEN response is 200 with `ContextualView { blocks, renderer_kind, revision_id: R }`

#### Scenario: Determinism across calls

- GIVEN call C1 against `R` succeeded
- WHEN C2 runs against the same `R`
- THEN blocks and `renderer_kind` of C2 equal C1 byte-for-byte

### Requirement: Runtime consumes the ViewSpec REST API contract

Runtime execution MUST load, search, save, update, and delete ViewSpecs through
the `viewspec-rest-api` capability. It MUST NOT define a second CRUD contract or
bypass `PostgresViewSpecStore` with direct SQL.

#### Scenario: Runtime loads through the owning capability

- GIVEN spec `S` was created through `viewspec-rest-api`
- WHEN runtime execution loads `S`
- THEN it uses the same owner-scoped `PostgresViewSpecStore` contract
- AND no duplicate CRUD handler or direct SQL path is invoked

### Requirement: Save → search → load → execute round-trip

The system MUST permit, without manual SQL or seed data: `POST /api/viewspecs` → 201 with id `S`; `GET /api/viewspecs?q=<title>` contains `S`; `GET /api/viewspecs/S` returns the saved body; `POST /api/viewspecs/S/execute` returns a `ContextualView`. An integration test MUST cover the round-trip.

#### Scenario: Full round-trip succeeds

- GIVEN a clean `view_specs` table and one `RevisionId R`
- WHEN steps 1–4 run in sequence
- THEN each step returns the expected status and the loaded body equals the saved body

### Requirement: Persisted transform matches authoring preview

When a ViewSpec contains a JSONata `transform`, the Explorer runtime MUST apply
the persisted expression to `TransformInputV1`, defined as the JSON
serialization of `ContextualView.projection.payload` and excluding capability
status, provenance, confidence, warnings, and truncation from the transformable
input. Authoring preview and runtime MUST use the same schema version and
sandboxed frontend evaluator. The transformed value replaces only the payload;
the projection envelope remains authoritative and unchanged. A transform error
MUST produce an explicit pane error and MUST NOT fall back to the untransformed
payload. Backend/Rust JSONata execution remains out of scope.

#### Scenario: Saved transform produces preview-equivalent output

- GIVEN authoring preview applied transform `items[fan_out > 5]` to `TransformInputV1` and produced output `P`
- WHEN the saved ViewSpec executes against the same revision and input
- THEN the runtime transform output equals `P`
- AND the renderer receives `P`, not the untransformed result

#### Scenario: Transform failure is explicit

- GIVEN a saved ViewSpec contains an invalid JSONata expression
- WHEN runtime rendering applies the transform
- THEN `PaneExecutionState` is `error` with code `viewspec_transform_invalid`
- AND no renderer receives the untransformed payload

### Requirement: Discriminated execution and pane status

The execute response MUST carry a `status` field with variants `empty`, `truncated`, `error`, and `ready`. `truncated` MUST include `truncation_reason`; `error` MUST NOT expose stack traces. The frontend `PaneExecutionState` additionally owns `loading` while a request is in flight and `revision_change` when the pane's retained `previous_revision_id` differs from the latest workspace `current_revision_id`. The execute endpoint MUST NOT invent the previous pin or require two revisions in one request. Every state MUST render distinctly with no silent collapse.

#### Scenario: Empty result surfaces state

- GIVEN a MoldQL query matching zero rows
- WHEN execute runs
- THEN status is `empty` and `blocks = []`

#### Scenario: Pane detects revision change from client context

- GIVEN the pane retains `previous_revision_id = R1`
- WHEN workspace-head polling reports `current_revision_id = R2`
- THEN frontend `PaneExecutionState` is `revision_change { previous_revision_id: R1, current_revision_id: R2 }`
- AND the execute response itself remains pinned to the one revision requested

### Requirement: Runtime registration of saved ViewSpecs

The runtime MUST register every saved `ViewSpec` loaded by the explorer with the in-process view-spec registry so it is discoverable alongside built-in views. Registration MUST happen on save (`POST /api/viewspecs`), on update (`PUT /api/viewspecs/:id`), and on search-result hydration (`GET /api/viewspecs`). Unregister MUST happen on delete (`DELETE /api/viewspecs/:id`). The registry MUST expose `list_for(object_type)` and `lookup(id)` returning the registered `ViewSpec` (or `None`). Registration MUST be idempotent: re-registering the same `id` replaces the prior entry and logs a `tracing::warn!`. Registration MUST NOT silently swallow errors; a malformed persisted spec (failed schema validation on read) MUST surface a `tracing::error!` and MUST NOT be added to the registry.

#### Scenario: Save registers the spec

- GIVEN the runtime view-spec registry is empty
- WHEN `POST /api/viewspecs` returns 201 with id `S`
- THEN `registry.list_for(<symbol>)` includes `S`'s descriptor
- AND `registry.lookup(S)` returns `Some(S)`

#### Scenario: Delete unregisters the spec

- GIVEN spec `S` is registered
- WHEN `DELETE /api/viewspecs/S` returns 200
- THEN `registry.list_for(<symbol>)` no longer includes `S`
- AND `registry.lookup(S)` returns `None`

#### Scenario: Malformed persisted spec is rejected

- GIVEN the `view_specs` table contains a row whose `data_source` JSON does not match `DataSource` enum
- WHEN the runtime loads it on startup
- THEN a `tracing::error!` is emitted naming the row id
- AND the registry does not contain that row
- AND the next `execute_view_spec` call for that id returns 404 (`error = "view_spec_invalid"`), not a panic

### Requirement: Applicability discovery

The runtime MUST answer `GET /api/viewspecs/discover?applies_to=<InspectableObjectType>` returning `Vec<ViewSpecSummary>` (id, title, view_kind, renderer_kind, applies_to, updated_at) restricted to specs the calling `owner` registered AND whose `applies_to` matches the requested type. The response is the canonical input for the Explorer's "Available views" surface and the `ViewRegistry::list_for(object.type)` lookup. The endpoint MUST respect `owner` scoping; cross-owner reads MUST return 404 (no existence leak). The endpoint MUST NOT include `props` or `data_source` in the summary payload.

#### Scenario: Symbol discover lists matching specs

- GIVEN two specs owned by `alice` — one with `applies_to = Symbol`, one with `applies_to = File`
- WHEN `GET /api/viewspecs/discover?applies_to=Symbol&owner=alice` runs
- THEN the response lists only the `Symbol`-applies spec
- AND omits the `File`-applies spec

#### Scenario: Cross-owner discover returns empty

- GIVEN spec `S` owned by `alice`
- WHEN `GET /api/viewspecs/discover?applies_to=Symbol&owner=bob` runs
- THEN the response is `[]` (no existence leak)

#### Scenario: Discover integrates with ViewRegistry

- GIVEN a saved spec `S` with `applies_to = Symbol` and a built-in registry exposing 4 views for `Symbol`
- WHEN `PaneInspector` calls `ViewRegistry::list_for(Symbol)`
- THEN the response is the 4 built-ins plus `S`
- AND `S` is rendered using `S.renderer_kind` dispatch (the normal path)

### Requirement: Feature flag `runtime_renderer_dispatch` — emergency rollback only

The system MUST expose the flag `runtime_renderer_dispatch` with default `true`. When `true`, `renderer_kind` drives dispatch via `rendererRegistry` and is the normal path. Setting it `false` is a **temporary, emergency rollback action** that mounts the legacy `isGraphViewKind` path and emits a warning. The flag value MUST be logged at startup. Flipping it at runtime MUST NOT require a restart or crash an open pane. Its value MUST be visible via `GET /api/diagnostics/runtime_flags`.

#### Scenario: Default uses renderer_kind path

- GIVEN the binary started with the default configuration
- WHEN a graph-shaped ViewSpec opens
- THEN `runtime_renderer_dispatch = true`
- AND `rendererRegistry.get(view.renderer_kind)` renders the view

#### Scenario: Flag on routes to normal renderer_kind path

- GIVEN the flag is `true` and a pane is open
- WHEN a new pane opens for a `ContextualView` with `renderer_kind = "graph"`
- THEN the next render uses `rendererRegistry.get("graph")` (the normal path)
- AND no `isGraphViewKind` fallback runs

#### Scenario: Diagnostics endpoint reports flag

- GIVEN the binary started with `runtime_renderer_dispatch = true`
- WHEN `GET /api/diagnostics/runtime_flags` runs
- THEN the response is `{ "runtime_renderer_dispatch": true }`

## Out of Scope

- JSONata execution in Rust; the persisted transform executes in the sandboxed frontend runtime
- Remote / plugin renderers (out of v1)
- MoldQL Pattern Profile v1 (E28.3)
- Cross-workspace sharing, versioning, history

## Coverage

- **Happy paths**: covered (execute, CRUD, round-trip, runtime registration, applicability discovery, flag on/off)
- **Edge cases**: covered (unknown revision, idempotent delete, empty/client-owned revision_change, malformed persisted spec)
- **Error states**: covered (404, 422, 503, discriminated `status`, `FeatureDisabled` removed, no silent JSON fallback)
