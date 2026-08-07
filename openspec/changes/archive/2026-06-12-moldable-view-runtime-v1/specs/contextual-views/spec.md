# Delta for Contextual Views

> Adds a new requirement to the existing `contextual-views` spec
> describing the unified `available_views` listing that the
> `ViewRegistry` (Phase 1) populates. The existing 5 requirements
> (1–5) are unchanged. This delta is purely additive — the
> endpoint shape and `ContextualGraphResponse` are not touched.

## ADDED Requirements

### Requirement: 6. `available_views` listing is registry-driven

The endpoint `GET /api/objects/:object_id/views` MUST return the
`ViewDescriptor` list produced by
`ViewRegistry::list_for(object.type)`. The list MUST include every
built-in view registered via `register_view!` whose `applies_to`
matches the object's type, in alphabetical id order. The wire
shape `Vec<ViewDescriptor> { id, title }` is unchanged for Phase 1;
the richer descriptor shape (with `view_kind`, `renderer_kind`,
`is_builtin`) ships in a follow-up.

#### Scenario: Symbol listing returns 4 built-ins

- GIVEN a `Symbol` object
- WHEN `GET /api/objects/<symbol_id>/views` runs after Phase 1
- THEN the response is a JSON array of 4 elements with ids
  `["call-graph", "overview", "quality", "source"]` (alphabetical)
- AND each element has the same `{ id, title }` shape the
  pre-change endpoint produced

#### Scenario: File listing returns 1 view

- GIVEN a `File` object
- WHEN `GET /api/objects/<file_id>/views` runs
- THEN the response is a JSON array of 1 element: `[{ id:
  "quality", title: "Quality" }]`
- AND no `Symbol`-only view (`call-graph`, `source`) is present

#### Scenario: Existing test suite stays green

- GIVEN `crates/cognicode-explorer/src/api_views_tests.rs` (or
  equivalent)
- WHEN `cargo test -p cognicode-explorer views_endpoint` runs
  after Phase 1
- THEN every existing test passes byte-identical

(Previously: this endpoint was populated by a hardcoded
`match object_type` mapping in the service layer. The mapping is
replaced by `ViewRegistry::list_for`; the wire shape is preserved
for Phase 1.)
