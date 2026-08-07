# Delta for EntryPointResolver

This delta extends the Spotter search path so saved runtime `ViewSpec`s become first-class search hits alongside symbols, files, and the other entry-point kinds. The recommended Spotter visibility model is **all owners visible**: every user in the workspace can find every runtime view in the workspace via Spotter.

## ADDED Requirements

### Requirement: 5. ViewSpec as a Spotter result kind

`POST /api/spotter` MUST include saved `ViewSpec`s in its index and return matches with `kind: "viewspec"`. ViewSpec matches MUST be searchable by `title` and by their declared `view_kind` string. The result envelope MUST include `id`, `title`, `view_kind`, `applies_to`, `owner`, and `updated_at` for every hit.

#### Scenario: Spotter finds a ViewSpec by title

- GIVEN a saved `ViewSpec` titled "Hot Symbols" owned by `alice`
- WHEN the user types "Hot" in the Spotter
- THEN the result set contains `{ kind: "viewspec", id: "V", title: "Hot Symbols", view_kind: "quality_hotspots", applies_to: "Symbol", owner: "alice" }`

#### Scenario: Spotter finds a ViewSpec by view_kind

- GIVEN a saved `ViewSpec` with `view_kind: "callers_and_implementors"` titled "Find callers"
- WHEN the user types "callers_and_implementors" in the Spotter
- THEN the result set contains the spec as a `kind: "viewspec"` match

#### Scenario: ViewSpec matches visible to all owners

- GIVEN `alice` and `bob` in the same workspace, and `alice` saved a `ViewSpec` titled "Hot Symbols"
- WHEN `bob` types "Hot" in the Spotter
- THEN the result set contains `alice`'s spec; the entry's `owner` field is `"alice"`
- AND `bob` can open the spec in a new pane (read-only if he is not the owner — see Requirement 6)

### Requirement: 6. ViewSpec result action

Clicking a `kind: "viewspec"` Spotter result MUST open the spec in a new pane via the `EntryPoint::ViewSpec { id }` resolution path. If the spec's `applies_to` does not match the currently focused object, the Explorer opens a fresh pane scoped to the spec itself.

#### Scenario: Open from Spotter

- GIVEN the user is on a `Symbol` pane
- WHEN they click a `kind: "viewspec"` Spotter hit
- THEN a new pane opens to the right titled with the spec's title
- AND the spec is executed via `executeViewSpec(id)` and rendered with the existing renderer lookup

#### Scenario: Non-owner can view but not edit

- GIVEN a `ViewSpec` owned by `alice`
- WHEN `bob` opens the spec from Spotter
- THEN the spec renders in the new pane
- AND the "Edit" action is hidden in `ViewTabs` (the existing ownership check from `viewspec-authoring-flow` Requirement 9)

## MODIFIED Requirements

### Requirement: 4. Search results are not a flat list

`EntryPoint::SearchResult` MUST NOT resolve to a single object. The resolver returns `ResolvedEntryPoint::SearchResults { items, default_view_kind: SemanticSearchResults }`. The `items` collection now includes the new `kind: "viewspec"` matches alongside the existing symbol / file / scope hits. The Explorer renders the unified hit set as a moldable `semantic_search_results` view, not as a flat list. The user can save the result set as a `ViewSpec` from inside the view (unchanged).

(Previously: the search hit set only covered symbols, files, scopes, decisions, docs, issues, and evidence. ViewSpecs are now first-class hits — all owners visible — with the same grouping / filtering UX.)

#### Scenario: Search opens a moldable view

- GIVEN a Spotter query `"create_user"` returning 4 hits (2 symbols, 1 docs, 1 ViewSpec)
- WHEN the user submits
- THEN the Explorer opens the `semantic_search_results` view showing the 4 hits as filterable, groupable rows
- AND the ViewSpec row carries `kind: "viewspec"` and is grouped under a "Custom views" header (or similar visual treatment)

#### Scenario: ViewSpec row is interactive

- GIVEN the user is in the `semantic_search_results` view with a ViewSpec hit
- WHEN they click the hit
- THEN the spec opens in a new pane via the `EntryPoint::ViewSpec { id }` path (Requirement 6)
- AND the rest of the search results remain in the original pane

#### Scenario: Save-as ViewSpec persists

- GIVEN the user is in the `semantic_search_results` view
- WHEN they click "Save as ViewSpec"
- THEN a new `ViewSpec` is created with `view_kind = SemanticSearchResults`, `data_source.query = the original Spotter query`, `renderer_kind = Composite`, and the user's title
- AND the new view appears in `ViewTabs` within 5 s
