---
target: apps/explorer-ui/src
total_score: 23
p0_count: 3
p1_count: 2
timestamp: 2026-08-07T13-28-04Z
slug: apps-explorer-ui-src
---
# CogniCode Explorer — UX/UI Critique Report

**Target**: `apps/explorer-ui/src` (TypeScript/React app, 1440×900 desktop, Vite + MSW mocks)
**Date**: 2026-08-07
**Slug**: apps-explorer-ui-src
**Score**: 23/40 (Acceptable → significant improvements needed)

---

## Design Health Score (Nielsen's 10 Heuristics)

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 2 | MCP "Run" button invisible — primary action disappears; connection status shown but secondary signals scattered |
| 2 | Match System / Real World | 3 | Most labels use domain language well (graph, edge, perspective); some jargon leaks in "Quality", "MCP", "Hotspots" |
| 3 | User Control and Freedom | 2 | Modal Escape works; no undo/redo on evidence pinning; "Custom View" disabled with no tooltip explaining why |
| 4 | Consistency and Standards | 3 | Strong consistency in panes, tabs, headers; inconsistent pill/badge sizes across panels (Drift/Workspace Quality/Workspace) |
| 5 | Error Prevention | 2 | MCP modal Run button looks enabled with no path validation; no confirmation on destructive actions |
| 6 | Recognition Rather Than Recall | 2 | Power features buried: `Open in draw.io`, `Download PNG/SVG`, `Pin evidence` — discoverable only by hover exploration |
| 7 | Flexibility and Efficiency of Use | 3 | Cmd-K Spotter is excellent; keyboard nav mostly works; few bulk actions |
| 8 | Aesthetic and Minimalist Design | 2 | Workbench landing is cluttered: 3 entry tiles + 4 mini-tabs + 3 sections of metrics competing for attention |
| 9 | Help Users Recognize/Diagnose/Recover from Errors | 2 | Error states exist (LoadingTier) but error messages are generic; no recovery suggestions inline |
| 10 | Help and Documentation | 2 | MCP tool descriptions shown inline (good); no global help center, no tooltips on most buttons |
| **Total** | | **23/40** | **Acceptable** — Solid foundation, but several P0/P1 gaps before v1.0 |

---

## AI Slop Verdict

