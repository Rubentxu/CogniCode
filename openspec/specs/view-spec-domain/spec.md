# Spec: ViewSpec Domain Vocabulary

## Purpose

Introduce `ViewKind`, `RendererKind`, `HierarchyKind`, and the
`ViewSpec` DTO as first-class domain vocabulary in Rust and
TypeScript. The catalog is forward-compatible: a reserved
`Custom(String)` arm accepts future / user-defined values without
breaking deserialisation. No runtime behaviour changes — this spec
is purely additive vocabulary. Phase 0 of the Moldable View Runtime
roadmap.

## Domain

`view-spec-domain` — NEW capability. No existing spec to delta
against; this is a full spec.

---

## ADDED Requirements

### Requirement: 1. `ViewKind` enum (Rust + TS)

The system MUST define a `ViewKind` enum in
`crates/cognicode-explorer/src/dto.rs` and a matching
`viewKindSchema` in
`apps/explorer-ui/src/api/schemas.ts`. The enum MUST be
`#[derive(Serialize, Deserialize)]` with
`#[serde(rename_all = "snake_case")]` and MUST include a
`#[serde(other)]` `Custom(String)` variant for forward compatibility.

Built-in variants (catalog from ADR-008 §First-class ViewKind):

| Group | Variants |
|-------|----------|
| Core | `vertical_slice`, `call_graph`, `seam_map`, `dependency_graph`, `source_view`, `data_flow`, `impact_radius`, `diff_view` |
| C4 | `c4_context`, `c4_container`, `c4_component`, `c4_code` |
| Quality | `quality_hotspots`, `evidence_view`, `decision_graph` |
| Architecture | `architecture_rationale`, `architecture_drift`, `boundary_map`, `dependency_pressure`, `change_impact_story`, `ownership_map`, `risk_map`, `decision_trace` |
| Development | `test_slice`, `debug_slice`, `refactor_plan`, `callers_and_implementors`, `usage_examples`, `api_surface`, `dead_code_candidates`, `semantic_search_results` |
| Living doc | `doc_code_alignment`, `example_object`, `composed_narrative`, `project_diary`, `concept_map`, `evidence_pack` |
| Custom | `Custom(String)` — fallback for unknown / future values |

#### Scenario: Built-in variant serialises snake_case

- GIVEN `ViewKind::CallGraph` (Rust) / `"call_graph"` (TS)
- WHEN `serde_json::to_string` runs
- THEN output is `"call_graph"`

#### Scenario: Unknown string deserialises to Custom

- GIVEN JSON `"view_kind": "future_view"`
- WHEN deserialised
- THEN `ViewKind::Custom("future_view".to_string())` (no error)

#### Scenario: Rust ↔ TS parity

- GIVEN the Rust enum (sans `Custom`)
- WHEN the TS zod schema is built from the same catalog
- THEN every built-in Rust variant has a matching TS literal

### Requirement: 2. `RendererKind` and `HierarchyKind` enums

The system MUST define `RendererKind` and `HierarchyKind` enums
with the same serde + zod discipline as `ViewKind`.

`RendererKind` built-ins:
`graph`, `table`, `tree`, `code`, `markdown`, `vega_lite`, `json`,
`composite`. Falls back to `Custom(String)` for unknown ids.

`HierarchyKind` built-ins:
`file_tree`, `module_tree`, `type_hierarchy`, `call_hierarchy`,
`package_graph`, `c4_hierarchy`. Falls back to `Custom(String)`.

#### Scenario: `RendererKind::Json` round-trip

- GIVEN `RendererKind::Json`
- WHEN serialised then deserialised
- THEN the value equals `RendererKind::Json`

#### Scenario: Unknown hierarchy id deserialises

- GIVEN `"hierarchy_kind": "experimental_x"`
- WHEN deserialised
- THEN `HierarchyKind::Custom("experimental_x".to_string())`

### Requirement: 3. `ViewSpec` DTO

The system MUST define `ViewSpec` (Rust) and `viewSpecSchema` (TS).

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `id` | `String` | yes | Server-assigned `Uuid` on persist; client-suggested on create. |
| `title` | `String` | yes | Non-empty; ≤ 200 chars. |
| `applies_to` | `InspectableObjectType` | yes | The object kind the view is for. |
| `view_kind` | `ViewKind` | yes | Semantic intent. |
| `data_source` | `DataSource` | yes | `DataSource::Moldql { query }` (Phase 1 only). |
| `transform` | `Option<Transform>` | no | `Transform::Jsonata { expression }` in v1; `None` accepted. |
| `renderer_kind` | `RendererKind` | yes | Visual rendering strategy. |
| `props` | `serde_json::Value` / `unknown` | no | Renderer-specific config; defaults to `{}`. |
| `created_at` | `String` | yes (server) | ISO-8601 UTC; `#[serde(default)]` for create payload. |
| `updated_at` | `String` | yes (server) | ISO-8601 UTC; `#[serde(default)]` for create payload. |

