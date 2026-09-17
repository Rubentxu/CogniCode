# WU1 follow-through — Per-tool drift (runtime vs `docs/MCP-TOOLS.md`)

This is the **per-tool** counterpart to the high-level drift
report in `wu1-mcp-capability.md`. The brief asked for:

```text
- tool name
- description
- input schema
- required parameters
- aliases
- preconditions
- read/write effect
- graph/index prerequisites
```

against `docs/MCP-TOOLS.md`. Most of those fields are NOT in
`docs/MCP-TOOLS.md` (the doc only has name + truncated
description). What IS in the doc is the tool-name list. So the
useful drift at the per-tool level is:

**Net new tools (runtime has, docs missing):** 5

| Tool | Stability | Category | Description |
|------|-----------|----------|-------------|
| `cfg_per_function` | experimental | graph | "Build the CFG (control flow graph) for one function given adjacency, root, and exits." |
| `dominators_cfg` | experimental | graph | "Compute per-function dominators over a CFG adjacency slice." |
| `slice_backward` | experimental | graph | "Backward slice from a (variable, use_sites[]) criterion." |
| `slice_forward` | experimental | graph | "Forward slice from a (variable, definition_site) criterion." |
| `taint_flow` | experimental | graph | "Forward taint analysis over a DFG (declarative Rust patterns)." |

All 5 are **experimental** and live in the `graph` category.
None are anchor tools in the user-facing workflows; they are
listed under "Experimental tools" in `skills/cognicode-mcp/SKILL.md`
with a clear "use when no stable tool covers your need"
caveat.

**Removed/renamed tools (docs has, runtime missing):** 0

**Unchanged tools (in both):** 68

## What this means

- The "+5 since 2026-08-06" headline is **entirely experimental
  tools** in the slicing/CFG space. The "stable" surface is
  unchanged.
- `docs/MCP-TOOLS.md` needs to be regenerated. It is currently a
  load-bearing reference for sandbox coverage gates (per its
  preamble: "Coverage gate (G2): `just release-scorecard`"). The
  drift has not yet broken the gate because the 5 new tools are
  all `experimental` and not part of the scorecard's stable-tool
  coverage check (assumed — not verified).
- The drift is **not user-visible** because:
  - `cognicode-mcp/SKILL.md` lists the experimental tools once
    with no workflow around them.
  - The runtime's `_meta.stability` correctly labels them
    `experimental`.
  - No public skill or docs/MCP-TOOLS.md claim they are stable.

## Action

- Regenerate `docs/MCP-TOOLS.md` from the current runtime. This is
  a docs cleanup that does NOT require code changes; a future
  docs cycle can do it.
- For now, the per-tool drift table above is the load-bearing
  inventory. The new `validate_skills.py` enforces it via the
  runtime catalog (51 MCP tool refs in `cognicode-mcp` validated
  against the 73-tool catalog — so any future drift in either
  direction would be caught at validation time).

## Pre-existing diff in `docs/MCP-TOOLS.md`

`docs/MCP-TOOLS.md` has not been regenerated since 2026-08-06
(its own preamble). It claims 68 tools. The cycle's verification
report explicitly flagged this as pre-existing drift. The fix
(deferred to a docs-cleanup cycle) is: re-run the probe
(`bash sandbox/scripts/list_mcp_tools.sh`) and regenerate. **Not
fixed in e84.1.**

## Schema drift (input parameters)

The high-level drift report (`wu1-mcp-capability.md`) listed
parameter-name drift observed during WU18 UAT:

- `analyze_impact(symbol)` → `analyze_impact(symbol_name)`.
- `trace_path(symbol_a, symbol_b)` → `trace_path(source, target)`.
- `generate_contract` requires `file_path` and `function_name`.
- `safe_refactor` requires `action` and `target`.
- `reparse_on_edit` requires `file_paths` (list).
- `graph_explain` requires `symbol`.
- `check_architecture` accepts optional `scope`.

`docs/MCP-TOOLS.md` does not document input schemas (only name +
truncated description), so schema drift is invisible in the doc.
The user-facing skill (`cognicode-mcp`) was corrected in WU18 to
use the actual runtime parameter names.
