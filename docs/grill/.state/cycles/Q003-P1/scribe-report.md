# Q003-P1 — Scribe Report

**Cycle:** Q003-P1
**Pass:** 3
**Category:** tooling
**Status:** modified
**Confidence:** high

## Question

What component library for the Explorer frontend?

## Proxy answer

Radix UI primitives + Tailwind CSS 4 + cmdk for Spotter. No shadcn/ui — fighting its opinions costs more than styling Radix directly.

## Skeptic challenge

1. Radix UI maintenance slowed post-WorkOS acquisition (2022). shadcn/ui already supports Base UI as alternative layer.
2. Miller Columns — the hardest component — has NO Radix primitive. Manual keyboard nav in cascading columns is the real complexity.
3. ~8 components total — "code judo" with aria attributes + Tailwind may be simpler than Radix dependency.
4. Tailwind 4 @theme works but CSS variables from prototype need mapping to `--color-*`, `--font-*` namespaces.
5. cmdk justified for Spotter (portal, focus trap, overlay, filtering).

## Judge decision: MODIFIED

**Final answer:** cmdk for Spotter (cmd+K overlay). Everything else — MillerColumn, ViewTabs, ListRow, CardGrid, Playground, ColumnHeader, Breadcrumb — uses direct ARIA attributes + Tailwind CSS 4. No direct Radix dependencies. cmdk handles its own Radix Dialog internally. ViewTabs ~60 lines following WAI-ARIA Tabs Pattern (APG).

**Why:** Only Spotter has genuine complexity (portal, focus trap, overlay, filtering). Everything else is simple enough for direct ARIA. Miller Columns are custom regardless — no library helps with cascading column keyboard navigation.

## Rejection trace

- **Rejection reason:** Proxy added full Radix UI dependency for ~8 components without per-component complexity analysis. Seven of eight components are simple enough for direct ARIA. Only Spotter (cmdk) has genuine complexity warranting a third-party dependency.
- **Remedy level:** `minimize_dependencies`
- **Remedy proposed:** Remove Radix UI. Keep cmdk only. Use direct ARIA + Tailwind for all other components.
- **What Proxy missed:** Failed to evaluate per-component complexity. Applied "component library" as default answer without counting which components actually need library primitives. Overlooked that Miller Columns have no Radix equivalent anyway — the hardest component was going to be custom regardless.
- **Proxy learning:** Before selecting a component library: (1) enumerate every component in the tree; (2) classify each as "standard ARIA pattern" or "genuinely complex"; (3) only add a dependency for the genuinely complex ones.

## Impact

| Area | Affected? |
|---|---|
| CONTEXT.md | yes — component architecture decision |
| ADR | yes — `DRAFT-explorer-component-architecture` |
| API | no |
| Persistence | no |
| Tests | yes — component tests per WAI-ARIA pattern |
| Security | no |
| Observability | no |

## Follow-up questions spawned

1. Should we spike Miller Columns keyboard navigation before committing to direct ARIA implementation?
2. Does cmdk's internal Radix Dialog dependency create any version conflict risk with React 19?

## Validation required

Yes. Miller Columns keyboard navigation needs a spike before implementation. The cascading-column focus model (arrow keys across columns, tab between interactive elements) has no WAI-ARIA precedent — must be validated with real users.

## Pattern contribution

This cycle established rejection pattern #3: **"Default library assumption without per-component analysis"** — selecting a component library as a project-level default rather than evaluating each component's complexity individually.

---

## Artifacts produced

| Artifact | Path | Action |
|---|---|---|
| Ledger entry | `docs/grill/.state/2026-06-07-explorer-frontend-gaps.ledger.md` | Appended |
| Working summary | `docs/grill/.state/2026-06-07-explorer-frontend-gaps.summary.md` | Updated |
| Cycle report | `docs/grill/.state/cycles/Q003-P1/scribe-report.md` | Created |
| ADR draft | `docs/adr/drafts/DRAFT-explorer-component-architecture.md` | Created |
