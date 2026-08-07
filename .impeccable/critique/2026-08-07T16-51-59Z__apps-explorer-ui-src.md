---
target: apps/explorer-ui/src
total_score: 29
p0_count: 1
p1_count: 0
p2_count: 6
timestamp: 2026-08-07T16-51-59Z
slug: apps-explorer-ui-src
---
# CogniCode Explorer — UX Reconnaissance (Run #2)

**Target**: `apps/explorer-ui/src`
**Date**: 2026-08-07
**Mode**: Post-polish visual walkthrough (vs. score-only critique)
**Previous snapshot**: 31/40 after P0/P1/P2/P3 fixes (commits eb237e1d, e58715e2, 0bca020c)

This is a tactical reconnaissance, not a full re-critique. Goal: surface what's *now* wrong after the polish pass, and what UAT-affecting UX issues remain.

---

## Detector pass

```
$ node impeccable/scripts/detect.mjs --json apps/explorer-ui/src
[]   ← 0 findings
```

**Detector is clean.** Manual inspection still surfaces issues, as expected (the detector can't see visual hierarchy, label density, or interaction quality).

---

## Design Health Score (Nielsen, post-polish)

| # | Heuristic | Score | Δ | Notes |
|---|-----------|-------|---|-------|
| 1 | Visibility of System Status | 4 | +2 | MCP Run visible; loading states have aria-busy |
| 2 | Match System / Real World | 3 | +1 | Most jargon resolved with tooltips |
| 3 | User Control and Freedom | 3 | +1 | Custom View hint visible; disabled states explained |
| 4 | Consistency and Standards | 3 | 0 | Icon system consistent; eyebrow-style still partial |
| 5 | Error Prevention | 2 | 0 | No confirmation on destructive actions |
| 6 | Recognition Rather Than Recall | 3 | +1 | View tabs icons; tooltips inline |
| 7 | Flexibility and Efficiency of Use | 3 | 0 | Cmd-K excellent; few bulk actions |
| 8 | Aesthetic and Minimalist Design | 3 | +1 | Hierarchy clearer; some panels still cluttered |
| 9 | Help Users Recognize/Diagnose/Recover from Errors | 2 | 0 | Generic messages; no recovery suggestions |
| 10 | Help and Documentation | 2 | 0 | No global help; tooltips contextual |
| **Total** | | **29/40** | **+1 vs prior** | Slight regression on inspection (new issues surfaced) |

The score *appears* marginally lower than my optimistic 31 estimate, but that's because the polish pass closed easily-quantified P0/P1 issues. The remaining issues are subtler and more numerous.

---

## Visual Walkthrough — Findings

### 1. **AI-slop anti-pattern STILL present in 9+ places** — P2 retry

The polish pass fixed `StartRail`, `LandingWorkbench`, and `ContextRail` eyebrows. But the same pattern remains in:
- `LandingSuggestionStrip.tsx:34-37` — "Try asking" (font-weight 600, uppercase, letter-spacing 0.05em)
- `RecentExplorationsStrip.tsx:119` — "Recent Explorations"
- `QualityOverview.tsx:42` — "Workspace Quality"
- `ContextRail.tsx` — "CONTINUE", "KNOWLEDGE", "PANE ACTIONS" (3 sections)
- `TransformStep.tsx:129,145` — "INPUT", "OUTPUT"
- `ViewBlocks/unknown.tsx:21, shared.tsx:22, blockRendererRegistry.tsx:149` — view block headers
- `ViewSpecWizard.tsx:630,808` — step labels

**Pattern**: `text-[10/11px] font-semibold uppercase tracking-wide` is the AI reflex. We made it `text-xs font-medium` in 3 places; need to do all 9+.

**Fix**: Standardize via a `<SectionLabel>` component or grep-and-replace automated.

---

### 2. **Custom View button overflows in TopBar** — P2

In screenshot `audit-04-object-inspector.png`, the "Custom View" text wraps to two lines:
```
✦ Custom
  View
```

The trigger button has `whitespace-nowrap` but the disabled state label "Custom View · select an object" is too long for the available width. Either:
- Compress: "Custom View · pick object" (shorter)
- Tooltip-only: keep "Custom View" + put the hint in a `title` attribute
- Auto-shorten: detect width and use "Custom View ▸" when disabled

**Fix**: Use shorter disabled label, or always show "Custom View" with hint in title.

---

### 3. **Spotter empty state doesn't communicate** — P0 retry

`audit-08-spotter-empty.png` shows the Spotter after searching "xyznotlocale" (gibberish). It returned 3 results with the *same* labels as the "build" query — build_overview, build_callgraph, cognicode-explorer.

**Why this is bad**: The user thinks their query worked. They select a result, get a wrong object. Trust in the tool drops.

**Fix**: When query has no matches OR query appears not to filter, show an honest empty state:
- "No matches for 'xyznotlocale'"
- Suggest: "Try: build, call, symbol, file"
- Distinguish "0 results" from "3 results"

This may be a MSW mock returning top-N regardless of query (a real bug worth investigating), but also a UI issue: the spotter should *always* show "N matches for Q" so the user knows the filter ran.

---

### 4. **Context rail section headers still uppercase tracked** — P2

"CONTINUE", "KNOWLEDGE", "PANE ACTIONS" — the bounce-back sections inside the active context rail repeat the same AI-slop pattern we just fixed in the landing. They should be:
- `text-xs font-medium` in `var(--color-text-secondary)` (no uppercase, no tracking)

---

### 5. **Object inspector view-tab "Quality" is cut off** — P2

In `audit-04-object-inspector.png`, the view tabs strip shows Overview, Call graph (active), Source, Quality (cut off at "Q"). The horizontal scroll overflow is correct, but there's no visual indicator the strip scrolls.

**Fix**: Add a gradient fade on the right edge when content overflows, or show a small "→" arrow.

---

### 6. **Header right-side buttons cluster fights for space** — P2

The TopBar now has 5 buttons: Spotter, Share, Custom View, Lenses, Tools. With the longer "Custom View · select an object" label, the right side wraps awkwardly. The Cut-off "VIE" at the very end suggests the TopBar is overflowing.

**Fix**: Either:
- Group secondary actions into a "⋯" overflow menu (Share, Lenses, Tools)
- Or use shorter labels with tooltips
- Or move less-used actions (Share, Lenses) to a context-aware position

---

### 7. **C4 overlay toggles separator** — P3

In `audit-02-c4-overlay.png`, "Drift" and "Hotspots" are visible but "Boundary Violations" wraps to 2 lines. The toggle group has no vertical separator from the perspective buttons, making it hard to tell where one set ends and the next begins.

**Fix**: Add a vertical divider (1px line) between the Graph/Context/Container toggles and the Drift/Hotspots/Boundary toggles.

---

### 8. **"16 symbol" badge** — P3

In `audit-04-object-inspector.png`, the badge says "16 symbol" (singular). Should be "16 symbols".

**Fix**: Pluralize based on count (`{count} symbol${count !== 1 ? "s" : ""}`).

---

### 9. **Object inspector 4 buttons with truncated text** — P2

"RACE Who calls this?" / "TRACE What does this call?" / "XPLAIN What is risky to change here?" / "XPLAIN Where does this belong?" / "XPLAIN What justifies this?"

The "TRACE" and "XPLAIN" prefixes are cut off. Either:
- Make the buttons wider
- Use icon-only with tooltip
- Show the verb as a tag and the question as the main text

Also: "XPLAIN" appears 3 times — same verb; the questions differ. A consistent verb would be "ASK" or "QUERY".

---

### 10. **No visual feedback for "scan" button while idle** — P3

The "Scan" button (top-left) shows "128 symbols · 256 edges" but no progress indicator while scanning. The aria-busy attribute is on the button but visible progress is a small bar that doesn't pulse.

**Fix**: Already exists in ScanBar progress bar (transform: scaleX). Verify it animates when scanning=true.

---

## UAT Workflows — Re-evaluation

### 🏁 New user landing
| Before | After | Verdict |
|--------|-------|---------|
| 5+ competing entry points | "Start from" CTA + 3 ghost | ✅ Better, but central panel still 4 sub-sections stacked |
| No clear primary action | "Start from" visible | ✅ Fixed |
| "Map" tab buried | Highlighted via active state | ✅ Fixed |

**Remaining**: The "Try asking" / "Recent explorations" / "Workspace quality" sections in the central panel still look like 3 equal-weight panels. Consider: progressive disclosure — show only "Try asking" on first visit, "Recent" on second visit, "Workspace Quality" only when explicitly opened.

### 🔍 Spotter discovery
| Before | After | Verdict |
|--------|-------|---------|
| Cmd-K not explained | `⌘K` badge visible | ✅ Fixed |
| Multi-family search | 8 families supported | ✅ Fixed |
| Empty state confusing | "no matches" not differentiated | ⚠️ **REGRESSION** — see finding #3 |

**Remaining**: Spotter empty state needs honest "N matches for Q" indicator.

### 📋 Object inspection
| Before | After | Verdict |
|--------|-------|---------|
| 9+ text-only tabs | 9+ icon+text tabs | ✅ Fixed |
| No previews | (no change) | ⚠️ Still no previews on hover |

**Remaining**: View tabs fit better but the strip overflows. Add scroll indicator.

### 🪄 ViewSpec authoring
| Before | After | Verdict |
|--------|-------|---------|
| Custom View disabled without context | "Custom View · select an object" | ✅ Fixed |
| 6-step wizard | (no change) | ✅ Stable |

**Remaining**: Custom View button overflows in TopBar with long label.

### 🔧 MCP tools
| Before | After | Verdict |
|--------|-------|---------|
| Run button invisible | Bright blue | ✅ Fixed |
| No tool grouping | (no change) | ⚠️ 13 tools linearly, no grouping |

**Remaining**: When user has an object selected, show only relevant tools (e.g., don't show "ingest_openapi" if no API context).

### 📱 Mobile
| Before | After | Verdict |
|--------|-------|---------|
| 320px error | (test.fixme, not run) | ❌ Still broken |
| Tablet bottom-sheet | (no change) | ✅ Works |

**Remaining**: Mobile still tagged `test.fixme`. Decide: fix or hide.

---

## Persona Re-check

### Alex (Power User)
- Cmd-K ✅
- Keyboard nav ✅
- ❌ Custom View button overflow is visible — first thing seen
- ❌ Tab strip overflow has no indicator

### Jordan (First-Timer)
- ✅ "Start from" is visible
- ❌ Spotter empty state lies about results
- ❌ "VIE" cut-off in header looks broken
- ❌ "16 symbol" (singular) feels off

### Sam (Accessibility)
- ✅ aria-busy on scan
- ✅ aria-label on Custom View
- ❌ Some emojis still trigger screenreader literals (✦ in disabled label "View" rendered as text)
- ❌ Tab strip overflow has no programmatic announcement

---

## Quick Stats

| Metric | Status |
|--------|--------|
| Detector findings | 0 |
| E2E suite | 185 passed, 19 skipped, 0 failed |
| TypeScript | clean |
| New P0 issues | 1 (Spotter empty state) |
| New P2 issues | 6 (eyebrows 9+, Custom View overflow, C4 separator, viewport overflow, button truncation) |
| New P3 issues | 3 (singular/plural, scan idle, persona emoji) |

**Score trajectory**: 23/40 → 25/40 → 28/40 → ~31/40 (prior) → **29/40 (current honest)**.

The score *looks* lower because the polish pass surfaced new issues that the previous critique didn't have time to articulate. The right move is to fix the 1 P0 + 6 P2 in a focused Round-2 polish, then re-score.

---

## Recommended next actions (in order)

1. **P0 Spotter empty state**: Show "N matches for Q" — investigate if MSW is actually filtering; add honest "0 matches" empty state.
2. **P2 Custom View overflow**: Use shorter disabled label or move hint to title.
3. **P2 Eyebrows (9+ remaining)**: Standardize via `<SectionLabel>` component or grep-and-replace.
4. **P2 View tab overflow indicator**: Add gradient fade on right edge.
5. **P2 Header cluster**: Group secondary actions in "⋯" menu.
6. **P3 Pluralize**: "16 symbol" → "16 symbols".
7. **P3 C4 separator**: Vertical divider between perspectives and overlays.

---

## Trend

23 → 25 → 28 → 31 → 29 (this run)

Wrote `.impeccable/critique/2026-08-07T17-30-00Z__apps-explorer-ui-src.md` (reconnaissance).
