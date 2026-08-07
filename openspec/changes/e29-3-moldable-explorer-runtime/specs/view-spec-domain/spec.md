# Delta for ViewSpec Domain Vocabulary

This delta extends `view-spec-domain` by adding `mermaid` to the `RendererKind` enum (Rust + TS) so the new Mermaid built-in renderer in `renderer-registry-frontend` can be selected by `renderer_kind` dispatch.

## MODIFIED Requirements

### Requirement: 2. `RendererKind` and `HierarchyKind` enums

The system MUST define `RendererKind` and `HierarchyKind` enums with the same serde + zod discipline as `ViewKind`.

`RendererKind` built-ins:
`graph`, `table`, `tree`, `code`, `markdown`, `vega_lite`, `json`,
`composite`, **`mermaid`**. Falls back to `Custom(String)` for unknown ids.

`HierarchyKind` built-ins:
`file_tree`, `module_tree`, `type_hierarchy`, `call_hierarchy`,
`package_graph`, `c4_hierarchy`. Falls back to `Custom(String)`.

(Previously: `RendererKind` did not include `mermaid`.)

#### Scenario: `RendererKind::Json` round-trip

- GIVEN `RendererKind::Json`
- WHEN serialised then deserialised
- THEN the value equals `RendererKind::Json`

#### Scenario: Unknown hierarchy id deserialises

- GIVEN `"hierarchy_kind": "experimental_x"`
- WHEN deserialised
- THEN `HierarchyKind::Custom("experimental_x".to_string())`

#### Scenario: `RendererKind::Mermaid` serialises snake_case

- GIVEN `RendererKind::Mermaid`
- WHEN `serde_json::to_string` runs
- THEN output is `"mermaid"`

#### Scenario: TS schema includes `mermaid` literal

- GIVEN the TS `rendererKindSchema`
- WHEN the schema enumerates its literals
- THEN `"mermaid"` is in the list alongside the other built-ins