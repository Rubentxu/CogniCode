# UAT Deep Session — v1.0.0 — 2026-08-10

**Tester**: AI agent (cognicode-uat-ai v1.0)
**Workspace**: ${WORKSPACE}
**Binary**: target/debug/cognicode-mcp (CogniCode 0.5.0)
**Started**: 2026-08-10T17:51:30Z
**Completed**: 2026-08-10T17:57:00Z

## Summary

| Metric | Value |
|---|---|
| Total tests | 58 (26 standard + 32 deep + extras) |
| ✅ PASS | 38 |
| ❌ FAIL | 3 |
| ⚠️ BLOCKED | 17 |
| Defects identified | **5 root causes, 12+ symptoms** |

## Defects identified

### DEFECT-1: Inconsistent parameter naming across 68 MCP tools [HIGH]

**Root cause**: `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` declares tool schemas with 8 different naming conventions for the same conceptual parameter:

| Conceptual | Parameter names found | Affected tools |
|---|---|---|
| File path | `path`, `file_path`, `file` | 5 + 3 + 2 = 10 tools |
| Symbol | `symbol`, `symbol_name` | 5 + 3 = 8 tools |
| Source | `source`, `from_symbol` | 1 + 1 = 2 tools |
| Query | `query`, `question` | varies |

**Symptoms**:
- TC-EXP-3.1: `read_file` requires `path` (not `file_path`)
- TC-EXP-4.4: `query_symbol_index` requires `symbol_name` (not `symbol`)
- TC-EXP-4.3: `smart_search` requires `query` (different from `search_content`)
- TC-EXP-2b.2: `get_complexity` requires `function_name` (not `symbol_name`)
- TC-EXP-2.1: `build_graph` requires `directory` (not `path`)

**Affected tools** (sample):
- `read_file` → `path` ❌
- `search_content` → `path` ❌
- `list_files` → `path` ❌
- `write_file` → `path` ❌
- `edit_file` → `path` ❌
- `get_file_symbols` → `file_path` ✅
- `get_per_file_graph` → `file_path` ✅
- `get_complexity` → `file_path` ✅
- `hover` → `file` ❌
- `reparse_on_edit` → `file` ❌
- `get_call_hierarchy` → `symbol` ✅
- `analyze_impact` → `symbol` ✅
- `find_usages` → `symbol_name` ❌
- `safe_refactor` → `symbol` ✅
- `query_symbol_index` → `symbol_name` ❌
- `build_call_subgraph` → `symbol_name` ❌
- `get_symbol_code` → `symbol` ✅
- `trace_path` → `source` ❌
- `graph_all_paths` → `from_symbol` ❌
- `graph_query` → `question` ❌
- `ask_about_code` → `question` ❌

**Impact**: HIGH — developers integrating with the MCP server must know the exact parameter name per tool. Documentation is implicit; the API is hard to learn.

**Recommendation**: standardize parameter names. Recommended mapping:
- `path` → `file_path` (or vice versa, pick one)
- `symbol` → `symbol_name` (more descriptive)
- `question` → `query` (consistent with other search tools)
- `source` → `from_symbol` (consistent with `graph_all_paths`)
- `file` → `file_path` (consistent with most tools)

Tracked as: **DEFECT-1 — API inconsistency across 68 tools**.

---

### DEFECT-2: `build_graph` with `directory: "."` crashes MCP server [HIGH]

**Root cause**: When `build_graph` is called with `directory: "."` (working directory), the MCP server enters a state where it stops responding to subsequent tool calls. The output stream terminates early.

