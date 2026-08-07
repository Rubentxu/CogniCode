# Delta for ViewSpec Authoring Flow

This delta lifts `viewspec-authoring-flow` from "Phase 4 (DEFERRED)" to "Phase 4 (ACTIVE)" and adds the user-visible behaviours required for the first reviewable slice. The JSONata sandbox lives in `viewspec-jsonata-sandbox`; the wizard consumes it.

## ADDED Requirements

### Requirement: 6. Auto-preview on Transform step

When the user reaches step 4 AND the `JSONata` expression is non-empty, the wizard MUST auto-trigger the preview (see `viewspec-jsonata-sandbox`) without a click. The trigger MUST be debounced to 300 ms after the last expression or input change. The preview panel MUST re-render in place with the new `output` or `error`.

#### Scenario: Typing auto-previews

- GIVEN step 4 with an empty input
- WHEN the user types `items[price > 10]`
- THEN within 300 ms the preview panel shows the filtered output

#### Scenario: Editing the input re-previews

- GIVEN step 4 with a saved expression
- WHEN the user changes the MoldQL input in step 3 and returns to step 4
- THEN the preview re-runs against the new input

### Requirement: 7. Inline JSONata error display

When the sandbox reports `{ ok: false, error }`, the wizard MUST display the error inline beneath the editor in red, verbatim from JSONata's parser. No modal, no toast.

#### Scenario: Parse error renders inline

- GIVEN the user types `items[`
- WHEN the sandbox returns `{ ok: false, error: "Syntax error: expected expression" }`
- THEN the wizard renders the error in red beneath the editor

#### Scenario: Budget exceeded renders inline

- GIVEN a long-running expression
- WHEN the sandbox returns `{ ok: false, error: "budget_exceeded" }`
- THEN the wizard renders "Transform exceeded 100 ms budget — simplify the expression"

### Requirement: 8. Edit mode pre-fills all fields

`ViewSpecWizard` MUST accept an optional `editSpec?: ViewSpec` prop. When provided, the wizard MUST pre-fill every field (title, view_kind, renderer_kind, applies_to, data_source, transform, props) before first paint, and Save MUST call `PUT /api/viewspecs/:id` instead of `POST /api/viewspecs`.

#### Scenario: Edit pre-fills

- GIVEN `editSpec = { id: "V", title: "Hot Symbols", ... }`
- WHEN the wizard opens
- THEN every field shows the corresponding value on first render
- AND the Save button label reads "Save changes"

#### Scenario: Edit Save calls PUT

- GIVEN the wizard is in edit mode
- WHEN the user clicks Save
- THEN the request is `PUT /api/viewspecs/V`
- AND on 200, the wizard closes and `useWizardDraft.clear()` runs

### Requirement: 9. Edit mode ownership check

The wizard MUST only enter edit mode if `editSpec.owner === current_user`. If the spec is owned by another user, the "Edit" action is hidden in `ViewTabs` and the wizard MUST refuse to open in edit mode even if a programmatic caller passes `editSpec`.

#### Scenario: Edit hidden for non-owned spec

- GIVEN a ViewSpec owned by `alice`
- WHEN `bob` views the spec in `ViewTabs`
- THEN "Edit" is absent from the overflow menu

#### Scenario: Wizard refuses non-owned edit

- GIVEN `openWizard({ editSpec: <alice's spec> })`
- WHEN the current user is `bob`
- THEN the wizard does NOT open
- AND a `console.warn` is logged with the spec id

## MODIFIED Requirements

### Requirement: 2. Wizard structure

The authoring flow MUST be a 5-step wizard rendered in a modal drawer:

1. **Pick ViewKind** — searchable list of all values from `ViewRegistry.known_view_kinds()`, grouped by family (Core, C4, Quality, Architecture, Development, Living doc).
2. **Pick RendererKind** — searchable list, defaulting to the `default_renderer_for(view_kind)` mapping from CONTEXT.md §ViewSpec.
3. **Configure data source** — a `MoldQL` text editor with autocomplete from the catalog of available objects (`symbols`, `docs`, `evidence`, `issues`, `rules`, `decisions`).
4. **Adjust transform** — a `JSONata` expression editor with side-by-side input (the `MoldQL` result) and output panels. **Preview auto-triggers on expression or input change, debounced 300 ms (Requirement 6); parser errors render inline in red (Requirement 7).**
5. **Save** — a summary screen with title, applies_to (inferred from the focused object), and a Save button. **Save calls `POST /api/viewspecs` for create mode, or `PUT /api/viewspecs/:id` for edit mode (Requirement 8); the drawer closes on success.**

(Previously: step 4 required a manual "Run preview" click; step 5 always called `POST`; transform errors were not specified inline.)

#### Scenario: Wizard enforces step ordering

- GIVEN the user opens the wizard
- WHEN they click "Next" on step 1 without picking a ViewKind
- THEN the Next button is disabled; no skip-to-step-3

#### Scenario: Live JSONata preview

- GIVEN step 4 with a `JSONata` expression `items[price > 10]` and a `MoldQL` result with 12 items
- WHEN the user types the expression
- THEN the side panel shows the filtered items within 300 ms (no Run click)
- AND any expression error is rendered in red with the parser message (Requirement 7)

#### Scenario: Save posts to backend

- GIVEN the user reaches step 5 with a valid spec
- WHEN they click Save
- THEN `POST /api/viewspecs` (or `PUT /api/viewspecs/:id` in edit mode) is called with the spec body
- AND the drawer closes
- AND the new or updated view appears in the available-views list within 5 seconds (via SWR revalidation)

### Requirement: 3. Edit existing ViewSpec

The authoring flow MUST also support editing a saved runtime `ViewSpec`. The "Edit" action appears in the `ViewTabs` overflow menu next to the view's title. The wizard opens pre-populated with the saved spec's values; **Save calls `PUT /api/viewspecs/:id` (Requirement 8)**. **The Edit action MUST only render when the spec is owned by the current user; if a programmatic caller bypasses this, the wizard refuses to open in edit mode (Requirement 9).**

(Previously: pre-fill was high-level; the explicit PUT call and the ownership check are new.)

#### Scenario: Edit pre-fills all fields

- GIVEN a saved `ViewSpec` with id `V` and title "Hot Symbols"
- WHEN the user picks "Edit" on that view
- THEN the wizard opens with title="Hot Symbols", view_kind=QualityHotspots, data_source already populated, and the renderer_kind pre-selected
- AND the Save button label reads "Save changes"

#### Scenario: Edit requires ownership check

- GIVEN a `ViewSpec` owned by a different `owner` than the current user
- WHEN the user views that spec in the Explorer
- THEN the "Edit" action is hidden; only the workspace owner sees it (MCP `view_update` may differ — see ADR-008)

## REMOVED Requirements

### Requirement: 4. JSONata sandbox (moved to `viewspec-jsonata-sandbox`)

(Reason: the full sandbox contract — worker file path, lazy-load, 100 ms budget, 1 MB cap, race cancellation — now lives in `viewspec-jsonata-sandbox`. The wizard is a consumer; duplicating the contract here would drift.)

The wizard MUST still call the sandbox through the contract in `viewspec-jsonata-sandbox`. Step 4 just uses it.
