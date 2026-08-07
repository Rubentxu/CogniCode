# mcp-edge-metadata Specification

## Purpose

MCP views expose per-edge `(Provenance, f64)` confidence metadata so external
agents can weigh evidence quality and apply per-edge trust heuristics instead
of treating every call-graph relation as uniformly trustworthy.

## Requirements

### Requirement: TypedRelation carries optional provenance and confidence

The `TypedRelation` DTO MUST expose two optional fields: `provenance`
(`Option<String>`) and `confidence` (`Option<f64>` in range 0.0..=1.0). Both
fields MUST be annotated with `#[serde(default)]` and MUST serialize as JSON
`null` when absent. Existing call sites that construct `TypedRelation` without
these fields MUST continue to compile and serialize cleanly.

#### Scenario: View builder populates metadata from a metadata-aware repository

- GIVEN a view builder receives a repository that downcasts to
  `dyn MetadataAwareRepository` and the edge has `(Provenance::CallSite, 0.85)`
- WHEN the builder serializes a `TypedRelation` for that edge
- THEN the JSON payload contains `"provenance": "call-site"` and
  `"confidence": 0.85`
- AND neither field is `null`

#### Scenario: View builder leaves metadata null for a mock repository

- GIVEN a view builder receives a repository that does NOT downcast to
  `dyn MetadataAwareRepository` (e.g., an in-memory mock)
- WHEN the builder serializes a `TypedRelation`
- THEN the JSON payload contains `"provenance": null` and `"confidence": null`
- AND no panic or hard error is raised

### Requirement: EvidenceBlock carries optional provenance

The `EvidenceBlock` DTO MUST expose an optional `provenance` field
(`Option<String>`) annotated with `#[serde(default)]`. The existing
`confidence` field MUST be populated from the per-evidence edge confidence
rather than the hardcoded value `1.0`.

#### Scenario: Evidence block reports per-evidence confidence

- GIVEN an evidence block is built from an edge with confidence `0.72`
- WHEN the block is serialized
- THEN the JSON payload contains `"confidence": 0.72` (not `1.0`)
- AND `provenance` is populated with the edge's provenance string

#### Scenario: Evidence block degrades gracefully without metadata

- GIVEN an evidence block is built from a path that cannot resolve
  per-evidence confidence (mock repo, missing edge data)
- WHEN the block is serialized
- THEN `confidence` and `provenance` serialize as `null`
- AND no panic is raised

### Requirement: View builders downcast to MetadataAwareRepository

Call-graph and scope-dependency view builders MUST attempt a downcast from
`&dyn SymbolRepository` to `&dyn MetadataAwareRepository` using
`CallGraphRepository::as_metadata_aware()` (or an equivalent `Any`-based
mechanism). On success, the builder MUST populate provenance/confidence from
`callees_with_metadata()` / `dependencies_with_metadata()`. On failure, the
builder MUST leave the optional fields as `None` and SHOULD log a warning
once per view build.

#### Scenario: Downcast succeeds on a Postgres-backed repository

- GIVEN the explorer service is wired with a `CallGraphRepository` produced
  by the Postgres bridge
- WHEN `build_callgraph()` or `build_scope_dependencies()` runs
- THEN the downcast returns `Some`
- AND each emitted `TypedRelation` carries non-null provenance and confidence
- AND no "metadata unavailable" warning is logged

#### Scenario: Downcast fails on a mock repository

- GIVEN the explorer service is wired with a mock `SymbolRepository`
  (e.g., in a unit test)
- WHEN `build_callgraph()` runs
- THEN the downcast returns `None`
- AND the warning `"metadata-aware repository not available; emitting null
  provenance/confidence"` is logged at most once per view build
- AND each emitted `TypedRelation` serializes with `null` metadata

### Requirement: Serde backward compatibility for existing JSON consumers

JSON payloads produced by view builders prior to this change MUST continue to
deserialize into the updated `TypedRelation` and `EvidenceBlock` structs
without error. Payloads produced by the updated builders MUST round-trip
through `serde_json` without losing populated fields.

#### Scenario: Legacy payload deserializes into updated DTO

- GIVEN a JSON payload `{"source": "a", "target": "b", "kind": "calls"}`
  produced by a pre-change build
- WHEN `serde_json::from_str::<TypedRelation>` parses it
- THEN the call succeeds
- AND `provenance` and `confidence` resolve to `None`

#### Scenario: Enriched payload round-trips through serde

- GIVEN a `TypedRelation` populated with `provenance: Some("call-site")` and
  `confidence: Some(0.9)`
- WHEN it is serialized with `serde_json::to_string` and the result is parsed
  back via `serde_json::from_str`
- THEN both fields retain their populated values

## Acceptance Criteria

1. `TypedRelation` JSON payloads in call-graph views include `provenance` and
   `confidence` fields; values are non-null when the repository is
   metadata-aware.
2. `EvidenceBlock` JSON payloads include a `provenance` field; `confidence` is
   sourced from per-evidence edge confidence (not hardcoded `1.0`).
3. No view builder emits the literal `1.0` as a confidence default for
   relations backed by real edge data.
4. Mock repository tests produce `None` metadata fields without panic.
5. The MCP `inspect_object` tool returns enriched relations for symbols with
   known call-graph edges.
6. A serde round-trip test covers both an old (field-absent) payload and a
   new (fields-populated) payload.

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Repository downcast fails (mock, future adapter) | Fields serialize as `null`; one warning log per view build; no panic |
| Serde consumer pre-dates this change (no `provenance`/`confidence` in JSON) | Deserialization succeeds; missing fields resolve to `None` |
| Edge has no `Provenance` recorded in `CallGraph` | Field is `None` (NOT empty string, NOT 0.0) — distinguishes "unknown" from "low confidence" |
| `confidence` value out of 0.0..=1.0 range (data corruption) | Pass through; do not clamp silently. Clamping is the responsibility of the producer (Postgres bridge) |
| Multiple edges between same source/target with different provenance | Each edge emitted as a separate `TypedRelation`; no deduplication at the view layer |
| `relation_for()` helper called with `None` metadata | Helper MUST accept `Option<(Provenance, f64)>`; absent means leave DTO fields as `None` |
| View builder invoked concurrently on the same repository | Downcast and lookup MUST be safe under shared `&dyn` reference; no `&mut` introduced |

## Out of Scope

- New MCP tools (no `explorer_call_graph_edges` or similar). This change
  enriches existing view outputs only.
- Changing `ExplorerService.repo` from `Arc<dyn SymbolRepository>` to a
  concrete type. The trait object stays; downcast is at the call site.
- Enriching non-call-graph views (scope view, file view, module view) with
  metadata. Only call-graph and scope-dependency views change.
- Postgres-side flag or schema changes. The bridge already loads
  metadata-rich graphs; this change is consumer-side.
- Exposing provenance/confidence in the MoldQL executor. MoldQL stays
  metadata-blind.
- Clamping, normalization, or trust-score computation on confidence values.
  Values flow through as-recorded.
- Migrating `Provenance` from a stringly-typed enum to a richer schema
  (semver, commit SHA, etc.). The current `Provenance` type is the source of
  truth.
