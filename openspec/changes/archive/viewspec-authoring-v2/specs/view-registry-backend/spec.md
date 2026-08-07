# Delta for ViewRegistry Backend

This delta activates the `ViewSpecStore` plumbing that `view-registry-backend` deferred to Phase 2. The Postgres-backed `ViewSpecStore` is already implemented; this change wires `ViewRegistry::list_for` to query it and merges runtime views into the descriptor list.

## ADDED Requirements

### Requirement: 6. `list_for` merges runtime views

`ViewRegistry::list_for(object_type)` MUST return built-in descriptors first (id-ordered), then runtime `ViewSpec`s from the `ViewSpecStore` matching `object_type`, ordered alphabetically by title. When `spec_store` is `None`, only built-ins are returned (preserving Phase 1 behaviour).

#### Scenario: List merges built-ins + runtime

- GIVEN 4 built-in providers for `Symbol` and 2 runtime specs `{ title: "Hot Symbols", applies_to: Symbol }` and `{ title: "All APIs", applies_to: Symbol }`
- WHEN `list_for(Symbol)` runs
- THEN the result has 6 entries: first 4 built-ins (id-ordered), then 2 runtime entries with `is_builtin: false`, `source: "runtime"`, alphabetically by title

#### Scenario: List filters runtime by applies_to

- GIVEN 4 built-ins for `Symbol` and 1 runtime spec with `applies_to: File`
- WHEN `list_for(Symbol)` runs
- THEN the result has only the 4 built-ins

#### Scenario: None spec_store returns built-ins only

- GIVEN `ViewRegistry::new(None)`
- WHEN `list_for(Symbol)` runs
- THEN the result equals the Phase 1 output (built-ins only)

### Requirement: 7. `get` resolves runtime ids

`ViewRegistry::get(id)` MUST look up built-ins first, then runtime specs by `ViewSpec.id`. Runtime spec ids are stable `Uuid` strings and MUST NOT collide with built-in ids (`"overview"`, `"call-graph"`, etc.).

#### Scenario: Get returns built-in

- GIVEN `get("call-graph")`
- WHEN the registry runs
- THEN the result is the built-in `ViewDescriptor` for `call-graph`

#### Scenario: Get returns runtime spec

- GIVEN a runtime spec with `id: "7f1c..."`
- WHEN `get("7f1c...")` runs
- THEN the result is the runtime `ViewDescriptor` with `source: "runtime"`

## MODIFIED Requirements

### Requirement: 3. `ViewRegistry` service

The `ViewRegistry` struct in `crates/cognicode-explorer/src/registry.rs` exposes:

```rust
pub struct ViewRegistry { /* opaque */ }

impl ViewRegistry {
    /// When `Some`, the store is queried on every `list_for` / `get`
    /// call. When `None`, runtime paths are no-ops (Phase 1 compat).
    pub fn new(spec_store: Option<Arc<dyn ViewSpecStore>>) -> Self;

    /// Built-ins first (id-ordered), then runtime specs (title-ordered).
    /// See Requirement 6.
    pub fn list_for(
        &self,
        object_type: InspectableObjectType,
    ) -> Vec<ViewDescriptor>;

    /// Look up a single view across built-ins + runtime.
    /// See Requirement 7.
    pub fn get(&self, id: &str) -> Option<ViewDescriptor>;

    pub fn known_view_kinds(&self) -> &'static [ViewKind];
}
```

`ViewSpecStore` is no longer a forward declaration. It MUST declare at minimum `list_for_applies_to(object_type) -> Vec<ViewSpecDescriptor>` and `get(id) -> Option<ViewSpec>`. The Postgres implementation lives in the explorer crate behind the `postgres` feature flag.

(Previously: `spec_store` was a forward declaration; `list_for` and `get` ignored it. The runtime path is now active.)

#### Scenario: Phase 1 backwards compatibility

- GIVEN `ViewRegistry::new(None)`
- WHEN `list_for(Symbol)` runs
- THEN only the 4 built-ins are returned

#### Scenario: Active spec_store merges runtime

- GIVEN `ViewRegistry::new(Some(postgres_store))` with 2 runtime specs for `Symbol`
- WHEN `list_for(Symbol)` runs
- THEN the result has 6 entries (4 built-ins + 2 runtime, in the order from Requirement 6)

### Requirement: 4. Service integration is additive

`ExplorerService` keeps its current `build_*` dispatch and `apply_lens` flow unchanged. The `GET /api/objects/:id/views` endpoint now returns built-ins + runtime views through `ViewRegistry::list_for(object.type)`. The wire shape (`Vec<ViewDescriptor> { id, title }`) is unchanged for existing built-ins; runtime entries add `is_builtin: false` and `source: "runtime"` so the frontend can render the "custom" badge.

(Previously: the endpoint returned built-ins only, via a hardcoded mapping.)

#### Scenario: Existing endpoint returns same shape for built-ins

- GIVEN a symbol `S` of type `Symbol`
- WHEN `GET /api/objects/S/views` runs
- THEN the response includes the 4 built-in entries with the same `{ id, title }` shape as before
- AND any runtime entries add `is_builtin: false` and `source: "runtime"`

#### Scenario: ViewBuilder tests still pass

- GIVEN the existing tests in `crates/cognicode-explorer/src/domain/views.rs::tests`
- WHEN `cargo test -p cognicode-explorer` runs
- THEN every existing test passes byte-identical
