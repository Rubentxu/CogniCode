# ADR-012: UI-visible capability contract and interaction validation

**Status**: ACCEPTED (promoted 2026-08-10)
**Date**: 2026-07-22  
**Deciders**: User, OpenCode orchestrator  

## Context

CogniCode often lands backend and runtime capabilities before the Explorer UI
fully exposes them. That creates a recurring gap:

- the code says a capability exists;
- the registry may even expose it;
- but the user cannot clearly discover, inspect, or complete the flow in the
  graphical interface.

For a moldable-development product, this is not a minor UX problem. It breaks
the product contract.

## Decision

Every new moldable capability must satisfy a **UI-visible capability contract**
before it is considered done.

### Required completion gates

1. **Discoverable**
   - available through Spotter, affordances, or another explicit UI entry path.
2. **Inspectable**
   - renderable inside Explorer panes with correct identity and navigation.
3. **Usable**
   - the user can perform the primary interaction without MCP or direct API use.
4. **Validated**
   - backed by user-interaction tests (Vitest and/or Playwright) for the happy
     path and key edge states.

### Required documentation

Each feature plan and roadmap item must describe:

- the UI entry point;
- the visible artifact or pane result;
- the test path that proves the interaction;
- what remains intentionally out of scope.

## Alternatives considered

### 1. Keep backend completion as the default definition of done

Rejected. This repeatedly creates invisible or half-visible capabilities.

### 2. Treat UI work as optional polish after architecture work

Rejected. In CogniCode, the Explorer UI is the main product surface.

### 3. Validate only at unit/service level

Rejected. That does not prove moldable interaction from the user's point of
view.

## Consequences

### Positive

- Forces product coherence.
- Prevents backend-only “done” illusions.
- Makes roadmap items more honest and reviewable.

### Negative

- Raises delivery cost for every feature.
- Requires tighter coordination between Rust and React work.

### Mitigations

- Use small, explicit interaction contracts.
- Reuse E2E infrastructure and affordance-driven test entry points.

## References

- [ADR-005](./ADR-005-investigation-mode.md)
- [ADR-006](./ADR-006-functional-gtoolkit-parity.md)
- `apps/explorer-ui/e2e/*`

## Implementation Log

- **2026-08-10 (E31-C)**: UI visible capability contract implemented. The Explorer UI exposes the spotter, lens panel, view spec wizard, and onboarding flow as first-class capabilities.
