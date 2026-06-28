# Archive Report: E18-1 Landing Workbench

**Date**: 2026-06-28
**Change**: `e18-1-landing-workbench`
**Milestone**: E18 (Moldable UX Foundation)
**Status**: **PASS_WITH_WARNINGS** (per user decision at SDDK budget exhaustion)
**PR**: https://github.com/Rubentxu/CogniCode/pull/76

## Cycle summary

| Phase | Status | Artifact |
|-------|--------|----------|
| Explore | success | `sddk/.../e18-1-landing-workbench/explore-report.md` |
| Propose | success | `sddk/.../e18-1-landing-workbench/proposal.md` |
| Spec | success | `sddk/.../e18-1-landing-workbench/spec.md` |
| Design | success | `sddk/.../e18-1-landing-workbench/design.md` |
| Tasks | success | `sddk/.../e18-1-landing-workbench/tasks.md` (3 stacked PRs) |
| Apply PR1 | success | `a703449` — state slice + entry-point types + Spotter kind |
| Apply PR2 | success | `3cf3443` — LandingWorkbench + 3 sections |
| Apply PR3 | success | `6422fe7` — Shell wiring + tests + cleanup |
| Apply correction 1 | success | `cd0dbc1` — default tab + spotterKind + C4 restore |
| Apply correction 2 | success | `dd04e89` — runtime initialState fix + strengthened E2E + C4 round-trip |
| Verify 1 | FAIL | 4 critical issues found |
| Verify 2 | FAIL | 1 critical issue remained |
| Verify 3 | PASS_WITH_WARNINGS | 1 UX bug + 36 stale snapshots documented |

## What landed

### State (PR1)
- `apps/explorer-ui/src/state/slices/landingWorkbench.ts` — new useReducer slice (39 lines)
- `apps/explorer-ui/src/components/LandingWorkbench/entryPointTypes.ts` — 5-type catalog (54 lines)
- `state/context.ts` — `landingWorkbench` field in AppState, `SET_LANDING_TAB` action, `spotterKind` field
- `state/slices/spotter.ts` — extended reducer to handle kind
- `state/slices/index.ts` — wiring updates

### Components (PR2)
- `apps/explorer-ui/src/components/LandingWorkbench/LandingWorkbench.tsx` — tabbed shell (155 lines)
- `apps/explorer-ui/src/components/LandingWorkbench/StartFromSection.tsx` — entry-point grid (80 lines)
- `apps/explorer-ui/src/components/LandingWorkbench/InvestigationsSection.tsx` — investigation templates (120 lines)
- `apps/explorer-ui/src/components/LandingWorkbench/ResumeSection.tsx` — recent explorations (58 lines)

### Wiring (PR3)
- `apps/explorer-ui/src/components/Shell.tsx` — swap GraphLanding → LandingWorkbench
- `apps/explorer-ui/src/components/Spotter.tsx` — seed kind filter from state
- `apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx` — doc comment only (5 lines, no code change)

### Tests
- `apps/explorer-ui/src/state/slices/landingWorkbench.test.ts` — 7 reducer tests
- `apps/explorer-ui/src/components/LandingWorkbench/LandingWorkbench.test.tsx` — 9 component tests
- `apps/explorer-ui/e2e/landing-workbench.spec.ts` — 4 E2E tests (4 tabs, entry-point click, Graph tab parity, C4 round-trip)
- `apps/explorer-ui/src/components/Shell.test.tsx` — updated for new testid

## Test results

| Suite | Result |
|-------|--------|
| TypeScript | ✅ PASS |
| Vitest (new) | 16/16 ✅ |
| Vitest (full) | 695/696 (1 pre-existing unrelated fail) |
| Playwright (new) | 4/4 ✅ |
| Playwright (regression) | 126 pass / 36 fail (documented follow-ups) |

## Documented follow-ups (post-merge hotfix)

1. **KindFilterChips UX bug** — `Spotter.tsx:346-348` returns null when no query results. Entry-point click opens Spotter but the kind chip is invisible. ~5-line fix.
2. **Stale visual-regression snapshots** (35 PNG baselines) — `view-tabs-coverage.spec.ts-snapshots/*.png` and `visual-regression.spec.ts` need regeneration due to default-tab change.
3. **suggestedQuestions.test.ts** — pre-existing `InspectableObjectType` route variant count mismatch. Unrelated to E18-1.

## Spec compliance

- COMPLIANT: 7 scenarios
- PARTIAL: 2 scenarios
- FAILING: 1 scenario (KindFilterChips UX bug — code correct, test exposes polish issue)
- UNTESTED: 6 scenarios (scope for future cycles; not blocking)

## ADR conformance

- **ADR-002 (Moldable Exploration Parity Program)**: ✅ respected — substrate already exists
- **ADR-005 (Investigation Mode)**: ✅ partially implemented — entry points + investigations sections lay the groundwork; full investigation entity is E21

## Lessons learned

1. **State slice + runtime initialState**: When adding a new state slice, both the slice's `initialXxxState` constant AND the runtime `initialState` object in `context.ts` must be updated. They are separate code paths. Future cycles should grep for all 3 places.
2. **SDDK correction cycles**: Max 2 iterations. Use them wisely — the runtime initialState bug should have been caught in cycle 1.
3. **Strengthened tests expose real bugs**: The strengthened Spotter kind chip test caught a UX bug that ghost assertions in cycle 1 missed. This is the value of verifying with realistic assertions.
4. **Visual-regression baselines need care**: Changing default landing tab affects all snapshots showing the landing. Regenerate baselines explicitly, not as a side effect.

## Next steps

1. Merge PR #76
2. Open follow-up PR for KindFilterChips fix (~5 lines)
3. Open follow-up PR for snapshot regeneration (~35 PNG files)
4. Proceed to E18-2 (Spotter intent) and E19-1 (C4 rename)