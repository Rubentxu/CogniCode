# Architecture Decision Records

This directory holds the architecture decision records (ADRs) for the CogniCode workspace.

| Number | Title | Status | Date |
|--------|-------|--------|------|
| [ADR-001](./ADR-001-parked-crates.md) | Parked Crates — Activation Criterion | ACCEPTED | 2026-06-25 |
| [ADR-002](./ADR-002-moldable-exploration-parity-program.md) | Moldable Exploration Parity Program | PROPOSED | 2026-06-25 |
| [ADR-003](./ADR-003-diagram-representations.md) | Diagram Representations — draw.io as Derived View | PROPOSED | 2026-06-28 |
| [ADR-004](./ADR-004-c4-investigation-model.md) | C4 Investigation Model | PROPOSED | 2026-06-28 |
| [ADR-005](./ADR-005-investigation-mode.md) | Investigation Mode — Knowledge Artifacts | PROPOSED | 2026-06-28 |
| [ADR-006](./ADR-006-functional-gtoolkit-parity.md) | Functional GToolkit Parity through MoldQL, ViewSpecs | PROPOSED | 2026-07-02 |
| [ADR-007](./ADR-007-node-properties-graph-query-port.md) | node_properties() on GraphQueryPort | PROPOSED | 2026-07-03 |

## Format

Each ADR follows the standard structure:
- **Status**: PROPOSED / ACCEPTED / DEPRECATED / SUPERSEDED
- **Date**: ISO date of decision
- **Deciders**: who made the decision
- **Context**: why the decision is needed
- **Decision**: what was decided
- **Alternatives considered**: other options weighed
- **Consequences**: positive, negative, mitigations
- **References**: related ADRs, commits, engram obs

## Convention

- ADRs are numbered sequentially (`ADR-NNN-...md`)
- Filenames use kebab-case
- Status changes are recorded in-place (an accepted ADR is not re-numbered)
- Superseding ADRs reference the prior number in the References section
