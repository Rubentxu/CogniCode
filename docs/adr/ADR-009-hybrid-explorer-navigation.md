# ADR-009: Hybrid Explorer Navigation

**Fecha:** 2026-06-12  
**Estado:** PROPOSED  
**Decisión:** Frontend-first visual navigation with backend semantic exploration persistence  
**Fuente:** grill-with-docs on gtoolkit-like navigability  
**Confianza:** alta  

---

## Context

CogniCode needs gtoolkit-like navigability: users should move from object to object, open multiple views, drill down, go back, and preserve an exploration narrative.

The current implementation has no explicit navigation model. `BrainSession` keeps a focus node and a FIFO history, but that is not enough for an Explorer UI with panes, tabs, selected graph nodes, scroll positions, and visual breadcrumbs.

The Explorer UI is the primary consumer of moldable views. MCP consumers are important, but they can receive a degraded semantic version of navigation rather than the full visual state.

## Decision

Adopt a **hybrid navigation model**:

1. **Frontend owns rich visual navigation state**
   - open panes
   - active tabs
   - selected nodes
   - scroll position
   - split layout
   - visual breadcrumbs
   - local drill-down stack
   - GtPager-like lateral pane stack

2. **Backend persists semantic exploration state**
   - ordered events such as `{ object_id, view_id, query, timestamp }`
   - enough to restore or share an exploration
   - enough for MCP to inspect the path with degraded non-visual fidelity

3. **Synchronization is explicit**
   - frontend periodically or intentionally saves semantic navigation events to an `ExplorationSession`
   - backend does not attempt to own browser-only UI details

Explorer navigation is pane-based, not replacement-based. Clicking a related
object opens a new inspection pane to the right instead of replacing the current
object. Each pane owns:

- `object_id`
- `active_view_id`
- `available_views`
- `local_state`
- `outgoing_links`

This preserves exploration narratives such as:

```text
[Entry Point] → [Symbol] → [Repository] → [Decision] → [Test]
```

## Rationale

- The frontend is the only layer that knows visual state such as scroll, pane layout, selected nodes, and active local renderer state.
- The backend is the correct layer for semantic persistence, restore, sharing, and MCP access.
- A backend-only navigation stack would make the Explorer feel constrained and would overfit visual concerns into server-side state.
- A frontend-only stack would make explorations impossible to share, restore, or inspect through MCP.

## Alternatives Considered

- **Backend-only navigation stack:** rejected because it cannot naturally represent rich browser state and would make the Explorer less moldable.
- **Frontend-only navigation state:** rejected because MCP, restore, and shareable explorations need backend-readable semantic state.
- **Full state mirroring:** rejected because syncing every visual detail creates unnecessary coupling and fragility.

## Consequences

- The Explorer can evolve quickly without backend schema churn for every visual detail.
- Backend APIs need an `ExplorationSession` concept for semantic navigation events.
- MCP can list and inspect exploration paths, but not reproduce exact visual UI state.
- Browser-local state and persisted semantic state must have clear boundaries.
- The main Explorer UI should use a lateral pane stack similar to GtPager, not single-page replacement navigation.

## Validation

- [ ] User can drill from one object/view to another and go back visually.
- [ ] Clicking related objects opens new panes without losing the previous pane.
- [ ] User can restore a previous exploration path.
- [ ] User can share an exploration path with another Explorer session.
- [ ] MCP can inspect the semantic exploration path.
- [ ] Browser-only UI state is not required for backend restore.
