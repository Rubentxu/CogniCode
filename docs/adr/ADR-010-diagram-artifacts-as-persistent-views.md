# ADR-010: Diagram artifacts as persistent, regenerable views

**Status**: ACCEPTED  
**Date**: 2026-07-22  
**Deciders**: User, OpenCode orchestrator  

## Context

CogniCode already supports Mermaid, C4, draw.io-derived flows, and snapshot
export paths. However, diagrams are still too close to being export side
effects instead of durable architecture artifacts.

The problem is not “we cannot draw.” The problem is “we cannot reliably treat a
diagram as a first-class, inspectable, regenerable object linked to the graph,
the decision record, and the user narrative.”

## Decision

We will treat diagrams as **persistent artifacts derived from real views**, not
as standalone drawings.

### Rules

1. Every diagram must have a source view or source query.
2. Every persisted diagram must carry provenance:
   - source object id;
   - source view kind/spec;
   - export format;
   - creation timestamp;
   - investigation/ADR linkage when present.
3. Diagram artifacts must be visible and usable in Explorer UI, not only
   downloadable.
4. Regeneration must preserve semantic linkage even if the rendered file is
   replaced.

### Supported representation modes

- transient preview from a current view;
- persisted artifact attached to investigation or ADR;
- reopened artifact with trace-back to its source view.

## Alternatives considered

### 1. Diagram files only (`.mmd`, `.png`, `.drawio`) with no object identity

Rejected. This loses provenance, traceability, and moldable navigation.

### 2. External diagram tool as the canonical source of truth

Rejected. CogniCode would become a consumer of diagrams rather than the system
that explains them.

### 3. Backend-only export endpoints without UI persistence

Rejected. The product requirement is visible, explorable architecture, not only
file generation.

## Consequences

### Positive

- Diagrams become part of the knowledge graph and investigation workflows.
- Users can move from visual artifact to source rationale and back.
- Export remains available without losing moldable semantics.

### Negative

- Requires artifact metadata discipline and UI affordances.
- Adds lifecycle questions such as regeneration, versioning, and stale exports.

### Mitigations

- Start with Mermaid/C4/export surfaces already present.
- Treat diagrams as derived, not hand-authored source-of-truth objects.

## Revision Note — E24.1 (2026-07-23)

Rules R1–R4 are now **closed** by the E24.1 slice (PR 1–4, `feat/e24-diagram-artifacts`):

- **R1 (source view/query)**: `DiagramProvenance` struct carries `object_id`, `view_kind` (String tag), `spec_id: Option<String>`, `query_id: Option<String>`. Single structured source per diagram.
- **R2 (provenance metadata)**: `investigation_artifacts` gains a nullable JSONB `provenance` column (migration m0016, additive). `Artifact` domain struct carries `provenance: Option<DiagramProvenance>`. Backward-compatible — pre-E24.1 rows deserialize with `provenance: None`.
- **R3 (Explorer visibility)**: `ExportMenu` builds provenance on auto-save. `MermaidRenderer` shows provenance badge. `PaneInspector` reopen renders badge with "Reopen source view" button dispatching `SELECT_OBJECT`.
- **R4 (regeneration)**: `DiagramRegenerator` re-emits Mermaid from provenance using the existing `emit_mermaid_for_snapshot` dispatch table. Returns `Err(RegenerateError::SourceNotFound)` when source object is gone.

**Out of scope for E24.1**: standalone (non-investigation) diagrams (E24.2), real `DecisionTrace` executor (E24.3), live SVG rendering (Phase 2).

## References

- [ADR-003](./ADR-003-diagram-representations.md)
- [ADR-004](./ADR-004-c4-investigation-model.md)
- [ADR-005](./ADR-005-investigation-mode.md)
