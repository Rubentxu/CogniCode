# CogniCode Test Plan

> **Status**: DRAFT (E31-B sub-cycle of v1.0.0 readiness; pending maintainer sign-off for T1)
> **Date**: 2026-08-10
> **Owner**: Rubentxu (maintainer)
> **Scorecard gate**: enables G13 (Test Plan comprehensivo) per `docs/RELEASE-1.0.0-PLAN.md` §1.1
> **Predecessor docs**: `docs/RELEASE-1.0.0-PLAN.md`, `sandbox/results/baseline/`, `.claude/skills/test-pyramid/`

## 1. Purpose

This document is the **canonical strategy** for CogniCode testing. It is the input that makes the G13 scorecard gate testable. Per E31-A (PR #237) §1.1, v1.0.0 cannot be tagged unless `docs/TEST-PLAN.md` exists (T1) AND T1–T6 GREEN in a scorecard run (T7). This document defines what each sub-criterion means and codifies the operational ownership.

Scope:

- **In**: 5 testing levels, ownership, coverage matrices, regression policy (T6), stability threshold (T7).
- **Out**: per-tool behavior spec (lives in `openspec/specs/{tool-name}/`), CI recipe hardening (just recipes, GH Actions).

## 2. Test Pyramid — 5 Levels

```
                 ▲
                ╱ ╲
               ╱   ╲      L4: browser-E2E (Playwright)        — slow, expensive
              ╱─────╲
             ╱       ╲     L3: sandbox-E2E (real repos)      — the corpus
            ╱─────────╲
           ╱           ╲    L2: integration (vitest + cargo)
          ╱─────────────╲
         ╱               ╲   L1: unit (cargo + vitest unit)
        ╱                 ╲
       ─────────────────────
```

Inverse relation: more L1 tests, fewer L4 tests; cheaper failure detection the lower you go.

### L1 — Unit

- **Tooling**: `cargo test` + `vitest run`
- **Surface**: 7 crates with `tests/` dirs (cognicode-core, cognicode-macros, cognicode, cognicode-explorer, cognicode-runtime, cognicode-graph-wasm, spike-ladybug) + per-component vitest files (43 in apps/explorer-ui/src)
- **Runtime**: <5 min wallclock target per crate; full workspace <30 min
- **Receipt**: `just test-unit`
- **Owner**: feature author (every `fix(*)` adds at least one test, see §5)

### L2 — Integration

- **Tooling**: vitest + cargo feature matrix + hyperscript mocks for cross-crate glue
- **Surface**: ports/adapters interaction (Calls, Ports, Stores, Repositories), keep-alive contracts, watcher events, session lifecycle. ~12 test files centered in cognicode-core/tests/ + apps/explorer-ui/src/test/
- **Runtime**: <10 min wallclock
- **Receipt**: subset of `cargo test --workspace` + `npm test` filtered by tag `@integration`
- **Owner**: feature author + reviewer (cross-crate contracts)

### L3 — Sandbox E2E (the corpus)

- **Tooling**: sddk-validate + podman quadlets per language + `clone_repos.sh` pinned SHAs + ground-truth matchers (scoring engine)
- **Surface**: 788 named scenarios across 49 manifests (rust, ts, python, go, java, javascript, php, ruby, swift, terraform, ansible); corpus covers Tier-1 (rust ts python go java) + Tier-2 (javascript php ruby swift) + Tier-3 (terraform ansible scale experiments)
- **Runtime**: smoke lane <5 min (Tier-A); full matrix 30–60 min nightly
- **Receipt**: `just sandbox-ci-smoke` (PR), `just sandbox-ci-full` (nightly), `just release-scorecard`
- **Owner**: maintainer (corpus expansion), feature author (per-tool scenario extension)
- **Out of scope**: Tier-3 typescript SCAL-001 timeout — documented best-effort, see ADR candidate for E31-F

### L4 — Browser E2E

- **Tooling**: Playwright (config in `apps/explorer-ui/playwright.config.ts`) + MSW for API mocking
- **Surface**: 41 `.spec.ts` files in `apps/explorer-ui/e2e/` covering PaneInspector, PaneStack, ObjectInspector variants, LensPanel, ContextualPanel, InteractiveGraph, LandingWorkbench, view tabs, evidence flows
- **Runtime**: 5–15 min wallclock full suite; per-spec <30s typical
- **Receipt**: `just explorer-e2e` or `just test-e2e`; `just explorer-e2e-stability 3` for flakiness budget
- **Owner**: frontend feature author + UX reviewer (browsing flows + screenshot reviews)

### L5 — Performance Regression

- **Tooling**: Criterion (cargo bencher) for Rust crates; custom JS bench for the explorer (`apps/explorer-ui/src/bench/`)
- **Surface**: graph algorithms (crates/cognicode-core/benches/graph_benchmarks.rs), MCP handler throughput, frontend render times for large graphs
- **Runtime**: <10 min per bench run; weekly cadence
- **Receipt**: `just perf-bench`
- **Owner**: maintainer (gate on regressions)

## 3. Coverage Matrices

The G13 sub-criteria T3–T5 require machine-readable matrices. This section inlines the **initial state** as of 2026-08-10. Each sub-cycle B2–B4 will refresh them as data improves.

### T3 — MCP tools × Tier-1 languages

The runtime tool catalog has **68 MCP tools** (regenerated from `bash sandbox/scripts/list_mcp_tools.sh`). Tier-1 languages are `rust`, `ts`, `python`, `go`, `java`.

**Tools exercised in sandbox scenarios today** (from `grep "^\s\+tool:" sandbox/manifests/*.yaml`, sorted by frequency):

| Tool | Sandbox scenarios | Tier-1 coverage today |
|------|-------------------|------------------------|
| `edit_file` | 53 | rust✓ ts✗ py✓ go✓ java✓ (rust via `rust_repos.yaml`+`baseline_test.yaml`; py via `python_repos.yaml`; go via `go_repos.yaml`; java via `java_repos.yaml`+`spring-petclinic`; **ts gap**) |
| `read_file` | 49 | rust✓ ts✗ py✓ go✓ java✗ (**ts gap, java gap**) |
| `build_graph` | 68 (45 + 23) | rust✓ ts✗ py✓ go✓ java✓ (**ts gap**) |
| `get_file_symbols` | 72 (37 + 35) | rust✓ ts✗ py✓ go✓ java✓ (**ts gap**) |
| `search_content` | 32 | rust✓ ts✗ py✓ go✓ java✓ (**ts gap**) |
| `get_complexity` | 26 | rust✓ ts✗ py✓ go✓ java✓ (**ts gap**) |
| `safe_refactor` | 18 | rust✓ via `petclinic` java✓ (**ts gap, python gap**) |
| `iac_query` | 18 | terraform× ansible× (not Tier-1; ok) |
| `semantic_search` | 16 | rust✓ ts✗ py✗ go✗ java✗ (**4 gaps**) |
| `query_symbol_index` | 13 | rust✓ (**4 gaps**) |
| `find_usages` | 10 | python✓ rust✓ (**3 gaps**) |
| `get_entry_points` | 12 | rust✓ ts✓ (real repos.ts) python✓ java✓ go✓ |
| `build_lightweight_index` | 11 | rust✓ python✓ (**3 gaps**) |
| `get_call_hierarchy` | 11 | rust✓ (**4 gaps**) |
| `get_leaf_functions` | 11 | rust✓ (**4 gaps**) |
| `analyze_impact` | 9 | rust✓ ts✗ py✓ go✗ java✗ (**3 gaps**) |
| `debug_analyze` | 9 | rust✓ python✓ (**3 gaps**) |
| `get_outline` | 8 | python✓ rust✓ (**3 gaps**) |
| `list_files` | 8 | rust✓ ts✓ python✓ go✓ java✓ |
| `http_get` | 13 | not an MCP tool (helper); ignored |
| (38 remaining tools) | 0 each | **none in sandbox** |

**T3 current verdict**: 5 tools (8%) full Tier-1 coverage; ~25 tools (37%) partial; ~38 tools (56%) absent from sandbox. **Action**: B2/B3 sub-cycles add scenarios for the partial and absent tools.

### T4 — UI panes × browser-E2E specs

41 Playwright `.spec.ts` files in `apps/explorer-ui/e2e/` cover the following panes (initial mapping):

| Pane (or feature area) | Spec files |
|------------------------|-----------|
| `LensPanel` | `lens-panel.spec.ts`, `lens-panel-toggle.spec.ts` |
| `ObjectInspector` / `PaneInspector` | `object-inspector.spec.ts`, `pane-stack-drilldown.spec.ts` |
| `PaneStack` | `pane-stack.spec.ts`, `pane-stack-drilldown.spec.ts` |
| `ContextualPanel` | `contextual-panel.spec.ts`, `pane-navigation.spec.ts` |
| `InteractiveGraph` | `interactive-graph*.spec.ts`, `phase1-executors-full.spec.ts` |
| `LandingWorkbench` | `landing-real-data.spec.ts`, `landing-workbench.spec.ts` |
| `ViewTabs` | `view-tabs-coverage.spec.ts` |
| `LensSidebarToggle` | `lens-sidebar-toggle.spec.ts` |
| `EvidencePin` | `pin-evidence.spec.ts` |
| `SpotterSearch` | `spotter-multifamily.spec.ts` |
| `OnboardingWizard` | `onboarding-wizard.spec.ts` |
| `ViewSpecWizard` | `viewspec-wizard-full.spec.ts`, `viewspec-authoring.spec.ts` |
| (28 remaining specs) | cross-cutting + flow tests (error states, share, smart MCP, etc.) |

**T4 current verdict**: 12 of 12 known panes have ≥1 spec; new panes ship with a spec in the same PR (T6 enforcement).

### T5 — Sandbox flaky scenario log

**Initial state (2026-08-10)**: the flaky-tracking subsystem exists in `sandbox/scripts/analyze_stability.py` (tracks per-tool CV) and in `sandbox/scripts/release_scorecard.py` G6 (max CV < 0.10). **No dedicated per-scenario flaky log file exists yet.**

**Action** (E31-B4 sub-cycle):

- Produce `sandbox/results/flaky_scenarios.md` as a per-scenario table (id, tool, language, pass_rate over last 30 days, quarantine status). Live updated by nightly scorecard runner.
- G6 acceptance criterion stays the same (max tool CV < 0.10); T5 surfaces per-scenario visibility for the maintainer.
- Known-flaky scenarios are quarantined (removed from G6 max-CV computation). **Unknown-flaky** (newly flaky not in this log) **fails** G13 — that's the explicit "no surprise flaky" guarantee.

## 4. Sub-criterion ownership matrix

| Sub-criterion | Description | Owner | Measurable artifact |
|---------------|-------------|-------|---------------------|
| **T1** | `docs/TEST-PLAN.md` exists, signed | Maintainer | This file (DRAFT → ACEPTADO when maintainer signs) |
| **T2** | 5 levels defined with strategy | Maintainer | §2 of this file |
| **T3** | Every MCP tool with ≥1 scenario per Tier-1 language | Feature author (per tool) + maintainer (matrix) | `sandbox/reports/mcp_tool_tier1_coverage.yaml` |
| **T4** | Every UI pane with ≥1 browser-E2E spec | Frontend author + UX reviewer | `apps/explorer-ui/e2e/COVERAGE.md` |
| **T5** | Sandbox-E2E nightly + flaky log maintained | Maintainer | `sandbox/reports/flaky_scenarios.md`; nightly artifact archive |
| **T6** | Regression test in every `fix(*)` PR since v0.92.0 | PR author (policy enforced at PR review) | All fix commits from v0.92.0 onward have a test added/changed in their diff |
| **T7** | Scorecard stable ≥N=5 consecutive nights | Maintainer (cadence) | `sandbox/results/stability.json` CV <0.10 sustained; scorecard.json archived per run |

## 5. Regression Policy (T6)

**Rule**: every pull request that contains `fix(*)` commits MUST also add, modify, or re-enable at least one test in the same PR. The test can be unit (L1), integration (L2), sandbox scenario (L3), or browser-E2E (L4) — the level chosen depends on the bug's surface, but **a test must exist**.

**Enforcement** (3 layers):

1. **CI lint gate** — pre-commit hook + GH Action that scans PR diff for `fix(*)` commits and confirms at least one file matching `**/*test*.{rs,ts,tsx}` or `**/scenarios/*.yaml` changed in the same PR. No test = CI failure with explicit message.
2. **PR template** — checkbox at PR creation: *"This PR contains at least one test for the change"*, default unchecked, must be ticked before review.
3. **Reviewer expectation** — code review blocks merge if the fix lacks a test, regardless of CI result.

**Edge cases**:

- `chore(*)` or `docs(*)` commits are exempt (no behavior change to test).
- `refactor(*)` commits SHOULD keep tests green but don't require new tests (refactor preserves behavior).
- Dependency-only fixes (`fix(deps)`) are exempt unless behavior changed.

**Initial application**: this policy enters force from this document's ACEPTADO moment. Existing fix commits since v0.92.0 are not retroactively required to add tests.

## 6. Stability Threshold (T7)

**Cadence**: 1 full `sandbox-ci-full` run per day minimum; weekly longer campaigns.

**Stability measurement**: per-tool coefficient of variation (CV) of `timing_p95_ms` across N consecutive runs. Lower CV = more stable.

**Acceptance**: max CV across all tools <0.10 for 5 of 7 consecutive cadence-days (allows 2 days of grace).

**Quarantine**: a single tool with CV >0.10 in 3+ of 7 consecutive days is quarantined, becomes a `sandbox/results/flaky/` candidate, and is removed from G6's max-CV computation until handled.

**Tool audit**: the 5 worst-CV tools from each run are reviewed at the next scorecard cycle; documented in `sandbox/results/audit_<ts>.md`.

## 7. Maintenance & Update

- **Cadence**: this document updates when:
  1. New test level added (rare; would bump document version).
  2. New tool tier added (Tier-0, etc.).
  3. T6 policy exception ratified.
  4. Coverage thresholds revised.
- **Approval**: maintainer (single); co-approval required only for T6 policy changes.
- **Versioning**: track in CHANGELOG.md under `[Unreleased]` block with bullet `test-plan: <summary>`.

## 8. Roadmap to v1.0.0

This document (E31-B) closes T1 and T2 in a single cycle. Subsequent cycles close T3–T7 sub-criteria mechanically:

| Sub-cycle | Closes | Compute |
|-----------|--------|---------|
| **E31-B** (this cycle) | T1, T2 | authoring this doc + initial coverage matrices |
| E31-B2 | T3 (partial — Tier-1) | new scenarios for tools without Tier-1 coverage |
| E31-B3 | T4 | missing pane specs + screenshot coverage audit |
| E31-B4 | T5 | nightly cron live + first flaky log populated |
| E31-B5 | T6 CI gate | implement the lint + GH Action enforcing T6 rule |
| E31-B6 | T7 scoreboard run | 5+ consecutive nights achieving max CV <0.10 |

Once all T1–T7 are GREEN in a scorecard run, G13 turns GREEN and v1.0.0 is achievable (modulo E31-C 14 ADRs closure).

## 9. References

- `docs/RELEASE-1.0.0-PLAN.md` — the umbrella v1.0.0 plan (Pillar 7 / G13 derived from §1.1)
- `~/.sddk-knowledge/CogniCode/cycles/CYC-2026-08-10-e31-a-release-plan-pillar-7.md` — E31-A cycle
- `~/.sddk-knowledge/CogniCode/audits/2026-08-10-e31-pre-flight.md` — pre-flight audit identifying TEST-PLAN.md absence as blocker for G13
- `sandbox/scripts/release_scorecard.py` — gate G13 wiring (added in E31-A)
- `openspec/specs/openspec-conformance/spec.md` — openspec harness; orthogonal to this document
- `.claude/skills/test-pyramid/SKILL.md` — generic testing-pyramid skill for ad-hoc test design
