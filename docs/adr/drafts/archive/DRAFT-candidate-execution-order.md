# DRAFT: 5-Wave Candidate Execution Order

**Status:** DRAFT — requires human review before promotion to numbered ADR
**Date:** 2026-06-11
**Source:** auto-grill-loop Q007-P1, Q013-P2

## Context
Six architectural deepening candidates were identified in the CogniCode codebase. They share files (`handlers/mod.rs`, `dto/file_ops.rs`, `rmcp_adapter.rs`), creating merge conflict risk if executed in parallel without ordering.

## Decision
Execute in 5 waves with automated CI gating per wave:

| Wave | Candidates | Files | ΔLines | Gate |
|------|-----------|-------|--------|------|
| 1 | C3+C5+C6 | 5+2+15 | ~500 | Test suite + bench <5% regression |
| 2 | C2 Builder | 1 | ~150 | Coexistence tests + `#[deprecated]` count=0 |
| 3 | C4 Unification | 5+ | ~500 | Trybuild tests + DTO migration test |
| 4 | C2 Deletion | 1 | ~50 | Dead-code lint clean |
| 5 | C1 Tool Registry | 2+ | ~200 | Integration suite + tool count match |

## Rationale
- **C4 gates C1**: Q001-P1 confirmed C1 compiles against handler signatures that leak DTO types; C4 must clean the boundary first
- **Wave 1 parallel candidates**: C3, C5, C6 touch completely disjoint files — zero merge conflict risk
- **Per-wave CI gates**: Automated verification prevents wave bleed and enables independent rollback

## Consequences
- C1 (highest impact candidate) is Wave 5 — must wait for C4
- C4 elevated from "Vale la pena explorar" to critical path dependency
- Per-wave CI checks must be configured before each wave begins

## Alternatives Considered
- **Big-bang all-at-once:** rejected — merge conflicts, unreviewable PR, impossible to rollback
- **C1-first (skip C4):** rejected — Q001-P1 proved C1 compilation depends on schema/DTO boundary cleanliness
- **Manual per-wave verification:** rejected — error-prone; automated CI gates provide objective completion criteria

## Validation
- [ ] Wave 1 CI gates configured and passing
- [ ] Each wave merges independently (no wave N+1 work on wave N branch)
- [ ] Rollback tested: reverting any wave does not break earlier waves
