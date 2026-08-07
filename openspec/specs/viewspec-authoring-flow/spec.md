# Spec: ViewSpec Authoring Flow (Roadmap — Phase 4)

## Purpose

Define the Explorer-first authoring UX for runtime `ViewSpec`s.
The authoring flow is a wizard that lets a user pick a
`ViewKind`, choose a `RendererKind`, configure a `MoldQL` data
source, adjust a `JSONata` transform, preview the result, and
save the `ViewSpec` to the runtime store. **This spec is the
roadmap; implementation is deferred to Phase 4.** Phase 0/1/3
ship without any authoring UI; users consume built-in views and
runtime `ViewSpec`s that arrive via the API.

## Domain

`viewspec-authoring-flow` — NEW capability. No existing spec to
delta against; this is a full spec.

**Phase**: 4 (DEFERRED). Do not implement during the first safe
slice (Phases 0, 1, 3). The spec exists so the design and tasks
phases have a complete contract when Phase 4 is unlocked.

---

## ADDED Requirements

### Requirement: 1. Entry point: "Create custom view" action

The `ObjectInspector` MUST expose a **Create custom view** action
in the view-tabs overflow menu. The action is enabled only when
the focused object has at least one `ViewKind` registered for its
`applies_to` type.

#### Scenario: Action visible for symbol

- GIVEN the user is inspecting a `Symbol` object with 4 built-in
  views
- WHEN the view-tabs overflow menu opens
- THEN "Create custom view" is in the menu and is enabled
- AND it is also visible on `File` and `Scope` objects

#### Scenario: Action hidden for unsupported types

- GIVEN the user is inspecting an object of type `Rule` (no
  authoring surface in v1)
- WHEN the view-tabs overflow menu opens
- THEN "Create custom view" is absent or disabled

### Requirement: 2. Wizard structure

The authoring flow MUST be a 5-step wizard rendered in a modal
drawer:

1. **Pick ViewKind** — searchable list of all values from
   `ViewRegistry.known_view_kinds()`, grouped by family (Core,
   C4, Quality, Architecture, Development, Living doc).
2. **Pick RendererKind** — searchable list, defaulting to the
   `default_renderer_for(view_kind)` mapping from
   CONTEXT.md §ViewSpec.
3. **Configure data source** — a `MoldQL` text editor with
   autocomplete from the catalog of available objects
   (`symbols`, `docs`, `evidence`, `issues`, `rules`,
   `decisions`).
4. **Adjust transform** — a `JSONata` expression editor with
   live preview of the input JSON (the `MoldQL` result) and
   output JSON. Errors are shown inline.
5. **Save** — a summary screen with title, applies_to
   (inferred from the focused object), and a Save button. Save
   calls `POST /api/viewspecs` and closes the drawer.

#### Scenario: Wizard enforces step ordering

- GIVEN the user opens the wizard
- WHEN they click "Next" on step 1 without picking a ViewKind
- THEN the Next button is disabled; no skip-to-step-3

#### Scenario: Live JSONata preview

- GIVEN step 4 with a `JSONata` expression `items[fan_out > 5]`
  and a `MoldQL` result with 12 items
- WHEN the user types the expression
- THEN a side panel shows the filtered items in real time
- AND any expression error is rendered in red with the parser
  message

#### Scenario: Save posts to backend

- GIVEN the user reaches step 5 with a valid spec
- WHEN they click Save
- THEN `POST /api/viewspecs` is called with the spec body
- AND the drawer closes
- AND the new view appears in the available-views list within
  5 seconds (via SWR revalidation)

### Requirement: 3. Edit existing ViewSpec

The authoring flow MUST also support editing a saved runtime
ViewSpec. The "Edit" action appears in the view-tabs overflow
menu next to the view's title. The wizard opens pre-populated
with the saved spec's values; Save calls `PUT /api/viewspecs/:id`.

#### Scenario: Edit pre-fills all fields

- GIVEN a saved `ViewSpec` with id `V` and title "Hot Symbols"
- WHEN the user picks "Edit" on that view
- THEN the wizard opens with title="Hot Symbols",
  view_kind=QualityHotspots, data_source already populated, and
  the renderer_kind pre-selected

#### Scenario: Edit requires ownership check

- GIVEN a `ViewSpec` owned by a different `owner` than the
  current user
- WHEN the user views that spec in the Explorer
- THEN the "Edit" action is hidden; only the workspace owner
  sees it (MCP `view_update` may differ — see ADR-008)

### Requirement: 4. JSONata sandbox

`JSONata` expressions MUST execute in a sandboxed worker (Web
Worker) with:

- 100 ms evaluation budget per expression
- 1 MB input size cap
- No access to `globalThis`, `fetch`, `XMLHttpRequest`, or
  `importScripts`

A timeout or budget violation MUST surface as a wizard error and
MUST NOT crash the Explorer.

#### Scenario: Long-running expression is killed

- GIVEN a `JSONata` expression that loops for 10 seconds
- WHEN the user clicks "Run preview"
- THEN the worker is terminated at 100 ms
- AND the wizard shows "Transform exceeded 100 ms budget —
  simplify the expression"

#### Scenario: Oversized input rejected

- GIVEN a `MoldQL` result of 5 MB
- WHEN the user clicks "Run preview"
- THEN the worker rejects the input and the wizard shows
  "Input exceeds 1 MB cap"

### Requirement: 5. JSONata fallback (Rust-side)

Because the `jsonata-rs` crate maturity is uncertain
(proposal §Auto-Grill Q4), the v1 backend MUST accept BOTH
`Transform::Jsonata` and `Transform::None`. When the user picks
"no transform" in the wizard, the backend stores
`transform: None` and the runtime executor passes the
`MoldQL` result through unchanged.

The JSONata Rust executor is a follow-up; for v1 the explorer
preview is the only JSONata execution path. Backend just stores
the expression and returns it on `view_spec_get`.

#### Scenario: None transform persists

- GIVEN the user picks "no transform" in the wizard
- WHEN they save
- THEN the stored ViewSpec has `transform: None` (omitted on the
  wire thanks to `#[serde(skip_serializing_if = "Option::is_none")]`)

## Out of Scope (Phase 4 — explicit non-requirements)

- Sharing a ViewSpec across workspaces (no link-share in v1)
- Versioning / history (saved spec is current; no diff view)
- ViewSpec tags, folders, or favourites
- Importing a ViewSpec from a URL or JSON file
- Live-collaborative authoring
- A template gallery (curated ViewSpec starters)

## Coverage

- **Happy paths**: covered (action visible, wizard enforces order,
  preview works, save posts, edit pre-fills)
- **Edge cases**: covered (ownership, JSONata budget, oversized
  input, None transform)
- **Error states**: covered (timeout, parser error, save failure
  toast)
