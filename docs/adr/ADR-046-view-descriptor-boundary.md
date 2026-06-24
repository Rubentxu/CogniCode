# ADR-046: ViewDescriptor Boundary Contract

## Status

ACCEPTED — 2026-06-24

## Context

CogniCode has two parallel definitions of `ViewDescriptor` and `ViewSpec` that mirror each other across the explorer/core crate boundary:

- `cognicode_explorer::dto::ViewDescriptorDto` (struct, 4 fields) and `cognicode_explorer::dto::ViewSpec` (struct, typed enums)
- `cognicode_core::interface::mcp::schemas::ViewDescriptor` (struct, 4 fields) and `cognicode_core::interface::mcp::schemas::ViewSpec` (struct, string-based)

The one-way crate dependency (explorer → core) means core cannot import the explorer types. When ADR-008 added the ViewSpec tools to the core MCP, the types were duplicated as string-based schemas.

ADR-044 (v0.11.2) documented the **data** bridge (`BUILTIN_DESCRIPTORS_RAW` in `core::schemas::builtin_descriptors`) but did NOT resolve the type-level divergence. This ADR addresses the type-level concern by formalizing an anti-corruption layer.

## Decision

The explorer crate owns four `From` impls that formalize the boundary as an explicit anti-corruption layer:

1. `From<core::schemas::ViewDescriptor> for ViewDescriptorDto` (lossless, pure field copy)
2. `From<ViewDescriptorDto> for core::schemas::ViewDescriptor` (lossless, pure field copy)
3. `From<dto::ViewSpec> for core::schemas::ViewSpec` (lossless via serde round-trip)
4. `From<core::schemas::ViewSpec> for dto::ViewSpec` (lossless + infallible via `Custom(String)`/`#[serde(other)]` forward-compat arms on enum types)

These impls live in `crates/cognicode-explorer/src/boundary.rs` (new module).

## Boundary Contract

### 1. Field-parity invariant

Every field present in one side of the boundary pair must be present in the other, OR an explicit divergence is recorded in this ADR. The hand-written round-trip unit tests in `boundary.rs` enforce parity for the current field set:

- `ViewDescriptor` ↔ `ViewDescriptorDto` — 4 fields (id, title, is_builtin, source)
- `ViewSpec` ↔ `core::schemas::ViewSpec` — 11 fields (id, title, applies_to, view_kind, data_source, transform, renderer_kind, props, created_at, updated_at, owner)

Adding a field to one side without the other is **silent semantic drift** and is a contract violation.

### 2. `From` impl ownership rule

The **explorer** crate is the only crate that implements these conversions because it is the only crate that imports both sides (orphan-rule + dep direction explorer→core).

If a future `core → explorer` direction is ever needed, the impl must still live in the explorer crate (or a new shared `boundary` crate), never in core.

### 3. `ViewSpecSummary` decision

Preserved. `ViewSpecSummary` is consumed by Spotter search results (NOT dead). Verified by SDDK explore #2797.

### 4. Wire-format stability

HTTP JSON shape is unchanged across this change. The struct rename is internal; serde derives are identical; the JSON wire format is byte-identical.

### 5. Preventive ACL framing

The 4 `From` impls formalize a boundary already documented in ADR-044, NOT a replacement of existing conversion code. No cross-crate conversion exists today between the explorer `dto` types and the core `schemas` types. The impls are a **preventive** anti-corruption layer (DDD pattern).

### 6. `raw_to_view_descriptor` (L38) consolidation

`registry::raw_to_view_descriptor` (L38) was a manual conversion of `BuiltinDescriptorRaw` → `ViewDescriptorDto`. After this ADR, it is reduced to a one-liner that uses the new `From` impl.

### 7. `consolidated_handlers.rs:1339` deferred

L1339 maps `ViewSpecRow` (persistence) → `core::schemas::ViewSpec`. This is a different concern (persistence → schema, not explorer DTO → core schema). It is OUT OF SCOPE for this ADR. Deferred to a separate change.

## Consequences

### Positive

- The explorer↔core boundary is now explicit and documented
- Future divergence between dual types will be caught by the round-trip tests
- The trait/struct name collision is resolved (`ViewDescriptor` is the trait, `ViewDescriptorDto` is the struct)
- `raw_to_view_descriptor` is a one-liner — no risk of drift
- `list_all_builtin_descriptors` (zero callers) is removed
- HTTP wire format is byte-identical — no consumer impact

### Negative

- 4 hand-written `From` impls add maintenance surface (mitigated by round-trip tests)
- A new module `boundary.rs` increases the cognitive surface of the explorer crate
- Field-parity invariant is enforced by hand, not by a derive macro (deferred to post-merge)

## References

- **ADR-008** — MCP ViewSpec tool surface (the trigger for the original divergence)
- **ADR-044** — Data-sharing bridge via `BUILTIN_DESCRIPTORS_RAW` (predecessor adaptation)
- **CONTEXT.md** — `ViewDescriptor` (trait) and `ViewExecutor` terminology
- **engram #2797** — SDDK explore report
- **engram #2807** — SDDK proposal (Option B + C adopted)
- **engram #2809** — SDDK spec (amended)
- **engram #2811** — SDDK design (4 corrections)
- **engram #2813** — SDDK coherence (88/100 PASS)
