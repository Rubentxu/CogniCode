# Spec: ViewRegistry Backend Skeleton

## Purpose

Introduce a trait-object `ViewRegistry` that backend code uses to
discover built-in views by id. The registry is the first seam
toward the four-layer Moldable View Runtime (ADR-008). v1 scope
covers the **skeleton** (trait + `linkme` registration + descriptor
listing). Runtime ViewSpec CRUD lives in Phase 2 and is
intentionally out of scope here. Existing view builders
(`build_overview`, `build_callgraph`, `build_source`, `build_*_quality_*`)
MUST keep working through direct calls; the registry is an
*additional* discovery surface, not a replacement of the existing
service-layer dispatch.

## Domain

`view-registry-backend` — NEW capability. No existing spec to
delta against; this is a full spec.

**Phase**: 1 (skeleton). Phases 2–5 are tracked in sibling specs
(`view-spec-domain` Phase 0, `viewspec-authoring-flow` Phase 4,
`entry-point-resolver` Phase 5).

---

## ADDED Requirements

### Requirement: 1. `ViewDescriptorProvider` trait

The system MUST define a `ViewDescriptorProvider` trait in
`crates/cognicode-explorer/src/registry.rs`:

```rust
pub trait ViewDescriptorProvider: Send + Sync {
    /// Stable id (e.g. `"overview"`, `"call-graph"`).
    fn id(&self) -> &'static str;
    /// Human title (e.g. `"Overview"`).
    fn title(&self) -> &'static str;
    /// Object kinds the view applies to.
    fn applies_to(&self) -> &'static [InspectableObjectType];
    /// Semantic view intent. The provider's own `view_kind` is the
    /// canonical value — for built-ins it is the well-known constant;
    /// for future runtime providers it may be `ViewKind::Custom(_).
    fn view_kind(&self) -> ViewKind;
    /// Whether this provider is shipped compiled-in (true) or
    /// user-defined (false). Phase 1 always returns true.
    fn is_builtin(&self) -> bool { true }
}
```

The trait MUST NOT carry the `ContextualView` payload — descriptor
listing is metadata only. The existing service-layer dispatch
(which DOES build the payload) is unchanged.

#### Scenario: Provider returns stable metadata

- GIVEN a `StaticProvider { id: "overview", title: "Overview",
  applies_to: [Symbol], view_kind: ViewKind::Custom(...) }`
- WHEN `id()`, `title()`, `applies_to()`, `view_kind()` are called
- THEN each returns the configured value
- AND `is_builtin()` returns `true`

#### Scenario: Descriptor listing has no payload

- GIVEN a `ViewDescriptorProvider` for `overview`
- WHEN the registry returns its `ViewDescriptor`
- THEN the descriptor carries only `{ id, title, view_kind,
  applies_to, is_builtin }` — no blocks, no relations, no evidence

### Requirement: 2. `linkme`-based static registration

The system MUST provide a `register_view!` macro that emits a
`#[distributed_slice]` entry plus a `ViewDescriptorProvider` impl
block. Built-in views (`overview`, `call-graph`, `source`, `quality`)
MUST register through the macro in their respective
`domain/views.rs` files. The macro MUST work on stable Rust.

A single global `BUILTIN_VIEW_DESCRIPTORS: &[&dyn
ViewDescriptorProvider]` slice is exposed by the registry module
and is the only entry point callers need.

The system MUST use `linkme` as the primary registration mechanism
(used by `bevy`, `tracing-subscriber`). If `linkme` cannot compile
on the current toolchain, the registry MUST fall back to a
hand-rolled `inventory` / `OnceLock<Vec<&'static dyn
ViewDescriptorProvider>>` initialised at process start; the
fallback is documented in the module header.

#### Scenario: Built-in views are visible through the slice

- GIVEN the four built-in providers registered via the macro
- WHEN `BUILTIN_VIEW_DESCRIPTORS.iter()` runs
- THEN the slice contains exactly 4 entries with ids
  `["overview", "call-graph", "source", "quality"]`

#### Scenario: Duplicate id is a compile error

- GIVEN two `register_view!` invocations with `id: "overview"`
- WHEN `cargo build` runs
- THEN the build fails with a name-collision error from the macro

#### Scenario: linkme fallback works

- GIVEN a build where `linkme` is not available
- WHEN the registry initialises
- THEN the same 4 built-in descriptors are exposed via the
  `OnceLock` fallback; service tests pass unchanged

### Requirement: 3. `ViewRegistry` service

The system MUST define a `ViewRegistry` struct in
`crates/cognicode-explorer/src/registry.rs`:

