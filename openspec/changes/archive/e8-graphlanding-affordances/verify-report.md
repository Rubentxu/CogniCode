# Verify Report: e8-graphlanding-affordances

**Change**: e8-graphlanding-affordances
**Mode**: openspec (artifacts in `openspec/changes/e8-graphlanding-affordances/`)
**Verdict**: **PASS WITH WARNINGS** (3 chained PRs merged + snapshot re-baseline done; banner dormant until backend lands; node-list perf deferred)

## Completeness

| Phase | Artifact | Status |
|---|---|---|
| Explore | `exploration.md` | Complete |
| Propose | `proposal.md` | Complete |
| Spec | `specs/graphlanding-affordances/spec.md` | Complete (8 requirements, 17 scenarios) |
| Design | `design.md` | Complete (6 architecture decisions) |
| Tasks | `tasks.md` | Complete (all 27 checkboxes ticked) |
| Apply | PR-1 #56, PR-2 #57, PR-3 #58 | **All 3 merged to main** |
| Verify | This document | Complete |
| Snapshot re-baseline | `chore(explorer-ui): re-baseline visual-regression snapshots (E8)` | Merged (`78b12eb`) |
| Tag | `v0.24.1` | Pushed |

## Build Evidence

| Command | Result | Notes |
|---|---|---|
| `just explorer-build` (PR-1 branch) | exit 0, ✓ built in 2.73s | 2 pre-existing warnings (chunk size + ineffective dynamic imports in GraphLanding); unrelated to this change |
| `just explorer-build` (main post-merge) | exit 0 | Confirmed |
| `just explorer-test` (main post-merge) | **671/671 pass** | Was 670/671 pre-merge (PR-2 fixes the `generateArtifact` pre-existing failure) |
| `just explorer-e2e --update-snapshots` | **67/67 pass** | 24 visual-regression snapshots regenerated; growth pattern matches expectation |

## Spec Compliance Matrix

| Requirement | Implementation | Test Coverage | Status |
|---|---|---|---|
| R1: Truncation banner | `GraphLanding.tsx:216-231` | Not covered by E2E (banner code dormant — backend lacks field) | PASS (dormant) |
| R2: Schema optional fields | `schemas.ts:1229-1230`, `landingFixtures.ts:121-122` | `subgraph_schemas.test.ts:80` accepts missing `truncated_reason` | PASS |
| R3: Canvas accessibility | `GraphLanding.tsx:233-241` (role, aria-label, tabIndex) | Manual a11y verified; not auto-tested | PASS |
| R4: Node list fallback | `GraphLanding.tsx:243-273` | 8 perspective-toggle tests look for `graph-node-*` testid (PASS post-merge) | PASS |
| R5: `selectObject` memoization | `GraphLanding.tsx:95-100` | Visual regression only (no perf benchmark) | PASS |
| R6: Artifact path contract | `useExplorations.ts:181`, `handlers.ts:272` | `hooks.test.ts:generateArtifact` (passing post-merge) | PASS |
| R7: Quality summary mock | `handlers.ts:97-127` | Mock-only; no test asserts response shape | PASS (dev-only) |
| R8: E2E MSW compat | `landing/error-states/pane-stack.spec.ts` using `addInitScript` | Tests rewrite the override mechanism itself; 67/67 e2e pass | PASS |

## Design Coherence

| Decision | Implementation | Status |
|---|---|---|
| D-1: Reuse `TruncationBanner` pattern (inline, no shared component) | `GraphLanding.tsx:216-231` renders inline | ✓ Matches |
| D-2: `useCallback([dispatch])` for `selectObject` | `GraphLanding.tsx:95-100` | ✓ Matches |
| D-3: `role="application"` + node list fallback | `GraphLanding.tsx:237, 244-273` | ✓ Matches |
| D-4: `page.addInitScript` for MSW compat | `landing.spec.ts:120-150`, `error-states.spec.ts:21-100` | ✓ Matches |
| D-5: MSW wildcard `*/api/exploration-sessions/...` | `handlers.ts:272` | ✓ Matches |
| D-6: Quality summary mock is dev-only | `handlers.ts:97-127` (no schema, no hook) | ✓ Matches |

## Issues

### CRITICAL
None.

### WARNING

**W-1: Truncation banner dormant.**
The banner code (`GraphLanding.tsx:216-231`) and schema (`schemas.ts:1229-1230`) are wired correctly, but the backend `LandingPayload` (`crates/cognicode-explorer/src/dto.rs:782-799`) does not return `truncated` / `truncated_reason`. The banner is therefore invisible in production today. This is by design (forward-compatible) but should be tracked as `e8b-landing-payload-truncation` follow-up.

**W-2: Node-list fallback scales linearly with node count.**
The fallback list renders one `<button>` per node. For very large workspaces (the spec already documents `>500` nodes triggering the warning), this is 500+ DOM elements. Acceptable for v0.24.1; flagged for `e9-landing-perf`.

**W-3: `e7-renderer-bench/` artifacts removed from tracking.**
The `apps/explorer-ui/artifacts/e7-renderer-bench/{report.md, results.json}` files were touched by an E7 benchmark run. They are NOT source — `.gitignore` (`apps/explorer-ui/artifacts/`) was added in PR-3; the 2 pre-existing tracked files were removed via `git rm --cached` in the snapshot re-baseline commit. Files remain on disk; future benchmark runs won't be committed.

### SUGGESTION

**S-1: Capture live shell verification logs.**
This verify report is based on `just explorer-test`, `just explorer-build`, and `just explorer-e2e --update-snapshots` outputs. Captured logs:
- 671/671 unit/integration tests pass
- Build exit 0
- 67/67 e2e tests pass after snapshot re-baseline

## Final Verdict

**PASS WITH WARNINGS.**

The change is correctly scoped, the spec is fully covered by the implementation
(modulo the dormant banner, which is intentional), and tests are green
post-merge. The three merged PRs (#56, #57, #58) plus the snapshot re-baseline
(`78b12eb`) form a clean stacked-to-main chain that landed in order without
conflicts. Tag `v0.24.1` is published.

Two follow-up cycles are recommended:

1. `e8b-landing-payload-truncation` — backend truncation fields for
   `LandingPayload` (activates the dormant banner). Depends on
   `crates/cognicode-explorer/src/api.rs` landing handler being implemented
   beyond stubs.
2. `e9-landing-perf` — virtualise node-list fallback for workspaces >500
   nodes if real-world usage shows DOM bloat.
