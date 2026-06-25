# Architecture Decision Records

This directory holds the architecture decision records (ADRs) for the CogniCode workspace.

| Number | Title | Status | Date |
|--------|-------|--------|------|
| [ADR-001](./ADR-001-parked-crates.md) | Parked Crates — Activation Criterion | ACCEPTED | 2026-06-25 |

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
