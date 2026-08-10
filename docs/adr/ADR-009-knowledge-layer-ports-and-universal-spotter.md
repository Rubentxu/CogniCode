# ADR-009: Knowledge layer ports and universal Spotter

**Status**: ACCEPTED (promoted 2026-08-10)
**Date**: 2026-07-22  
**Deciders**: User, OpenCode orchestrator  

## Context

CogniCode Explorer is already strong at code-centered exploration, but its
knowledge layer is still fragmented. The main gap is not another graph view;
it is the absence of first-class ports and UI flows for architecture knowledge:

- ADRs are not yet exposed through a dedicated repository/index surface.
- Docs and evidence are not fully discoverable through the same Spotter flow as
  symbols, scopes, investigations, and saved explorations.
- The roadmap already identifies `e13-wave2-universal-spotter` as blocked by
  missing `DocRepository`, ADR index, and evidence store ports.

That means CogniCode still behaves more like a strong code explorer than a
living moldable knowledge environment.

## Decision

We will introduce a first-class **knowledge layer** composed of:

1. `DocRepository` for inspectable documents.
2. `AdrRepository` or ADR index for decision artifacts.
3. `EvidenceStore` for evidence objects and evidence-backed navigation.
4. Universal Spotter wave 2 that exposes `doc`, `adr`, and `evidence`
   families through the same interaction model as existing object families.

### Product contract

The knowledge layer is not complete when the backend port exists. It is only
complete when all of the following are true:

- the object family is searchable in Spotter;
- the result is inspectable in `PaneInspector`;
- the object has at least one default useful view;
- the user can navigate from code to decision/doc/evidence and back;
- the interaction is validated through UI tests, not only service tests.

## Alternatives considered

### 1. Keep docs/ADRs/evidence as secondary metadata only

Rejected. This preserves current fragmentation and prevents CogniCode from
becoming a real knowledge environment.

### 2. Add LLM-only retrieval over markdown without typed ports

Rejected. That would produce weaker guarantees, poorer provenance, and fragile
UX integration.

### 3. Build Spotter support first and ports later

Rejected. Spotter would become another shallow adapter over implicit knowledge,
instead of a stable architecture capability.

## Consequences

### Positive

- Unblocks `e13-wave2-universal-spotter`.
- Makes knowledge objects first-class citizens of the moldable system.
- Enables later features such as `ConceptMap`, `DocCodeAlignment`, and richer
  `DecisionGraph`/`EvidencePack` experiences.

### Negative

- Introduces new repo/service surfaces to maintain.
- Requires schema, index, and UI work together instead of isolated slices.

### Mitigations

- Ship in waves: ports first, then Spotter, then object-specific views.
- Keep each family behind explicit affordances and tests.

## References

- [ADR-002](./ADR-002-moldable-exploration-parity-program.md)
- [ADR-005](./ADR-005-investigation-mode.md)
- [ADR-006](./ADR-006-functional-gtoolkit-parity.md)
- `docs/ROADMAP.md` — `e13-wave2-universal-spotter`

## Implementation Log

- **2026-08-10 (E31-C)**: Knowledge layer ports implemented in e13-wave2-knowledge-layer-ports (PR #226, v0.86.0). AdrInspector + Ladybug stubs + Spotter e2e green. DocRepository, AdrRepository, EvidenceStore ports all wired.
