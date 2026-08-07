# Spec: ViewSpec REST API

## Purpose

This spec adds full CRUD + execute for ViewSpecs, backed by `PostgresViewSpecStore`. The previous `execute_view_spec` stub is replaced by a real handler that threads `RevisionId` through `MoldqlAstLowerer`, `MoldPlan` dispatch, and the applicable typed executor. All routes live under `/api/viewspecs` and follow the Explorer API's existing Problem Details shape.

> **Domain status:** `viewspec-rest-api` is a **new capability**. It does not reuse the existing `explorer` namespace, which already contains the `diagram-snapshot-export` capability.

## ADDED Requirements

### Requirement: `POST /api/viewspecs` — create

The system MUST expose `POST /api/viewspecs` that accepts a `ViewSpec` JSON body, validates it via `ViewSpec::validate()`, persists it via `PostgresViewSpecStore::insert`, and returns 201 with the saved body (server-assigned `id`, `created_at`, `updated_at`). Validation failure MUST return 400 (`error = "view_spec_invalid"`); unique violation on `(owner, title)` MUST return 409; `postgres` feature off MUST return 503 (`error = "feature_disabled"`).

#### Scenario: Happy-path create returns 201

- GIVEN a valid ViewSpec body
- WHEN `POST /api/viewspecs` is called
- THEN response is 201 with server-assigned `id`, `created_at`, `updated_at`
- AND `GET /api/viewspecs/:id` returns the same row

#### Scenario: Empty title returns 400

- GIVEN a body with `title = ""`
- WHEN `POST /api/viewspecs` is called
- THEN response is 400 with `error = "view_spec_invalid"`

### Requirement: `GET /api/viewspecs/:id` — read

The system MUST expose `GET /api/viewspecs/:id` that returns the saved `ViewSpec` or 404 (`error = "view_spec_not_found"`). The handler MUST require `owner` (query param) and scope the lookup to `(owner, id)`; owner mismatch MUST return 404 (no existence leak).

#### Scenario: Read returns the saved body

- GIVEN a saved ViewSpec `S` owned by `alice`
- WHEN `GET /api/viewspecs/S?owner=alice` is called
- THEN response is 200 with the saved body

#### Scenario: Owner mismatch returns 404

- GIVEN the same row owned by `alice`
- WHEN `GET /api/viewspecs/S?owner=bob` is called
- THEN response is 404 with `error = "view_spec_not_found"`

### Requirement: `GET /api/viewspecs` — list and search

The system MUST expose `GET /api/viewspecs?q=&owner=&limit=&offset=` returning `Vec<ViewSpecSummary>` (id, title, view_kind, renderer_kind, updated_at) for the calling `owner`, with `q` matching `title` case-insensitively as a substring. Pagination MUST default to `limit = 50, offset = 0`. The response MUST NOT include `props` or `data_source`.

#### Scenario: Search filters by substring

- GIVEN three specs owned by `alice` titled `["Hot symbols", "Cold modules", "Hot callers"]`
- WHEN `GET /api/viewspecs?q=Hot&owner=alice` is called
- THEN the response lists the two `Hot*` specs and omits `Cold modules`

#### Scenario: Pagination caps the page

- GIVEN 120 specs owned by `alice`
- WHEN `GET /api/viewspecs?owner=alice&limit=50&offset=0` is called
- THEN the response has at most 50 entries and `next_offset` is populated

### Requirement: `PUT /api/viewspecs/:id` — update

The system MUST expose `PUT /api/viewspecs/:id` that updates the mutable fields (`title`, `applies_to`, `view_kind`, `data_source`, `transform`, `renderer_kind`, `props`) and bumps `updated_at`. The handler MUST require `owner` scoping. Validation errors MUST return 400; unknown id / owner mismatch MUST return 404.

#### Scenario: Update bumps updated_at

- GIVEN a saved ViewSpec `S` with `updated_at = T1`
- WHEN `PUT /api/viewspecs/S?owner=alice` is called with `title = "Hot v2"`
- THEN response is 200 with `updated_at > T1` and `created_at` unchanged

### Requirement: `DELETE /api/viewspecs/:id` — delete

The system MUST expose `DELETE /api/viewspecs/:id` that removes the row scoped by `(owner, id)`. The handler MUST be idempotent: a missing id OR owner mismatch returns 200 with `{ deleted: false }` (no 404 leak).

#### Scenario: Delete removes the row

- GIVEN a saved ViewSpec `S` owned by `alice`
- WHEN `DELETE /api/viewspecs/S?owner=alice` is called
- THEN response is 200 with `{ deleted: true }`; `GET /api/viewspecs/S` returns 404

### Requirement: `POST /api/viewspecs/:id/execute` — execute pipeline

The system MUST expose `POST /api/viewspecs/:id/execute` that loads the spec from `PostgresViewSpecStore`, lowers MoldQL through `MoldqlAstLowerer` into `MoldPlan`, and dispatches the applicable typed read-only operation. `MoldPlan::Graph` MUST execute via `GraphExecutor` against the supplied `revision_id`; other supported variants MUST NOT be coerced into `GraphPlan`. The handler applies E29.2 projections and returns a `ContextualView` carrying `status` (`empty` | `truncated` | `error` | `ready`). `ContextualView.projection.payload` MUST serialize as `TransformInputV1`, the single canonical JSONata input used by authoring and runtime; the projection envelope is not part of that input. The handler MUST return 200 on success, 404 on unknown spec / revision, 422 on unsupported data source or plan variant, and MUST NOT return `FeatureDisabled` once this change lands.

#### Scenario: Execute returns real ContextualView

- GIVEN a saved ViewSpec `S` with `data_source = Moldql { ... }` and a known `RevisionId R`
- WHEN `POST /api/viewspecs/S/execute { revision_id: R, target: T }` is called
- THEN response is 200 with `ContextualView { blocks, renderer_kind, status, revision_id: R }`

#### Scenario: Unknown revision returns 404

- GIVEN a missing `RevisionId R_missing`
- WHEN execute is called with `revision_id = R_missing`
- THEN response is 404 with `error = "revision_unknown"`

#### Scenario: Non-graph MoldPlan is not coerced

- GIVEN a saved ViewSpec lowers to a supported `MoldPlan::ObjectSelection`
- WHEN execute runs against revision `R`
- THEN the object-selection executor handles the typed operation
- AND no synthetic `GraphPlan` is created
