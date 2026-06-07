# Q001-P1 Cycle Report — Scribe

**Cycle ID:** Q001-P1
**Date:** 2026-06-07
**Pass:** 1
**Status:** accepted (user-overridden)

---

## Question

TypeScript or JavaScript for the Explorer frontend?

## Classification

- **Category:** tooling
- **Priority:** critical
- **Confidence:** high (after user override)

---

## Proxy Answer

TypeScript strict mode (`strict: true`, strictNullChecks, noImplicitAny). `.tsx` for components, `.ts` for utilities. Domain types generated from Rust DTOs via `ts-rs` into shared `@cognicode/types` package. Prototype (478 lines vanilla JS) is spec only — clean rewrite, not migration. Confidence: high.

**What Proxy missed:** Did not acknowledge existing Leptos dashboard upfront. Technology answer was correct but reasoning should have started by checking the workspace and explaining why a separate React stack is intentional rather than accidental.

---

## Skeptic Challenge

**Intensity:** HIGH

Six architectural challenges raised:

| # | Challenge | Severity |
|---|-----------|----------|
| 1 | ARCHITECTURAL FORK: Leptos dashboard with 18 pages, 17 components, 61 tests exists | HIGH |
| 2 | ts-rs fragility: Community crate, 5 contributors, complex enum codegen may break | HIGH |
| 3 | Build pipeline doubling: cargo → ts-rs → npm → tsc → vite vs single cargo build | HIGH |
| 4 | Prototype already in Leptos: CodeExplorerPage proves interaction model in Rust | HIGH |
| 5 | TypeScript strict mode unnecessary: Rust's type system already strict | MEDIUM |
| 6 | MCP boundary: TypeScript types benefit only web UI, not MCP consumers | MEDIUM |

**Suggested correction:** Extend existing Leptos dashboard with new explorer pages.

---

## Judge Decision

### Initial verdict

**REJECTED** — Judge agreed with Skeptic that extending Leptos was architecturally preferable (zero new build steps, zero new types, zero technology fork).

### User override

> "no vamos a usar leptos. el dashboard es otra aplicacion independiente, no la relaciones"

### Final verdict

**ACCEPTED** with user clarification. React 19 + TypeScript strict mode. Dashboard is independent — no code sharing, no Leptos coupling.

### Rationale

User explicitly rejected extending Leptos dashboard. Explorer frontend is a standalone React application as intended by ADR 0009. Dashboard and Explorer are separate products.

---

## Decision

**TypeScript strict mode for Explorer frontend.** React 19 + Tailwind CSS (ADR 0009). Clean rewrite from prototype. Dashboard (Leptos) is a separate application — no code sharing, no technology coupling between them.

---

## Impact Assessment

| Area | Impacted |
|------|----------|
| CONTEXT.md | Yes — must document separate stacks |
| ADR | Yes — new draft created |
| API | No |
| Persistence | No |
| Tests | Yes — Playwright tests in TypeScript |
| Security | No |
| Observability | No |

---

## Artifacts Produced

- **Ledger entry:** `docs/grill/.state/2026-06-07-explorer-frontend-gaps.ledger.md`
- **Working summary:** `docs/grill/.state/2026-06-07-explorer-frontend-gaps.summary.md`
- **ADR draft:** `docs/adr/drafts/DRAFT-explorer-frontend-technology.md`

---

## Follow-up Questions

1. How should shared domain types flow from Rust DTOs to TypeScript frontend without ts-rs fragility?
2. Should Explorer share any infrastructure (auth, API client, deployment pipeline) with Dashboard?

---

## Proxy Learning

Before answering technology-selection questions, ALWAYS search workspace for existing implementations. The Proxy's answer was correct, but the reasoning should have started by:

1. Checking what already exists (`glob`, `grep` for frontend stacks)
2. Acknowledging the Leptos dashboard upfront
3. Explaining why a separate React stack is intentional (per ADR 0009, user's product separation)
4. Then presenting the TypeScript strict mode recommendation

---

## Validation Required

User override decision should be documented prominently to prevent future contributors from attempting to merge the two stacks. ADR draft serves this purpose.
