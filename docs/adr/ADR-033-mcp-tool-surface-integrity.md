# ADR-033: MCP Tool Surface Integrity — Phase 0

**Status:** Accepted (Phase 0)
**Date:** 2026-06-16
**Source:** Follow-up to ADR-027..ADR-032. Smoke testing revealed listed/unique/ghost tool count violations.

## Context

Smoke testing of the MCP server (`scripts/mcp/mcp_smoke_all.py`) revealed three tool surface integrity violations:

| Metric | Value | Status |
|--------|-------|--------|
| `listed_total` | 68 | — |
| `unique_listed` | 65 | ❌ (3 duplicates) |
| `missing` | 1 | ❌ (`get_graph_report` ghost) |

**Duplicate registrations** — three tools appear twice in `tools/list` with different schemas:

- `solid_audit`: first registration (canonical) defaults `{}`; second registration is identical.
- `graph_diff`: first registration (canonical) defaults `current: false`; second defaults `current: true` — a schema conflict that misleads MCP clients.
- `graph_timeline`: first registration (canonical) has a complete schema; second has an abbreviated description.

**Ghost tool** — `get_graph_report` is registered in `tools/list` but has **no dispatch arm**. Calls fall through to `ToolNotFound`. The handler returns `report: None` with a stale Sprint 2 message and is unreachable from dispatch.

## Decision

### 1. Extract `build_all_tools()` as single source of truth

Moved the inline `vec![Tool::new(...)]` from `fn list_tools()` into a new `pub(crate) fn build_all_tools() -> Vec<Tool>` at module level. `list_tools` now calls `let all_tools = build_all_tools();`.

**Rationale**: The "no duplicate names" invariant can only be verified against the real list — a snapshot list would be pre-deduped and cannot detect duplicates added to the real vec. Extraction makes the surface testable.

### 2. Delete the three duplicate registrations

Removed the second `solid_audit`, `graph_diff`, and `graph_timeline` blocks (lines 720–734 in pre-change source). Kept the canonical first registrations which have the better-specified schemas.

**Schema conflict detail**: the duplicate `graph_diff` declared `"current": { "type": "boolean", "description": "Compare against latest (default: true)" }` while the canonical declares `"current": { ... "default": false }`. Clients receiving both would see ambiguous behavior.

### 3. Delete ghost `get_graph_report` listing

Removed `Tool::new("get_graph_report", ...)` from `build_all_tools()`. The tool is absent from `tools/list` after Phase 0.

### 4. Delete stale placeholder handler

Removed `GetGraphReportInput`, `GetGraphReportOutput`, and `handle_get_graph_report` from `graph_query_handlers.rs`. The handler was unreachable from dispatch and returned a stale Sprint 2 message.

**No public client depends on `get_graph_report`** — verified by workspace grep. Future replacement requires a follow-up ADR with a real PG-backed implementation.

### 5. Add list↔dispatch parity tests

Added `tool_surface_parity` test module to `mcp_roundtrip_tests.rs`:

- `test_no_duplicate_tool_names` — asserts `HashSet::len == Vec::len` over `build_all_tools()` names
- `test_get_graph_report_not_listed` — asserts `"get_graph_report"` absent from `build_all_tools()`
- `test_all_listed_tools_are_dispatchable` — asserts every listed tool appears in the static dispatch allowlist
- `test_allowlist_subset_of_listed` — asserts the allowlist doesn't drift from actual tools

The static allowlist is the compile-time-checked proxy for async dispatch probing (which requires `rmcp::service::Peer::new` — `pub(crate)`).

### 6. Stage durable smoke baseline

`scripts/mcp/mcp_smoke_all.py` now supports:

- `--help` — documents the classification taxonomy and CLI surface
- `--persist <path>` — writes baseline JSON with `listed_total`, `unique_listed`, `duplicates`, `missing`, `gated`, per-tool classification
- `GATED` module constant — documents runtime-gated tools (`graph_diff`, `graph_timeline`, `generate_contract`) that return explicit capability errors, not `ToolNotFound`

## Consequences

| Metric | Before | After |
|--------|--------|-------|
| `listed_total` | 68 | 64 |
| `unique_listed` | 65 | 64 |
| Duplicates | 3 (`solid_audit`, `graph_diff`, `graph_timeline`) | 0 |
| Ghost (`get_graph_report`) | 1 | 0 |
| Unit tests added | 0 | 4 |

- No new public API. `build_all_tools()` is `pub(crate)` — test-accessible only.
- No external consumer regression. The ghost already returned `ToolNotFound`; duplicates already confused clients.
- `get_graph_report` re-advertisement requires a follow-up ADR with a real PG-backed `graph_report` implementation.

## Alternatives Considered

### Snapshot dedup test
A separate snapshot list could detect duplicates — but the snapshot itself would be deduplicated, masking duplicates added to the real `vec!`. The extraction approach makes the single source of truth testable at source.

### `#[allow(dead_code)]` on placeholder
Signals false intent. The handler, input, and output types exist in the source with no callers. Deletion is cleaner and prevents future confusion.

### Rewrite ADR-025/026/027
Those ADRs describe `get_graph_report` as available. Rewriting accepted ADRs is forbidden. Cross-referencing from this ADR preserves the history.

## Out of Scope (Future ADRs)

- **68→35 semantic consolidation** — distinct tools with overlapping functionality
- **Real PG-backed `graph_report`** — replaces the ghost when GraphReport persistence is ready
- **CODEOWNERS** — ownership for tool surface changes
- **Capability/config gating fixes** — `generate_contract`, `graph_diff`, `graph_timeline` remain listed and return explicit capability errors at runtime (classified as `GATED_OK`)

## Rollout

Two slices:

- **PR-A** (extraction): `build_all_tools()` moved to module level. Pure move, `git diff --color-moved` verifies zero semantic change. ~5 min review.
- **PR-B** (apply): ghost removal, duplicate deletion, parity tests, smoke script, ADR-033. ~20 min review.

Rollback = `git revert <sha>` for either PR. All changes are line-level removals or new files.

## Verification Log

- **Explore**: ✅ done (smoke baseline showing listed=68, unique=65, ghost=1)
- **Propose**: ✅ done (engram://2131)
- **Spec**: ✅ done (engram://2132)
- **Design**: ✅ done (engram://2133)
- **Tasks**: ✅ done (engram://2134)
- **Apply**: ✅ done (this ADR documents the applied state)
- **Verify**: ✅ done — parity tests pass; live smoke reports listed=64,
  unique=64, duplicates=0, missing=0
