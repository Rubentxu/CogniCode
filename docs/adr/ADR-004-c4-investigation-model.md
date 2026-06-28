# ADR-004: C4 Investigation Model — From Directory Projection to Semantic Levels

**Status**: PROPOSED  
**Date**: 2026-06-28  
**Deciders**: User, orchestrator session 2026-06-28

## Context

The current C4 implementation in CogniCode Explorer is a **directory-derived
component graph**. The `architecture_handler` (`crates/cognicode-explorer/src/api.rs:837`)
calls `build_architecture()` which synthesises nodes from `module_list()` and
creates `part_of` edges reflecting directory hierarchy.

This produces a useful **structural map** but is not true C4:
- No distinction between **Context** (system + external actors)
- No distinction between **Container** (deployable units: API, UI, DB, workers)
- No runtime relationships between containers
- No **Component**-level decomposition within containers (services, repos, handlers)
- No **Code**-level drilldown (symbols, types, routes, tests)
- No **Dynamic** view (request flow across containers/components)

The PerspectiveToggle in the UI shows "Graph" vs "C4 Components", which
**overpromises** architectural meaning for what is essentially a directory tree.

Meanwhile, `ViewKind` already reserves `C4Context`, `C4Container`,
`C4Component`, `C4Code` (`crates/cognicode-explorer/src/dto.rs:1172-1175`),
and `CONTEXT.md` catalogues all four levels plus the C4 hierarchy kind.
The domain vocabulary exists; the implementation does not.

## Decision

**Evolve C4 from a single directory-projection toggle into a multi-level
investigation model with overlays and dynamic views.**

### 1. Rename the current C4 toggle honestly

The current "C4 Components" perspective will be renamed to **"Structure"**
(or "Inferred Components") until true C4 levels are implemented. This prevents
false expectations.

### 2. Introduce C4 levels as explicit ViewKinds

| Level | ViewKind | Focus | Source |
|-------|----------|-------|--------|
| Context | `c4_context` | System + external actors + entry points | Derived from entry points, routes, IaC resources |
| Container | `c4_container` | Deployable units + runtime relationships | Derived from crate/workspace structure + dependency analysis |
| Component | `c4_component` | Internal modules per container | Enhanced `build_architecture()` with semantic grouping |
| Code | `c4_code` | Symbols within a component | Existing call graph / symbol views, scoped to component |
| Dynamic | `c4_dynamic` (new) | Request/event flow across containers | Derived from vertical slice / call path / event trace |

### 3. C4 level selector in UI

Replace the binary `PerspectiveToggle` (Graph ↔ C4) with a **level selector**:

```
[ Graph ] [ Context ] [ Container ] [ Component ] [ Code ]
```

Or a segmented control that includes the current graph as one option.

### 4. Overlays for investigation

Each C4 level supports **overlays** that color/annotate nodes:

| Overlay | What it shows |
|---------|---------------|
| **Hotspots** | Node complexity / fan-in (from existing `get_hot_paths`) |
| **Ownership** | Module/crate owner (requires ownership attribution — future) |
| **Drift** | Divergence from expected architecture (`drift_handler` already exists) |
| **Test coverage** | Test-to-code ratio per component |
| **Risk** | Combined: complexity + churn + dependency pressure |

Overlays are toggleable; only one active at a time (initially).

### 5. Dynamic views from investigation traces

A `c4_dynamic` view is generated from a **real execution path**:
- HTTP route → handler → use case → repository → DB
- Event → listener → handler → side effects

This connects C4 architecture to actual runtime behavior, which is the most
valuable C4 view for debugging and onboarding.

### 6. C4 → draw.io export

Every C4 level is exportable to Mermaid C4 syntax (per ADR-003), which
draw.io imports as editable shapes.

### 7. Expected architecture baseline

`.cognicode/architecture/expected.drawio` (or `.mermaid`) defines the
human-curated expected architecture. The `drift_handler` compares inferred
C4 against this baseline and reports:
- Missing containers/components
- Extra containers/components
- Wrong relationships

This makes C4 a **governance instrument**, not just a visualization.

## Alternatives considered

### A. Keep single-level directory C4
- Pros: Zero implementation effort.
- Cons: Overpromises, limited investigative value, no governance.
- **Rejected** — doesn't meet the product goal.

### B. Adopt Structurizr DSL as the C4 model
- Pros: Purpose-built C4 tooling.
- Cons: Adds a dependency, not draw.io-compatible, requires DSL learning.
- **Rejected** — Mermaid C4 syntax is sufficient and portable.

### C. Full C4 extraction from code (automatic container/component inference)
- Pros: Most accurate.
- Cons: Extremely hard to get right; requires heuristics per language/framework.
- **Deferred** — start with directory + module inference, evolve toward semantic.

## Consequences

### Positive
- C4 becomes a real investigation tool, not a cosmetic toggle.
- Dynamic views connect architecture to runtime behavior.
- Drift detection becomes meaningful with expected architecture.
- Draw.io export makes C4 communicable to stakeholders.

### Negative
- Significant implementation effort (5 ViewKinds + overlays + dynamic traces).
- C4 level inference will be imperfect (heuristic, not semantic extraction).
  **Mitigation**: start with directory/module inference, allow manual override.
- Overlay data (ownership, risk) requires additional backend capabilities.
  **Mitigation**: phase overlays; start with drift + hotspots (already available).

## Implementation phases

| Phase | Deliverable |
|-------|-------------|
| C4-1 | Rename toggle; honest labels |
| C4-2 | Level selector; Context + Container (basic) |
| C4-3 | Component enhancement (semantic grouping) |
| C4-4 | Overlays: drift + hotspots |
| C4-5 | Dynamic views from traces |
| C4-6 | Expected architecture + drift governance |
| C4-7 | Mermaid C4 export per level |

## References

- [ADR-002](./ADR-002-moldable-exploration-parity-program.md) — Moldable exploration program
- [ADR-003](./ADR-003-diagram-representations.md) — Diagram representations
- `CONTEXT.md` — C4 hierarchy kind, ViewKind catalog
- `crates/cognicode-explorer/src/api.rs:837` — `architecture_handler`
- `crates/cognicode-explorer/src/facades/graph.rs:987` — `build_architecture_creates_part_of_edges`
- `apps/explorer-ui/src/components/PerspectiveToggle.tsx` — current toggle
- `apps/explorer-ui/src/hooks/useArchitecture.ts` — current C4 fetch
