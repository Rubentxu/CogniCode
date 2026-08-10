# ADR-003: Diagram Representations — draw.io as Derived View

**Status**: ACCEPTED (promoted 2026-08-10)
**Date**: 2026-06-28  
**Deciders**: User, orchestrator session 2026-06-28

## Context

CogniCode Explorer produces graph-backed knowledge: call graphs, C4 views,
impact traces, decision rationale, and investigation paths. Users need to
**communicate** this knowledge to stakeholders who don't use the Explorer,
and to **curate** diagrams by hand (annotate, reorganize, hide, explain).

draw.io is the de facto open-source diagramming standard. It supports:
- Mermaid import (including C4Context / C4Container / C4Component / C4Dynamic)
- CSV-driven diagram generation
- Full XML format (`mxGraphModel`) for programmatic generation
- Shape-level editing of Mermaid-imported diagrams (not just images)
- Re-editable Mermaid source stored as `mermaidData` attribute

The question is: **what is the canonical serialization format for diagrams
produced by CogniCode?**

Three options were considered:
1. Generate draw.io XML (`mxGraphModel`) directly
2. Generate Mermaid as canonical, with draw.io as import target
3. Generate both in parallel from the same graph projection

## Decision

**Mermaid is the canonical diagram serialization format. draw.io is a
first-class derived representation, not the source of truth.**

The pipeline is:

```
Knowledge Graph
  ↓
ViewKind-specific projection (C4, call graph, investigation trace, …)
  ↓
Mermaid serialization (canonical)
  ├─ Inline rendering (Explorer, Markdown, GitHub)
  ├─ draw.io import (editable shapes, curatable)
  └─ SVG/PNG snapshot (static documentation)
```

### Design rules

1. **Every exportable ViewKind** must have a `to_mermaid()` projection.
   This includes: `c4_context`, `c4_container`, `c4_component`, `c4_dynamic`,
   `call_graph`, `dependency_graph`, `impact_radius`, `decision_trace`,
   `change_impact_story`, `vertical_slice`.

2. **draw.io integration** uses draw.io's built-in Mermaid importer
   (`Arrange > Insert > Mermaid`). No custom `mxGraphModel` generation.

3. **"Open in draw.io"** is a first-class action alongside "Inspect",
   "Trace", and "Save". It appears in:
   - C4 view toolbar
   - Investigation action menu
   - Pane inspector export menu

4. **Mermaid C4 keywords** (`C4Context`, `C4Container`, `C4Component`,
   `C4Dynamic`) are preferred over raw flowcharts for architecture views,
   because draw.io renders them with proper C4 shapes.

5. **Investigation artifacts** can embed Mermaid + draw.io exports. An
   Evidence Pack or Composed Narrative can contain diagram artifacts.

6. **Expected architecture** (human-curated) is stored as draw.io files
   in `.cognicode/architecture/*.drawio`. Drift detection compares the
   inferred graph against the Mermaid projection of these files.

### Why not mxGraphModel directly?

- **Fragile**: XML schema changes with draw.io versions; Mermaid is stable.
- **Hard to test**: Mermaid is text, diffable, and human-readable.
- **Coupling**: Generating mxGraphModel couples CogniCode to draw.io internals.
- **Portability**: Mermaid works in GitHub, Markdown, Notion, Obsidian, docs.
- **No downside**: draw.io already imports Mermaid as editable shapes.

### Why not both in parallel?

- Maintenance cost of keeping two generators in sync.
- Mermaid → draw.io import is already lossless (shapes are editable).
- Adding `mxGraphModel` later is a pure additive step if needed.

## Alternatives considered

### A. Generate mxGraphModel directly
- Pros: Full control over shape geometry, styles, layers.
- Cons: Fragile, hard to test, coupled to draw.io internals, not portable.
- **Rejected** — Mermaid covers 95% of needs and is portable.

### B. Use Structurizr DSL instead of Mermaid
- Pros: Structurizr is purpose-built for C4.
- Cons: Not supported by draw.io import; requires additional tooling.
- **Rejected** — Mermaid C4 syntax is draw.io-native and sufficient.

### C. No diagram export, rely on Explorer interactivity
- Pros: Zero maintenance.
- Cons: Users can't communicate or curate knowledge outside Explorer.
- **Rejected** — diagrams are a critical knowledge artifact.

## Consequences

### Positive
- Diagrams are portable, versionable, and diffable.
- draw.io integration comes for free (built-in Mermaid import).
- Same Mermaid works in GitHub READMEs, ADRs, and docs.
- Investigation artifacts can embed diagrams.
- Expected architecture is human-curatable in draw.io.

### Negative
- Mermaid C4 syntax has layout limitations (no custom positioning).
  **Mitigation**: draw.io Mermaid group allows manual re-layout after import.
- Mermaid C4Dynamic support is evolving.
  **Mitigation**: use flowchart with annotations for dynamic views if needed.
- Some complex graphs may exceed Mermaid rendering limits.
  **Mitigation**: scope exports to focused subgraphs, not the entire workspace.

## References

- [ADR-002](./ADR-002-moldable-exploration-parity-program.md) — Moldable exploration program
- [ADR-004](./ADR-004-c4-investigation-model.md) — C4 investigation model
- [draw.io Mermaid docs](https://www.drawio.com/docs/manual/mermaid/)
- `crates/cognicode-core/src/interface/mcp/schemas.rs` — `ExportMermaidInput`
- `crates/cognicode-core/src/interface/cli/commands.rs` — `graph.to_mermaid()`
- Engram obs-049303966b1f1326 — Explorer UX audit
- Engram obs-4b9e75080598f24f — draw.io as derived representation decision

## Implementation Log

- **2026-08-10 (E31-C)**: Diagram representations implemented in crates/cognicode-diagram-* (C4, UML, generic graphs). PaneInspector renders diagrams via the DiagramViewExecutor.
