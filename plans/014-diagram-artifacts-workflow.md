# Plan 014: Make diagram artifacts persistent, regenerable, and visible

> **Executor instructions**: Follow this plan step by step and verify each
> stage before continuing.

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: MED
- **Depends on**: `plans/013-knowledge-views-pack.md`
- **Category**: direction
- **Planned at**: commit `a130d53b`, 2026-07-22

## Why this matters

CogniCode already generates diagrams, but they are still too close to exports.
This plan makes diagrams first-class artifacts that users can inspect, reopen,
and relate to ADRs, investigations, and source views.

## Current state

- `docs/adr/ADR-010-diagram-artifacts-as-persistent-views.md` defines diagrams
  as derived, persistent artifacts.
- Existing UI/code surfaces include `MermaidRenderer.tsx`, `ExportMenu.tsx`,
  C4 Mermaid utilities, and investigation artifacts.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cargo test -p cognicode-explorer --lib` | exit 0 |
| UI tests | `npm --prefix apps/explorer-ui test -- MermaidRenderer ExportMenu` | exit 0 |
| E2E | `npm --prefix apps/explorer-ui run test:e2e:functional -- --grep "artifact|export|diagram"` | exit 0 |

## Scope

**In scope**:
- `crates/cognicode-explorer/src/domain/trace_mermaid.rs`
- `crates/cognicode-explorer/src/domain/c4_mermaid.rs`
- `crates/cognicode-explorer/src/api.rs`
- `apps/explorer-ui/src/components/ExportMenu.tsx`
- `apps/explorer-ui/src/components/MermaidRenderer.tsx`
- investigation artifact wiring paths

**Out of scope**:
- external diagram editor as source of truth
- manual diagram authoring workflow

## Steps

1. Define persisted diagram artifact metadata and linkage.
2. Wire export/generate flows to persist artifacts, not only download them.
3. Add UI affordances to reopen and inspect persisted diagram artifacts.
4. Validate regeneration and trace-back to source views.

## Done criteria

- [ ] A diagram artifact has stable provenance metadata
- [ ] The user can reopen it from Explorer UI
- [ ] Export remains available
- [ ] Interaction tests pass

## STOP conditions

- Artifact persistence requires a schema change not yet covered by investigation
  storage
- UI cannot reopen artifacts without a broader pane-state rewrite