```rust
pub struct ViewRegistry { /* opaque */ }

impl ViewRegistry {
    /// `None` ⇒ use built-in descriptors only. Phase 1 always
    /// passes `None`. Phase 2+ passes a `ViewSpecStore` handle.
    pub fn new(spec_store: Option<Arc<dyn ViewSpecStore>>) -> Self;

    /// All views that apply to the given object type, in stable
    /// order: built-ins first (by id), then runtime specs (by
    /// title).
    pub fn list_for(
        &self,
        object_type: InspectableObjectType,
    ) -> Vec<ViewDescriptor>;

    /// Look up a single view by id across built-ins + runtime.
    pub fn get(&self, id: &str) -> Option<ViewDescriptor>;

    /// Stable catalog of all known `ViewKind` values, for the
    /// authoring wizard. Phase 1 returns the Rust enum's
    /// `view_kind_catalog()` static list.
    pub fn known_view_kinds(&self) -> &'static [ViewKind];
}
```

In Phase 1 (this spec), `spec_store` MUST be `None` and the
runtime path is a no-op that returns an empty `Vec`. The
`ViewSpecStore` trait is declared as a forward declaration only
(no methods) so the type compiles but is never called.

#### Scenario: List filters by applies_to

- GIVEN the 4 built-in providers register
  `["overview", "call-graph", "source", "quality"]`, where
  `overview`/`call-graph`/`source` apply to `Symbol` and `quality`
  applies to `Symbol` + `File` + `Scope`
- WHEN `list_for(Symbol)` runs
- THEN the result has 4 entries
- WHEN `list_for(File)` runs
- THEN the result has exactly 1 entry: `quality`

#### Scenario: Built-ins sort by id

- GIVEN providers with ids `["call-graph", "overview", "source",
  "quality"]`
- WHEN `list_for(Symbol)` runs
- THEN the result order is
  `["call-graph", "overview", "quality", "source"]`
  (alphabetical by id)

#### Scenario: Unknown id returns None

- GIVEN the registry has 4 built-ins
- WHEN `get("nonexistent")` runs
- THEN `None` is returned

#### Scenario: Phase 1 ignores spec_store

- GIVEN `ViewRegistry::new(Some(store))` with a mock store that
  would return 3 runtime views
- WHEN `list_for(Symbol)` runs
- THEN the result has only the 4 built-in views; the store is
  never queried

### Requirement: 4. Service integration is additive

`ExplorerService` MUST keep its current `build_*` dispatch and
`apply_lens` flow working unchanged. The registry is wired into
**only** the `available_views` listing path
(`GET /api/objects/:id/views`). No existing call site is replaced.

The `InspectableObjectSummary.available_views` field MUST be
populated by `ViewRegistry::list_for(object.type)` instead of the
current hardcoded mapping. The wire shape
(`Vec<ViewDescriptor> { id, title }`) is unchanged — clients see
no difference for existing built-ins.

#### Scenario: Existing endpoint returns same shape

- GIVEN a symbol `S` of type `Symbol`
- WHEN `GET /api/objects/S/views` runs (existing endpoint)
- THEN the response body is `Vec<ViewDescriptor>` with the same
  4 entries the previous hardcoded list produced
- AND no `view_kind` field appears on the wire (Phase 1 keeps the
  descriptor shape stable; the richer `ViewDescriptor` with
  `view_kind` ships in a follow-up spec)

#### Scenario: ViewBuilder tests still pass

- GIVEN the existing tests in
  `crates/cognicode-explorer/src/domain/views.rs::tests`
- WHEN `cargo test -p cognicode-explorer` runs
- THEN every existing test passes byte-identical

### Requirement: 5. Module structure and visibility

The registry module MUST be reachable from both
`crates/cognicode-explorer/src/api.rs` (for `available_views`)
and `crates/cognicode-explorer/src/service.rs` (for future
runtime). It MUST NOT depend on `axum`, `tokio`, or any HTTP
machinery — pure Rust + serde. This keeps the registry
reusable from MCP and tests.

#### Scenario: No HTTP dependency in registry

- GIVEN `crates/cognicode-explorer/src/registry.rs`
- WHEN `cargo tree` runs scoped to the module
- THEN no `axum`, `tower`, or `hyper` symbols appear in the
  registry's dependency closure

## Out of Scope (Phase 1 — explicit non-requirements)

- `ViewSpecStore` (Postgres CRUD) — Phase 2
- `inspect_runtime_view` execution path (calling a runtime view)
  — Phase 4
- `MoldQL` data source execution against the registry — Phase 4
- ViewSpec validation at the registry layer (the DTO validates
  itself) — N/A
- Listing views through MCP — Phase 2+
- Migration tooling for the `named_views` table — separate spec
- De-duplication between built-in id and persisted spec id —
  Phase 2

## Coverage

- **Happy paths**: covered (list filters, sort, get by id, exact
  descriptor shape)
- **Edge cases**: covered (duplicate id, linkme fallback, empty
  spec_store)
- **Error states**: covered (unknown id → None; no panic on
  missing descriptors)
