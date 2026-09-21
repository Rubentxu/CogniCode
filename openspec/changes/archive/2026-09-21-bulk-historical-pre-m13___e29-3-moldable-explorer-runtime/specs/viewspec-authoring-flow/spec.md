# Delta for ViewSpec Authoring Flow

This delta lifts `viewspec-authoring-flow` so the wizard saves round-trip through the real `PostgresViewSpecStore` (no stub), invokes execute to validate the spec, and surfaces inline status feedback during save and execute.

## MODIFIED Requirements

### Requirement: 2. Wizard structure

The authoring flow MUST be a 5-step wizard rendered in a modal drawer:

1. **Pick ViewKind** — searchable list of all values from `ViewRegistry.known_view_kinds()`, grouped by family (Core, C4, Quality, Architecture, Development, Living doc).
2. **Pick RendererKind** — searchable list, defaulting to the `default_renderer_for(view_kind)` mapping from CONTEXT.md §ViewSpec.
3. **Configure data source** — a `MoldQL` text editor with autocomplete from the catalog of available objects (`symbols`, `docs`, `evidence`, `issues`, `rules`, `decisions`).
4. **Adjust transform** — a `JSONata` expression editor whose input is
   `TransformInputV1`: the JSON serialization of
   `ContextualView.projection.payload`, excluding the projection envelope.
   Preview MUST obtain this payload from the same execute contract used at
   runtime. Errors are shown inline.
5. **Save** — a summary screen with title, applies_to (inferred from the focused object), and a Save button. **Save calls `POST /api/viewspecs` (create) or `PUT /api/viewspecs/:id` (edit) against `PostgresViewSpecStore`. After the spec is persisted, the wizard MUST call `POST /api/viewspecs/:id/execute` with the active `revision_id`, apply the persisted JSONata transform with the same evaluator used by preview, and surface the resulting execute `status` (`ready`, `empty`, `truncated`, `error`) as inline feedback. The drawer closes only when status is `ready`; `error` and `truncated` keep it open. `revision_change` is a client-owned pane state, not an execute-response status.**

(Previously: step 5 always called `POST /api/viewspecs` against a stub; no execute call; no status feedback.)

#### Scenario: Wizard enforces step ordering

- GIVEN the user opens the wizard
- WHEN they click "Next" on step 1 without picking a ViewKind
- THEN the Next button is disabled; no skip-to-step-3

#### Scenario: Live JSONata preview uses TransformInputV1

- GIVEN step 4 with a `JSONata` expression `items[fan_out > 5]` and a preview execute response whose projection payload has 12 items
- WHEN the user types the expression
- THEN a side panel shows the filtered items in real time
- AND the evaluator input is exactly `TransformInputV1`, not the full `ContextualView`
- AND any expression error is rendered in red with the parser message

#### Scenario: Save posts to backend and executes

- GIVEN the user reaches step 5 with a valid spec and an active `revision_id R`
- WHEN they click Save
- THEN `POST /api/viewspecs` is called with the spec body
- AND a follow-up `POST /api/viewspecs/:id/execute { revision_id: R }` runs automatically
- AND inline feedback beneath Save shows the execute `status` (`ready`, `empty`, `truncated`, `error`)
- AND the saved transform output equals the step-4 preview for the same input
- AND the drawer closes only when status is `ready`

#### Scenario: Save error keeps drawer open

- GIVEN the execute call returns `status = "error"`
- WHEN the wizard receives the response
- THEN the drawer stays open, the error message renders inline in red, and no navigation occurs