**Verdict**: NOT OBVIOUSLY AI-GENERATED. The UI has restraint — no glassmorphism, no gradient text, no numbered eyebrows, no identical card grids. **However**:
- The dark theme follows the "tinted neutrals" pattern (#0d1117 GitHub dark) which is a recognizable 2024-2026 SaaS reflex
- The workbench landing's "4 entry tiles + recent explorations + quality" composition echoes Linear/Vercel/Notion design vocabulary
- Tool icons (⌘K, ✦, 🔍, ⚙) lean on emoji as a shortcut, common in AI-generated UI

**Risk**: Low. The UI feels engineered, not generated. Watch for: don't add decorative chrome, don't pile on badges, don't introduce gradient hero text.

---

## Anti-Patterns Detected (Detector + Manual)

| Severity | Pattern | Location | Count |
|----------|---------|----------|-------|
| **P0** | `--color-accent` referenced but undefined | `McpToolsModal.tsx`, `PinEvidenceModal.tsx`, `LensSidebarToggle.tsx`, `ViewSpecWizardTrigger.tsx`, `LandingWorkbench/StartFromSection.tsx` | **5 components** |
| **P0** | `--color-text-error` referenced but undefined | `McpToolsModal.tsx`, `PinEvidenceModal.tsx` | 2 |
| **P0** | `--color-accent-success` referenced but undefined | `PinEvidenceModal.tsx` | 1 |
| **P1** | Light-theme classes leaked in dark UI | `bg-red-50`, `bg-blue-100`, `text-blue-700`, `bg-red-200` in `ViewTabs.tsx`, `DriftSummaryPanel.tsx`, `PerspectiveToggle.tsx` | 5 instances |
| **P2** | `transition: width` in ScanBar (layout property animation → jank) | `components/ScanBar.tsx:83` | 1 (detector caught) |
| **P2** | Disabled "Custom View" button with no explanation tooltip | `Shell.tsx` TopBar | 1 |

**Visual evidence (05-mcp-modal.png)**: The "Run" button in the MCP Tools modal is **barely visible** — gray text on gray background. This is the runtime symptom of the `--color-accent` token gap. Users literally cannot see the primary CTA.

---

## UAT Workflows — Analysis

### 🏁 New user landing → first object

**Verdict**: ⚠️ **Too many entry paths compete for attention**

The landing (01-landing.png) presents:
- 4 entry tabs (Start from / Investigations / Recent / Graph)
- A "Map" panel with a placeholder graph ("Step back to the broadest system view")
- A "What modules have the most dependencies?" suggestion
- Recent explorations list
- Workspace Quality panel with 3 metrics

A first-time user faces **5+ visible options** simultaneously. Per cognitive load rules (≤4 items in working memory), this breaches the threshold.

**Fix**: Default to ONE entry path. Show the other 3 only after first interaction, or condense to a single hero CTA + secondary options.

---

### 🔍 Spotter discovery (Cmd-K)

**Verdict**: ✅ **Best UX surface in the app**

- Discoverable: Cmd-K is hinted via `⌘K` badge on the button label
- Fast: keyboard-first, with arrow navigation
- Multi-family: shows symbols, ADRs, files, rules, quality issues — a real "Spotter" not just a search
- Empty state: provides placeholder hints

**Issues**:
- No first-time hint/tooltip explaining "Cmd-K is the universal launcher"
- The "✦ Custom View" suggestion is disabled with no explanation

---

### 🔀 Perspective switching (Graph ↔ C4)

**Verdict**: ✅ **Good but visually muted**

The perspective toggle (Graph/Context/Container/Component/Code + Drift/Hotspots/Boundary Violations) is well-positioned but:
- All 5 perspective levels appear equally weighted — no visual cue for which is "default"
- C4 Container/Component/Code show "basic" badge that's never explained
- The active state is subtle (border + bg) — easy to miss

**Fix**: Active state needs more weight. Use a clear filled vs ghost treatment.

---

### 📋 Object inspection (view tabs)

**Verdict**: ⚠️ **View tabs are a "hidden masterpiece"**

The Object Inspector has 9+ view tabs (overview, source, call-graph, evidence, ownership-map, quality, test-slice, debug-slice, change-impact-story, usage-examples). This is CogniCode's core value prop but:
- Tabs are not labeled with icons — pure text, easy to overlook
- No descriptions or previews on hover
- Tabs spill horizontally without scrolling indicator

**Fix**: Add icon + label. Show preview thumbnails on hover. Consider dropdown for >7 tabs.

---

### 🪄 ViewSpec authoring (6-step wizard)

**Verdict**: ⚠️ **Strong flow, but custom view trigger buried**

- Wizard itself is well-structured (5 → now 6 steps with Scaffold)
- BUT: The "Custom View" button in TopBar is disabled with no explanation — looks broken to first-timers
- Trigger is also available in Object Inspector context menu (less discoverable)

**Fix**: Either enable Custom View by default OR add tooltip: "Select an object first to create a custom view."

---

### 🔧 MCP tools modal

**Verdict**: ❌ **Broken primary CTA**

**Visual proof (05-mcp-modal.png)**: The "Run" button at the bottom is **invisible** — gray text on gray background. This is the runtime symptom of `--color-accent` being undefined in `tailwind.css`. Users literally cannot tell if the button is enabled or what color it is.

This is a **P0** — the entire MCP tools feature is unreachable.

Other issues:
- Tool selector (`ingest_openapi`, `trace_route`, etc.) shows one at a time with no grouping by family
- 13 MCP tools — too many to scroll through linearly
- Result panel only shows raw JSON output, no structured rendering

---

### 📱 Mobile / responsive

**Verdict**: ⚠️ **Bottom-sheet exists but UX is incomplete**

- `responsive-mainflow-small-chromium-linux.png` (16 KB) shows the shell adapts
- Mobile bottom-sheet for spotter is implemented
- BUT: 320px viewport is `test.fixme` (known broken)

**Fix**: Either fix mobile properly or hide the app on mobile with a "Use desktop" message.

---

## Persona Red Flags

### 🏃 Alex (Power User)
- ✅ Cmd-K for everything
- ✅ Keyboard navigation works (Spotter, perspective toggle)
- ❌ No bulk actions (can't pin multiple evidence at once, can't batch-create views)
- ❌ Disabled "Custom View" button with no tooltip = looks like a bug
- ❌ No way to save/share complex state across sessions cleanly

### 🆕 Jordan (First-Timer)
- ❌ Landing has 5+ competing entry points
- ❌ "Custom View" disabled with no explanation
- ❌ No first-run hint for Spotter (Cmd-K not taught)
- ❌ "Quality" jargon everywhere without onboarding
- ❌ Tooltips missing on most actionable buttons

### ♿ Sam (Accessibility)
- ✅ Cmd-K keyboard accessible
- ✅ Most buttons have aria-labels
- ❌ Disabled state communicates via opacity only (needs `aria-disabled`)
- ❌ Light-theme colors (`bg-red-50`, `text-blue-700`) likely fail WCAG AA contrast on dark backgrounds
- ❌ Focus indicators on the perspective toggle are subtle

---

## Cognitive Load Score

8-item checklist:
- [ ] **Single focus**: ❌ Landing shows 5+ paths simultaneously
- [x] **Chunking**: ✅ Tabs use ≤4 items per group
- [x] **Grouping**: ✅ Panels grouped logically (header, body, footer)
- [ ] **Visual hierarchy**: ❌ All Workbench entries equal weight
- [x] **One thing at a time**: ✅ Modals focus on single tasks
- [ ] **Minimal choices**: ❌ 9+ view tabs visible at once in inspector
- [x] **Working memory**: ✅ Recent explorations surface context
- [ ] **Progressive disclosure**: ❌ Workbench shows everything upfront

**Failed: 4/8 = HIGH cognitive load (critical fix needed)**

---

## Priority Issues (ordered)

### 🔴 P0 — Token gap breaks MCP CTA
**What**: `--color-accent`, `--color-text-error`, `--color-accent-success` are referenced by 8+ components but never defined in `tailwind.css`. The MCP modal's "Run" button is **invisible**.
**Why**: The MCP tools feature — a core selling point — is literally unusable. Users will think the app is broken.
**Fix**: Add the three tokens to `src/tailwind.css`:
```css
--color-accent: #58a6ff;          /* = --color-primary */
--color-text-error: #f85149;       /* = --color-error */
--color-accent-success: #3fb950;  /* = --color-success */
```
**Command**: `/impeccable harden`

### 🔴 P0 — Visual hierarchy on landing is flat
**What**: Workbench landing presents 4 entry tiles + recent + quality + suggestion + map all at once.
**Why**: First-time users don't know where to start; choice paralysis.
**Fix**: Default to ONE primary CTA ("Open a saved exploration" or "Start from an object"), hide the rest behind a "More options" toggle.
**Command**: `/impeccable layout`

### 🟠 P1 — Light-theme color leakage in dark UI
**What**: `bg-red-50`, `bg-blue-100`, `text-blue-700`, `bg-red-200` in `ViewTabs.tsx`, `DriftSummaryPanel.tsx`, `PerspectiveToggle.tsx`.
**Why**: Light colors fail WCAG contrast on dark backgrounds; visible as muddy/gray patches; breaks design system cohesion.
**Fix**: Replace with tokens (`var(--color-error-bg)`, etc.) or dark-mode-tailwind equivalents (`bg-red-950`, `text-red-300`).
**Command**: `/impeccable colorize`

### 🟠 P1 — Disabled "Custom View" button is unexplained
**What**: TopBar shows "✦ Custom View" as a disabled button. Users see this every session and don't know why it won't activate.
**Why**: Creates perception of broken app; users may think they need to do something else first.
**Fix**: Add `title="Select an object first to create a custom view"` tooltip, or change to a contextual action that says "Create custom view from current selection."
**Command**: `/impeccable clarify`

### 🟡 P2 — View tabs are text-only with no icons
**What**: 9+ view tabs in Object Inspector show only text labels (e.g., "Change Impact Story", "Ownership Map").
**Why**: Long text labels hard to scan; tabs feel like raw data instead of tools; users miss powerful features.
**Fix**: Add icon + label per tab; consider dropdown for >7 tabs.
**Command**: `/impeccable layout`

### 🟡 P2 — Quality/severity jargon leaks everywhere
**What**: "Workspace Quality", "Drift", "Hotspots", "Boundary Violations", "Severity", "MCP" used without context.
**Why**: First-time users don't know what these mean; breaks discoverability.
**Fix**: Either add tooltips with one-line explanations, or simplify labels ("Code drift" instead of "Drift", "Health issues" instead of "Hotspots").
**Command**: `/impeccable clarify`

---

## Minor Observations

- `transition: width` in ScanBar.tsx:83 — layout animation, can be janky; should use `transform: scaleX()` instead
- Persisted settings use abbreviations ("🛡 8" for "8 quality issues") — emoji is unclear at a glance
- Spotter result rows show colored dots to indicate family (good!) but colors may not be distinguishable for color-blind users
- No keyboard shortcut hint anywhere outside the Cmd-K badge

---

## Questions to Consider

- Is the goal of the landing page to **showcase** (impress with features) or to **onboard** (guide to first action)? Currently it's neither.
- Should "Custom View" be a first-class action or a contextual one? Right now it's both — and disabled.
- Would a "tour mode" overlay (3-second interactive walkthrough) replace the need for tooltips everywhere?
- What if MCP tools were integrated INTO the inspector (context menu) instead of a separate modal? Would discoverability improve?

---

## Trend

23 → 28 (1 fix cycle, +5 points)

Wrote `.impeccable/critique/2026-08-07T15-03-02Z__apps-explorer-ui-src.md` (post-fix snapshot).

---

## Suggested Next Steps (in order)

1. **`/impeccable harden`** — Add the 3 missing CSS tokens (P0 fix). Quick, surgical, unblocks the MCP feature.
2. **`/impeccable layout`** — Fix the landing page hierarchy. Single hero CTA, hide the rest.
3. **`/impeccable colorize`** — Replace light-theme color leaks with dark-mode equivalents.
4. **`/impeccable clarify`** — Tooltips on disabled buttons, glossary for jargon.
5. **`/impeccable polish`** — Final pass on typography, spacing, motion.

---

## Resolution — Fixes Applied (2026-08-07)

### P0 #1 — Token gap (MCP Run invisible) — RESOLVED

**Commit**: `eb237e1d`

Added 4 missing alias tokens to `apps/explorer-ui/src/tailwind.css`:
```css
--color-accent: #58a6ff;              /* = --color-primary */
--color-accent-foreground: #0d1117;  /* = --color-primary-foreground */
--color-text-error: #f85149;          /* = --color-error */
--color-accent-success: #3fb950;      /* = --color-success */
```

**Verification**: MCP modal Run button now `rgb(88, 166, 255)` (previously gray-on-gray). All 13 mcp-tools-modal tests pass. Visual: `/tmp/critique/08-mcp-modal-with-path.png`.

**Heuristic improvement**: Visibility of system status 2 → 4.

---

### P0 #2 — Landing hierarchy — RESOLVED

**Commit**: `eb237e1d`

`apps/explorer-ui/src/components/StartRail.tsx`: promoted "Start from" as primary CTA (border + bg + description visible), reduced Investigations/Recent/Graph to ghost buttons (transparent bg, no border, hide description).

**Verification**: Visual confirms hierarchy readable at first glance. Visual: `/tmp/critique/landing-hierarchy.png`.

**Heuristic improvement**: Aesthetic +1, Recognition +1.

---

### P0 #3 — Custom View tooltip — RESOLVED

**Commit**: `eb237e1d`

`apps/explorer-ui/src/components/ViewSpecWizardTrigger.tsx`: inlined the disabled-state hint into the button label. "Custom View" → "Custom View · select an object" when `activeObjectId === null`. `title` attribute retained for hover; `aria-label` updated to "Custom view — select an object first" for screen readers.

**Verification**: Visual: `/tmp/critique/header-custom-view.png`.

**Heuristic improvement**: User Control +1.

---

### P1 — Light-theme color leaks — RESOLVED

**Commit**: `e58715e2`

Replaced 6 light-theme Tailwind classes with semantic dark-mode tokens:

| File | Before | After |
|------|--------|-------|
| `ViewTabs.tsx` (custom badge) | `bg-blue-100 text-blue-700` | `color-mix(var(--color-primary) 18%, transparent)` |
| `DriftSummaryPanel.tsx` (container) | `border-red-200 bg-red-50` | `color-mix(var(--color-error) 10%, transparent)` + 30% border |
| `DriftSummaryPanel.tsx` (heading) | `text-red-700` | `var(--color-error)` |
| `DriftSummaryPanel.tsx` (text) | `text-gray-700` | `var(--color-text-primary)` |
| `PerspectiveToggle.tsx` (Drift) | `bg-red-50 border-red-300 text-red-700` | `color-mix(var(--color-error) 18%, transparent)` + 50% border |
| `PerspectiveToggle.tsx` (Hotspots) | `bg-orange-50 border-orange-300 text-orange-700` | `color-mix(var(--color-warning) 18%, transparent)` |
| `PerspectiveToggle.tsx` (Boundary) | `bg-blue-50 border-blue-300 text-blue-700` | `color-mix(var(--color-info) 18%, transparent)` |
| `PaneInspector.tsx` (close hover) | `hover:bg-red-100` | removed (was light theme) |

**Pattern established**: dark theme uses semantic tokens (`--color-error`, `--color-warning`, `--color-info`) with `color-mix(in srgb, var(--color-X) NN%, transparent)` for backgrounds. **Avoid** Tailwind `bg-{color}-{50,100,200}` — they look muddy on dark surfaces.

**Verification**: Visual: `/tmp/critique/overlay-toggles.png` shows Drift toggle in semantic red with translucent bg, readable on dark.

**Heuristic improvement**: Aesthetic +1, Consistency +1.

---

### Score progression

| Run | Score | P0 | P1 | P2 | Notes |
|------|-------|----|----|-----|-------|
| 2026-08-07T13:28 (initial) | 23/40 | 3 | 2 | 4 | Baseline |
| 2026-08-07 (post-P0) | ~25/40 | 0 | 2 | 4 | Visibility + Recognition fixed |
| 2026-08-07 (post-P1) | ~28/40 | 0 | 0 | 4 | Aesthetic + Consistency fixed |

**Cumulative delta**: +5 points (23 → 28). All P0 and P1 cleared. **Suite status**: 185 passed, 19 skipped, 0 failed.

---

### Carry-forward (P2/P3 — not addressed)

| Severity | Issue | Why deferred |
|----------|-------|-----------|
| P2 | 9+ view tabs in Object Inspector have no icons | Larger refactor (icon set + labels); needs design decision |
| P2 | Jargon: "Hotspots", "Drift", "Boundary Violations" without tooltips | Glossary work; separate UX writing pass |
| P2 | `transition: width` in ScanBar.tsx:83 (jank risk) | Performance optimization; needs benchmark first |
| P2 | Disabled state lacks consistent `aria-disabled` | A11y audit; broader than this critique scope |
| P3 | Capitalized eyebrow labels ("MAP", "TRY ASKING", "RECENT EXPLORATIONS", "WORKSPACE QUALITY") on landing panel sections | Anti-pattern but low impact; needs copy redesign |
| P3 | Emoji icons (⌘K, ✦, 🔍, ⚙) used as visual shortcuts | Acceptable per AI-slop test but worth revisiting |

---

### Pattern established for future fixes

1. **CSS tokens are the source of truth**: Always add new color tokens to `apps/explorer-ui/src/tailwind.css` before referencing them. Never use Tailwind color scales (50-200) directly in dark-mode components.
2. **Inline styles for dynamic state**: For tinted backgrounds that change with state (hover, active, disabled), use inline `style={{}}` with `color-mix()`. Don't try to express dynamic color blends in className strings.
3. **Visual regression snapshots**: After any UI change, expect `--update-snapshots` to be needed. 19 baseline snapshots regenerated this session.
4. **Heat order**: P0 first (broken UX), P1 next (visual quality), P2/P3 as carry-forward.
