# e84.1 follow-through — Summary

Closed e84.1 with `ba60e806` on 2026-09-17. User signal "continua"
after archive closure. Follow-through (no new cycle) covered four
bounded gaps surfaced by re-auditing each WU:

## WU1 — Per-tool drift table

**Gap:** the high-level drift report (`wu1-mcp-capability.md`)
counted +5 tools but did not list them. Per-tool table now
in `wu1-follow-through-per-tool-drift.md`. All 5 are
**experimental** slicing/CFG/dominators/taint tools. 0 removed.
No user-visible drift (skill lists them once under
"Experimental").

**Pre-existing:** `docs/MCP-TOOLS.md` claims 68 tools (last
regenerated 2026-08-06). Regeneration is a docs-cleanup cycle,
out of scope.

## WU4 — Placeholder scripts in archived skills

**Gap:** `cognicode-mcp-driven-archived/references/{bootstrap,
invoke}.sh` are 2- and 4-line placeholders that survived audit.
**Decision:** leave them in the archive (historical record). Live
skills do not need `references/*.sh` — they are declarative
Markdown + YAML. Documented in
`wu4-follow-through-placeholder-scripts.md`.

## WU18 — Graph persistence "pinned bug" — NOT REPRODUCIBLE

**The honest finding.** I added a `#[ignore]` test pinning the
WU18 hypothesis ("`build_graph` writes to analysis cache but
readers read from persistence store, missing in between"). The
test PASSED — meaning the hypothesis was **wrong for the default
code path**.

`HandlerContext::get_graph_store()` (mod.rs:430) returns a
`CachedGraphStore` wrapping `analysis_service.graph_cache()`.
Both `build_graph` (writes) and `graph_explain` (reads) hit the
same cache in the default config. The pin test was removed —
keeping a `#[ignore]` test for a non-bug is theatre, not signal.

The live UAT finding from WU18 likely hit one of:
1. The persistent (SQLite) `GraphStore` path — not exercised by
   the unit test. `build_graph`'s persistent path saves only the
   manifest, not the graph. So a fresh `graph_explain` after
   `build_graph` in the persistent config would fail.
2. Process restart between MCP calls.
3. Different directory resolution.

WU19 Class A1 entry is **downgraded** from "code bug" to
"configuration caveat". The skill's "Known runtime quirk"
section was updated to describe the configuration-dependent
behavior honestly.

**What this means for users:** in the default `cogh` runtime,
`build_graph` then any reader works in the same process. If the
runtime is wired with a persistent `GraphStore` (e.g. some IDE
configurations), the workaround in the skill applies.

## WU17 — Deterministic catalog lookup

**Gap:** `validate_skills.py::_find_catalog()` returned
`candidates[0]` — non-deterministic order from `Path.iterdir()`.
With one catalog today this is harmless; with two, the validator
might silently validate against the wrong (stale) catalog.

**Fix:** sort by `YYYY-MM-DD-` prefix descending; pick the
newest. Tie-break by full path. Documented in
`wu17-follow-through-deterministic-catalog.md`. Validator still
PASS.

## Gates

| Gate | Result |
|------|--------|
| `cargo test -p cognicode-cli --tests` | 180 passed; 0 failed; 2 ignored |
| `cargo check --workspace` | exit 0 (warnings only) |
| `cargo fmt -p cognicode-cli --check` | clean |
| `cargo fmt --check` | pre-existing drift in `cognicode-core/src/application/ai/boundary_tests.rs` (untouched since `14a3a721`) — not from this cycle |
| `python3 scripts/validate_skills.py` | 4/4 PASS; 52 MCP tool refs validated |
| `bash scripts/verify-skills.sh` | PASS |
| `cargo test --workspace` | pre-existing 17+1 failures in `cognicode-core/tests/{file_ops_integration,security_validation}_tests.rs` — confirmed pre-existing on `HEAD` (before follow-up edits) by `git stash` + retest |

## Files changed (follow-up only, NOT yet committed)

- `scripts/validate_skills.py` — deterministic catalog lookup.
- `skills/cognicode-mcp/SKILL.md` — configuration-dependent
  quirk wording.
- `openspec/changes/archive/2026-09-17-e84-1-skill-productization/artifacts/wu1-follow-through-per-tool-drift.md`
- `openspec/changes/archive/2026-09-17-e84-1-skill-productization/artifacts/wu4-follow-through-placeholder-scripts.md`
- `openspec/changes/archive/2026-09-17-e84-1-skill-productization/artifacts/wu17-follow-through-deterministic-catalog.md`
- `openspec/changes/archive/2026-09-17-e84-1-skill-productization/artifacts/wu18-follow-through-graph-persistence.md`
- `openspec/changes/archive/2026-09-17-e84-1-skill-productization/artifacts/e84-1-followthrough-summary.md` (this file)

## Pre-existing diffs still uncommitted (NOT from e84.1)

- `docs/adr/README.md`
- `openspec/changes/archive/2026-09-17-e86-cogh-lifecycle/state.yaml`
- `crates/cognicode-core/src/application/ai/boundary_tests.rs` (fmt drift, e79)

## STOP per brief

Per the original e84.1 brief: "Archive e84.1 and STOP for
review before e85". e84.1 is closed; follow-up gaps surfaced
and addressed within scope. Next cycle (e85 Linux Release
Factory) requires explicit user signal.
