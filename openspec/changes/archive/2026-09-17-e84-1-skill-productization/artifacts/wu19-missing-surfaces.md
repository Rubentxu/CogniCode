# WU19 — Missing product surfaces and capability debt

The capability audit (WU1, WU2) and UAT (WU18) surfaced several
classes of missing product surfaces. None of these are fixed in
this cycle; they are recorded for future cycles.

## Class A — Public-but-broken runtime behaviour

### A1. MCP graph persistence across tool calls

**Discovered in:** WU18 (UAT).
**Location:** `crates/cognicode-core/src/interface/mcp/handlers/consolidated_handlers.rs:217,367,534,680,816,840`.
**Symptom:** After a successful `build_graph` call, subsequent
graph-reading tools (`graph_explain`, `graph_communities`,
`graph_pagerank`, etc.) return `"No call graph available. Run
build_graph first."`. Only `check_architecture` and a handful of
others auto-build and succeed.

**Root cause (preliminary):** `build_graph` writes to the
`WorkspaceSession.analysis` cache (`workspace_session.rs:944-958`)
but the consolidated handlers read from `ctx.get_graph_store()`
which is the **persistence store**, not the in-memory cache. Two
different stores.

**Workaround in skill:** documented in
`skills/cognicode-mcp/SKILL.md` "Known runtime quirk" section —
call `check_architecture` first (it auto-builds), or use Explorer
UI for graph-heavy workflows.

**Fix:** out of scope for e84.1. Records as a high-priority
candidate for a future cycle (e86 followup or e87). Estimated
fix: align the consolidated handlers to read from the
`WorkspaceSession.analysis` cache, OR teach `build_graph` to also
write to the persistence store.

### A2. Pre-existing CLI lifecycle bugs (pinned in e86)

**Discovered in:** e86 / e86-followup.
**Bugs:**

- `cogh rollback` fails on a populated `CreatedDir` because
  `rmdir` errors on a non-empty directory and the journal does
  not record individual files extracted.
- `cogh ide install` fails on a second install: stale shim
  EEXIST because `LinuxAdapter::install_shim` does not unlink the
  existing shim before symlinking.
- `cogh update` with a zero-component profile silently pins the
  tracker to a version that installed nothing.

**Status:** all three pinned in the e86 followup archive
(`openspec/changes/archive/2026-09-17-e86-followup-live-update/`).
**Fix:** out of scope for e84.1. Recorded as candidates for a
future cycle.

## Class B — Internal capabilities not yet exposed

These exist in `cognicode-core` but have **no CLI, MCP, or
Explorer surface today**. They are not advertised in any skill.
They are documented in ADRs and the LSI roadmap.

| Capability | ADR | Notes |
|------------|-----|-------|
| Canonical Facts/Evidence inspection | ADR-040 (Fact/Evidence/Hypothesis) | Domain-level only; no interface. |
| Intelligence Event Log queries | ADR-043 | Domain-level only; no interface. |
| SoftwareWorld / Trial / Fork | ADR-046 (Software World Fork, Trial, Diff and Promote) | Domain-level only. |
| Historical replay / held-out promotion | ADR-051 | Domain-level only. |
| Shadow comparison | (no ADR; mentioned in LSI roadmap) | Domain-level only. |
| AI SemanticMiner / FindingCritic / FixAgent | (mentioned in LSI roadmap) | AIX layer; only `ask_about_code` / `find_pattern_by_intent` exposed as experimental MCP tools, and they do NOT expose the full pipeline. |
| Governed promotion authority | ADR-047, ADR-050 (e80/e83) | No surface. Promotion decisions today are human-driven via the Explorer UI; no MCP/CLI gate. |
| e77 executable architecture (rules + governance) | ADR-049 | No MCP/CLI surface. `check_architecture` is the legacy Tarjan SCC cycle detector and is NOT a replacement. |

**These are not advertised.** The new user skills explicitly
**do not** teach these as if they were public. They are mentioned
in the `cognicode-developer` skill only (and even there, lightly).

## Class C — Internal capabilities exposed but mis-described

### C1. `docs-ingest` and `issues-ingest` in `cognicode --help`

**Location:** `crates/cognicode-cli/src/cmd/...` — listed in
`cognicode --help` but absent from default builds (require
`multimodal` Cargo feature).

**Symptom:** On a default build, `cognicode docs-ingest` returns
"Unknown command", but `cognicode --help` lists it with a long
description that promises behaviour. This is misleading.

**Recommended fix (out of scope for e84.1):** hide them from
`--help` in default builds, OR document them as opt-in features.
Recorded as future evolutive.

### C2. `cogh rollback` parameter reservation

**Location:** `cogh rollback --help` documents `[PLUGIN]` as
"reserved for future use; today the active install is the only
target". This is honest, not mis-described — but worth noting that
the help string says so explicitly. Keep.

### C3. `cogh plugin add` partial

**Location:** `cogh plugin add --help` says "from git-url or
registered marketplace". The marketplace registration is not
implemented.

**Recommended fix:** either implement marketplace registration, or
remove the marketplace reference from the help. Recorded as
future evolutive.

## Class D — Public-but-thin (capability exists but coverage is shallow)

### D1. Project listing / registry

There is no MCP tool today for "list all projects" or
"register a project". The Explorer has views for this but they
are not reachable through the MCP. Recorded as future evolutive.

### D2. View spec catalog

`list_view_specs` and `read_view_spec` exist and work, but the
catalog is mostly built-in (overview, call-graph, etc.). User-
defined runtime view specs are supported but underused. The
skill teaches the tools as-is.

### D3. IaC (`iac_query`)

The tool exists and is labelled experimental. It is taught in
the skill but with no claim of completeness.

### D4. Slicing / CFG / dominators / taint flow

Tools `slice_forward`, `slice_backward`, `cfg_per_function`,
`dominators_cfg`, `taint_flow` exist but are experimental. The
skill mentions them once and does not teach workflows around them.

## Class E — Discovery / publishing gaps

### E1. No public skill index URL yet

skills.sh discovery is mentioned in the brief but the URL is not
formalized in this cycle. Recorded as future evolutive.

### E2. No `cogh skill install`

Explicitly out of scope per the brief. Recorded as e86 work.

### E3. No offline-skill-cache UX

`SkillSource` model (WU14) supports offline caching, but no
runtime implementation. Recorded as e86 work.

## Summary of recorded debt

| Class | Count | Cycle |
|-------|-------|-------|
| A — runtime bugs | 1 (graph persistence) + 3 (CLI pinned) | future |
| B — internal-only | 8 capabilities | future evolutives |
| C — mis-described | 3 cases | future evolutives |
| D — thin coverage | 4 areas | future evolutives |
| E — discovery/publishing | 3 gaps | e86+ |

This debt is documented so that **future cycles can pick it up
without rediscovering it**.
