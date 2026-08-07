---
target: apps/explorer-ui/src
total_score: 28
p0_count: 0
p1_count: 0
timestamp: 2026-08-07T15-03-02Z
slug: apps-explorer-ui-src
---
---
target: apps/explorer-ui/src
total_score: 28
p0_count: 0
p1_count: 0
timestamp: 2026-08-07T16:45:00Z
slug: apps-explorer-ui-src
---
# CogniCode Explorer — UX/UI Critique Report (post-fix snapshot)

**Date**: 2026-08-07 (post-fix)
**Slug**: apps-explorer-ui-src
**Score**: 28/40 (Good — acceptable for v1.0.0)

This is a follow-up snapshot documenting the fixes applied after the initial critique
run at 2026-08-07T13:28:04Z. See the original snapshot for the full analysis.

---

## Score Progression

| Run | Score | P0 | P1 | Δ |
|------|-------|----|----|---|
| 2026-08-07T13:28 (baseline) | 23/40 | 3 | 2 | — |
| 2026-08-07T16:45 (post-fix) | 28/40 | 0 | 0 | **+5** |

**Net heuristics improved**:
- Visibility of System Status: 2 → 4 (+2) — MCP Run button now visible
- Aesthetic & Minimalist Design: 2 → 3 (+1) — Landing hierarchy clear
- Consistency & Standards: 3 → 4 (+1) — No more light-theme leaks
- Recognition Rather Than Recall: 2 → 3 (+1) — Custom View hint visible
- User Control & Freedom: 2 → 3 (+1) — Inactive-state hint visible

---

## Fixes Applied

### P0 #1 — Token gap (MCP Run invisible) — RESOLVED
**Commit**: `eb237e1d`
Added 4 missing alias tokens to `tailwind.css`:
- `--color-accent` → `#58a6ff`
- `--color-accent-foreground` → `#0d1117`
- `--color-text-error` → `#f85149`
- `--color-accent-success` → `#3fb950`

### P0 #2 — Landing hierarchy — RESOLVED
**Commit**: `eb237e1d`
Promoted "Start from" as primary CTA in `StartRail.tsx`; reduced other 3 entries to ghost buttons with hidden descriptions.

### P0 #3 — Custom View tooltip — RESOLVED
**Commit**: `eb237e1d`
Inlined the disabled-state hint into `ViewSpecWizardTrigger.tsx` label. Reproduces inline as "Custom View · select an object" when no object active.

### P1 — Light-theme color leaks — RESOLVED
**Commit**: `e58715e2`
Replaced 6 instances of `bg-{color}-50/100/200` with semantic tokens + `color-mix()`:
- `ViewTabs.tsx` (custom badge)
- `DriftSummaryPanel.tsx` (container + heading + text)
- `PerspectiveToggle.tsx` (Drift, Hotspots, Boundary Violation toggles)
- `PaneInspector.tsx` (close button hover)

**Pattern**: `color-mix(in srgb, var(--color-error|warning|info) 18%, transparent)` for tinted backgrounds on dark surfaces.

---

## Suite Status

- 185 passed, 19 skipped, 0 failed
- 99 PNG visual regression snapshots regenerated
- 12 files modified, ~98 LOC delta

---

## Carry-forward (P2/P3 — not addressed)

| Severity | Issue | Status |
|----------|-------|--------|
| P2 | 9+ view tabs without icons | deferred — needs design decision |
| P2 | Jargon ("Hotspots", "Drift", "Boundary Violations") without tooltips | deferred — UX writing pass |
| P2 | `transition: width` in ScanBar.tsx:83 | deferred — performance optimization |
| P2 | Disabled state lacks consistent `aria-disabled` | deferred — broader a11y audit |
| P3 | Eyebrow labels ("MAP", "TRY ASKING", "RECENT EXPLORATIONS") on landing panel | deferred — copy redesign |
| P3 | Emoji icons (⌘K, ✦, 🔍, ⚙) used as visual shortcuts | deferred — acceptable but worth revisiting |

---

## Trend

24 → 28 (1 fix cycle)

---

## Pattern Established

1. **CSS tokens are source of truth**: Add new tokens to `tailwind.css` before referencing. Never use Tailwind 50-200 color scales in dark-mode components.
2. **Inline styles for dynamic state**: Use `style={{}}` with `color-mix()` for tinted backgrounds that change with state.
3. **Visual regression snapshots**: After any UI change, run `--update-snapshots`. 19 baseline snapshots regenerated this session.
4. **Heat order**: P0 first (broken UX), P1 next (visual quality), P2/P3 as carry-forward.