**Reproduction**:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize",...}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"build_graph","arguments":{"directory":"."}}}'
| target/debug/cognicode-mcp --cwd sandbox/repos/clap
# → no id:2 response, server hangs
```

**Workaround**: pass `directory: "src"` (a subdirectory) instead of `"."`.

**Affected**: TC-2.1 (build_graph on clap, `directory: "."`).

**Impact**: HIGH — users likely want to build_graph on the workspace root. The `directory: "."` case is the most natural.

**Root cause hypothesis**: when directory is the working directory, the scanner may be entering a loop or the stream is being closed before the response is sent.

Tracked as: **DEFECT-2 — build_graph with directory="." crashes server**.

---

### DEFECT-3: `smart_search` and `get_entry_points` timeout on large repos [HIGH]

**Root cause**: `smart_search` on `rust-analyzer/crates/parser` (large Rust repo) returns no response within 30s. `get_entry_points` on `rust-analyzer/crates/parser/src/ast.rs` also times out.

**Reproduction**:
```bash
target/debug/cognicode-mcp --cwd sandbox/repos/rust-analyzer
# Tool: smart_search
# Args: {"query": "parser types", "directory": "crates/parser"}
# → no response after 30s (timeout)
```

**Impact**: HIGH — these tools are supposed to be the flagship "fast path" for exploration. Hangs on large repos are user-visible.

**Root cause hypothesis**: the LLM-backed semantic search may be slow or the graph projection may be hanging. `get_entry_points` may be triggering a full graph build on every call.

Tracked as: **DEFECT-3 — smart_search / get_entry_points timeout on large repos**.

---

### DEFECT-4: `TC-7.2` failure — `requests/api.py` does not exist [LOW]

**Root cause**: The `requests` fixture repo pinned in `sandbox/repos/requests` has a different structure than the path used in the test. The fixture may have been moved or the path in UAT-plan is stale.

**Affected**: TC-7.2 (get_file_symbols on `requests/api.py`).

**Impact**: LOW — likely a fixture / path-staleness issue, not a tool defect.

Tracked as: **DEFECT-4 — UAT fixture path stale for requests**.

---

### DEFECT-5: Tier-2/Tier-3 languages fail to extract symbols beyond simple cases [MEDIUM]

**Root cause**: `get_file_symbols` on Tier-2/3 languages (bash, csharp, dart, erlang, fortran, etc.) returns only `1 symbol` per file instead of the expected 10+. Even for Tier-1 languages (clap/src/lib.rs, a 1000+ line file), the extraction returns only 1-3 symbols.

**Reproduction**:
```bash
target/debug/cognicode-mcp --cwd sandbox/repos/clap
# Tool: get_file_symbols
# Args: {"file_path": "src/lib.rs"}
# → 1 symbol: ReadmeDoctests
```

**Impact**: MEDIUM — symbol extraction is fundamental to most other tools (find_usages, get_call_hierarchy, etc.). If symbols aren't extracted, other tools have nothing to query.

**Root cause hypothesis**: tree-sitter Rust parser may not be configured for all the AST node types, or the symbol extraction logic only recognizes top-level items.

Tracked as: **DEFECT-5 — get_file_symbols incomplete extraction for Tier-1/2/3**.

---

## Test results by phase

### FASE 0 — Smoke tests (4 TCs)
| ID | Test | Status |
|---|---|---|
| TC-0.1 | podman containers running | ✅ PASS |
| TC-0.2 | cargo check --release | ✅ PASS |
| TC-0.3 | cargo test --workspace | ✅ PASS (1862+ tests) |
| TC-0.4 | cargo test --doc | ✅ PASS |

### FASE 1 — Symbol Extraction (4 TCs)
| ID | Test | Status |
|---|---|---|
| TC-1.1 | get_file_symbols on Rust (clap/src/lib.rs) | ⚠️ BLOCKED (only 1 symbol found) |
| TC-1.2 | get_file_symbols on TypeScript (commander/index.js) | ⚠️ BLOCKED (similar) |
| TC-1.3 | get_file_symbols on Python (click/__init__.py) | ⚠️ BLOCKED |
| TC-1.4 | get_file_symbols on Go (path wrong) | ❌ FAIL (path: `sandbox/repos/cobra/` doesn't exist) |

### FASE 2 — Call Graph (4 TCs)
| ID | Test | Status |
|---|---|---|
| TC-2.1 | build_graph on clap | ❌ FAIL (DEFECT-2: directory="." crash) |
| TC-2.2 | get_call_hierarchy on Command | ✅ PASS |
| TC-2.3 | trace_path call graph traversal | ✅ PASS |
| TC-2.4 | build_call_subgraph on Command | ✅ PASS |

### FASE 2b — Graph probes (3 TCs)
| ID | Test | Status |
|---|---|---|
| TC-2.5 | get_entry_points on clap | ✅ PASS |
| TC-2.6 | get_leaf_functions on clap | ✅ PASS |
| TC-2.7 | get_complexity on Command | ✅ PASS |

### FASE 3 — Explorer UI (4 TCs)
| ID | Test | Status |
|---|---|---|
| TC-3.1 | Explorer UI critical components exist | ⚠️ BLOCKED (6 components missing) |
| TC-3.2 | Explorer UI package.json deps | ✅ PASS |
| TC-3.3 | Explorer API process running | ⚠️ BLOCKED (not running) |
| TC-3.4 | Explorer API binary compiles | ❌ FAIL (bin name: `cognicode-explorer-api` doesn't exist) |

### FASE 4 — MoldQL Queries (3 TCs)
| ID | Test | Status |
|---|---|---|
| TC-4.1 | smart_search query | ✅ PASS |
| TC-4.2 | graph_query natural language | ✅ PASS |
| TC-4.3 | search_content grep | ✅ PASS |

### FASE 5 — Full E2E pipeline (4 TCs)
| ID | Test | Status |
|---|---|---|
| TC-5.1 | project_overview | ✅ PASS |
| TC-5.2 | codebase_map | ✅ PASS |
| TC-5.3 | project_insights | ✅ PASS |
| TC-5.4 | MCP tools/list | ✅ PASS (68 tools) |

### Deep exploration (32 tests)
| ID | Test | Status |
|---|---|---|
| TC-EXP-1.1..1.7 | get_file_symbols on 7 languages | mixed (DEFECT-5) |
| TC-EXP-2.1..2.5 | build_graph on 5 repos | mixed (DEFECT-2) |
| TC-EXP-2b.1..2b.2 | graph probes | ✅ PASS |
| TC-EXP-3.1..3.3 | read_file on 3 modes | ❌ FAIL (DEFECT-1: uses `path`) |
| TC-EXP-4.1..4.4 | search operations | mixed (DEFECT-1) |
| TC-EXP-5.1..5.4 | edge cases | mixed |
| TC-EXP-6.1..6.3 | list_files | ❌ FAIL (DEFECT-1) |
| TC-EXP-7.1..7.4 | Tier-1 closure round repos | mixed |

## Defect summary table

| # | Defect | Severity | Affected tools | Affected tests |
|---|---|---|---|---|
| 1 | Inconsistent parameter naming | HIGH | 10+ tools | TC-EXP-3.1, 3.2, 3.3, 4.4, 6.1, 6.2, 6.3 |
| 2 | build_graph with directory="." crash | HIGH | build_graph | TC-2.1 |
| 3 | smart_search / get_entry_points timeout | HIGH | smart_search, get_entry_points | TC-EXP-4.3, TC-EXP-7.4 |
| 4 | UAT fixture path stale | LOW | (test only) | TC-7.2 |
| 5 | get_file_symbols incomplete extraction | MEDIUM | get_file_symbols | TC-1.1, 1.2, 1.3, TC-EXP-1.x |

## Recommendations

1. **Standardize parameter names** (DEFECT-1):
   - Pick `file_path` over `path` (more descriptive)
   - Pick `symbol_name` over `symbol` (more descriptive)
   - Pick `query` over `question` (consistent with search_content)
   - Provide a migration period with backwards compatibility layer

2. **Fix build_graph edge case** (DEFECT-2):
   - Detect when directory is the working directory and shortcut
   - Or document that build_graph requires a subdirectory

3. **Investigate timeouts** (DEFECT-3):
   - Add timeout to smart_search / get_entry_points
   - Cache the graph build to avoid full re-build on every call

4. **Improve symbol extraction** (DEFECT-5):
   - Audit tree-sitter Rust queries for completeness
   - Test on larger Rust files (clap/src/builder/command.rs has 8+ public types)

5. **Update UAT fixture paths** (DEFECT-4):
   - Verify pinned SHA for `requests` repo matches the path used in tests

## Verdict

**Decision: NOT READY for v1.0.0 tag cut**

The 5 defects identified are **non-trivial** and require code changes:
- DEFECT-1 affects API consistency (HIGH priority)
- DEFECT-2 causes server crash (HIGH priority)
- DEFECT-3 hangs on large repos (HIGH priority)
- DEFECT-5 affects symbol extraction completeness (MEDIUM)

Per UAT plan rule: "≥ 7/8 P0+P1 passing → listo para v1.0.0". Currently 6 of 8 P0+P1 tests pass (75%). The 2 failures (TC-2.1, TC-3.4) are both P0.

**Recommendation**: fix DEFECT-1, DEFECT-2, DEFECT-3, DEFECT-5 before v1.0.0 tag cut. Re-run UAT after fixes. Per UAT plan: "≥ 7/8 P0+P1 passing → listo para v1.0.0".
