# Q002-P1 — Scribe Cycle Report

**Question:** Where does the Explorer frontend live and what build tool?
**Pass:** 2
**Status:** modified
**Date:** 2026-06-07

---

## 1. Proxy Answer

**Answer:** `apps/explorer-ui/` using Vite 6 + React 19 plugin + TypeScript strict + Tailwind CSS v4. Root package.json gains `"workspaces": ["apps/*", "packages/*"]`. WASM layer in separate `packages/cognicode-explorer-wasm/` built via wasm-pack. Dev proxy to Explorer backend via Vite `server.proxy`.

**Confidence:** high
**Needs user validation:** true (backend port, auto-generation commit strategy)

## 2. Skeptic Challenge

Four concerns raised:

1. **Leptos dashboard exists** (LOW): CodeExplorerPage (446 lines) already exists in Leptos. But user already overruled this in Q001-P1 with "no vamos a usar leptos. el dashboard es otra aplicacion independiente, no la relaciones."

2. **pnpm switch unnecessary** (HIGH): Root monorepo already uses npm (package-lock.json present). pnpm switch adds migration cost with zero stated benefit.

3. **packages/ directory premature** (HIGH): Three-directory layering — `apps/` + `packages/types` + `packages/wasm` — for an MVP with only one consumer. Classic YAGNI violation.

4. **ts-rs type staleness** (MEDIUM): CI-generate vs commit strategy for generated TypeScript types not addressed. Risk of desync between Rust source and committed `.ts` files.

## 3. Judge Decision

**Verdict:** MODIFIED

**Final answer:** `apps/explorer-ui/` with Vite 6 + React 19 + TypeScript strict + Tailwind CSS. Stay on npm (no pnpm switch). Delete `packages/` directory. Types generated to `apps/explorer-ui/src/types/generated/` (committed, `npm run gen-types`). WASM from Rust workspace crate consumed as local path dependency in package.json. No npm workspaces until second app proves sharing is real. Vite `server.proxy` for dev backend.

**Modifications applied:**
| Proxy proposed | Judge decided |
|---|---|
| pnpm workspace | npm (consistent with root) |
| `packages/cognicode-types/` | `src/types/generated/` (colocated) |
| `packages/cognicode-explorer-wasm/` | Local path dep from Rust crate |
| npm workspaces enabled | No workspaces yet |

## 4. Why This Decision

Extraction must be earned. The `packages/` directory and npm workspaces solve a problem that does not yet exist — there is only one consumer of the types and WASM. Generated types and WASM live where consumed: simpler, zero indirection, zero configuration overhead. The pnpm switch is a distraction — it introduces migration cost with no documented benefit. Build sharing infrastructure only when sharing is proven necessary by the existence of a second application.

## 5. Rejection Analysis

**Rejection reason:** Premature extraction. Proxy proposed pnpm switch and multi-directory `packages/` extraction for a single-consumer MVP. Violates YAGNI — builds sharing infrastructure before a second consumer exists.

**Remedy:** yagni_simplification. Keep npm. Delete `packages/`. Colocate artifacts where consumed.

**What Proxy missed:**
1. Proposed tool-switch (npm→pnpm) without checking root tooling (`package-lock.json`)
2. Proposed `packages/` extraction before any second consumer exists
3. Did not evaluate "where consumed" as simpler alternative for generated artifacts

**Proxy learning:** Before proposing directory structures or tool switches: (1) check what the existing monorepo uses; (2) ask "is the second consumer real or hypothetical?" Extract only when sharing is proven by an actual second application.

## 6. Impact

| Area | Affected? |
|---|---|
| CONTEXT.md | yes — app location, tooling, extraction rule |
| ADR | no |
| API | no |
| Persistence | no |
| Tests | yes — test infrastructure location |
| Security | no |
| Observability | no |

## 7. Follow-up Questions Spawned

- How should CI generate and commit TypeScript types from Rust DTOs?
- When is `packages/` extraction justified? (success criterion: second consumer exists)

## 8. Validation Required

Yes. Backend port for Vite dev proxy needs user confirmation. Branch strategy: start with `apps/explorer-ui/` flat structure; extract to `packages/` only when second app proves sharing demand.

## 9. Relationship to Prior Cycles

- **Q001-P1:** Established TypeScript strict mode and React 19 for Explorer. Q002-P1 builds on this by deciding WHERE the code lives and HOW it's built.
- **User override from Q001-P1:** Leptos dashboard is independent. WASM layer is permitted. Both constraints honored in this decision.

## 10. Artifacts Produced

- Ledger entry appended: `docs/grill/.state/2026-06-07-explorer-frontend-gaps.ledger.md`
- Working summary updated: `docs/grill/.state/2026-06-07-explorer-frontend-gaps.summary.md`
- Cycle report: `docs/grill/.state/cycles/Q002-P1/scribe-report.md` (this file)