`DataSource::Moldql` carries `{ query: String }`. Unknown
`data_source.kind` MUST deserialise to a permissive `Other`
variant and MUST NOT fail parsing.

#### Scenario: Minimal ViewSpec round-trip

- GIVEN a `ViewSpec { id, title: "Hot Symbols", applies_to:
  Scope, view_kind: QualityHotspots, data_source: Moldql { query:
  "symbols where fan_out > 5" }, renderer_kind: Table, props: {},
  created_at: now, updated_at: now }`
- WHEN `serde_json::to_string` then `from_str` runs
- THEN the deserialised value equals the original
- AND `view_kind` is `"quality_hotspots"` on the wire
- AND `renderer_kind` is `"table"` on the wire

#### Scenario: Unknown data_source kind is permissive

- GIVEN JSON `{"data_source": {"kind": "graphql", "endpoint":
  "..."}, ...}`
- WHEN deserialised
- THEN deserialisation succeeds with a permissive `Other`
  payload; no panic, no schema rejection

#### Scenario: Empty title rejected at validation

- GIVEN `ViewSpec { title: "", ... }`
- WHEN `ViewSpec::validate()` runs
- THEN it returns `Err(ViewSpecError::EmptyTitle)`

### Requirement: 4. JSON Schema for ViewSpec

The system MUST publish a static `viewspec.schema.json` (under
`crates/cognicode-explorer/src/api/schemas/`) that validates any
`ViewSpec` payload exchanged over the wire.

The schema MUST:

- Enforce `title` non-empty, ≤ 200 chars.
- Enforce `applies_to` is one of the built-in
  `InspectableObjectType` strings.
- Allow `view_kind`, `renderer_kind`, `hierarchy_kind` to be ANY
  string (forward compatibility — strict enum validation lives in
  the Rust `ViewKind`/`RendererKind` enums, not in the wire schema).
- Allow `data_source` to be any object with a `kind` string.

The schema is the public contract for tooling (MCP, third-party
clients); Rust/TS code uses the typed enums and does not validate
against the schema at runtime in the hot path.

#### Scenario: Valid payload validates

- GIVEN a JSON ViewSpec with all required fields
- WHEN `jsonschema` validates
- THEN no errors are returned

#### Scenario: Empty title fails validation

- GIVEN a JSON ViewSpec with `title: ""`
- WHEN the schema validates
- THEN a `minLength` violation is reported

#### Scenario: Forward-compatible view_kind accepted

- GIVEN a JSON ViewSpec with `view_kind: "future_ai_view"`
- WHEN the schema validates
- THEN no error; the Rust deserialiser resolves it to
  `ViewKind::Custom("future_ai_view")`

### Requirement: 5. ViewSpec errors

The system MUST define a `ViewSpecError` enum
(`EmptyTitle`, `TitleTooLong`, `UnknownAppliesTo`,
`EmptyQuery`, `InvalidUuid`) that the validation surface
(`ViewSpec::validate`, store CRUD, MCP tools) returns on rejected
input. The error enum MUST convert to the existing
`ExplorerError` shape so transport layers don't need a new variant
set.

#### Scenario: Validation errors map to ExplorerError

- GIVEN `ViewSpec::validate()` returns
  `Err(ViewSpecError::EmptyTitle)`
- WHEN the API layer wraps it
- THEN the wire response is a Problem Details JSON with
  `error="view_spec_invalid"` and a `detail` naming the field

## Out of Scope (Phase 0)

- Runtime ViewSpec store (Postgres CRUD) — Phase 2
- `view-registry-backend` trait + linkme registration — Phase 1
- `renderer-registry-frontend` map — Phase 3
- ViewSpec authoring UX — Phase 4
- EntryPointResolver default ViewKind mapping — Phase 5
- Custom `view_kind` resolution in the executor — Phase 1+

## Coverage

- **Happy paths**: covered (round-trip, snake_case, schema accept)
- **Edge cases**: covered (Custom fallback, permissive data_source,
  empty title, length cap)
- **Error states**: covered (ViewSpecError → ExplorerError mapping)
