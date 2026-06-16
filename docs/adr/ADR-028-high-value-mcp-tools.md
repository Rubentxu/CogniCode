# ADR-028: High-Value MCP Tools — Combining Graphify + CogniCode Capabilities

**Status:** Accepted  
**Date:** 2026-06-16  
**Source:** Graphify feature analysis + tool consolidation audit

## Context

CogniCode MCP has 67 tools after consolidation (ADR-027) reduced to ~49.
Graphify offers several high-value features we lack:
- `add <url>` — external content ingestion
- `prs` / `triage_prs` — PR dashboard with graph impact

Conversely, CogniCode has unique capabilities Graphify doesn't:
- Type-ref extraction (References edges → SOLID analysis)
- IaC extraction (Terraform + Ansible in the same graph as code)
- LSP integration, safe refactoring, AVC contracts, file operations

This ADR defines 8 new tools that combine the best of both worlds, organized
by value-to-effort ratio.

## Decision

### Tier 1 — Agent-Optimized Composites (1 day each)

#### `codebase_map`

Generates a compact, structured map of the codebase optimized for LLM agent
system prompts and context windows.

```json
{
  "tool": "codebase_map",
  "arguments": { "format": "compact" }
}
```

**Algorithm:**
1. `get_entry_points` → API surface (HTTP routes, CLI commands, public functions)
2. `graph_insights` → communities, god nodes, health score
3. `get_hot_paths` → most-interconnected modules
4. Compress to token budget: `compact` (~400 tokens) or `detailed` (~2000 tokens)

**Output example (compact):**
```
Project: cognicode-core | 3421 symbols | Health: 87/100
Entry points: POST /api/users, cognicode analyze, UserCreated event
Key modules: auth (12 symbols, 8 callers), database (8 symbols), api (15 symbols)
God nodes: UserRepository (98 callers), AuthMiddleware (76 callers)
Hot paths: validate_token → check_permissions → query_db
```

#### `project_insights`

Composite dashboard in a single call. Replaces 5+ individual tool calls.

```json
{ "tool": "project_insights", "arguments": {} }
```

Returns: `graph_insights` + `get_hot_paths` + `detect_god_functions` +
`detect_long_parameter_lists` + `get_entry_points` + `health_score` + `dead_code`

**Token savings:** ~500 tokens vs 5 separate calls (~1500 tokens).

#### `review_pr` (v1)

PR review assisted by the graph. The agent provides the list of changed files;
the tool returns impact analysis for each.

```json
{
  "tool": "review_pr",
  "arguments": { "files": ["src/auth/middleware.rs", "src/api/users.rs"] }
}
```

**Algorithm:**
1. For each changed file, find all symbols defined in that file
2. `analyze_impact` on each changed symbol
3. `detect_api_breaks` between baseline and current
4. Return: impacted files, risk level, breaking changes, suggested reviewers

v2 (future): full git integration with `review_pr(branch)`.

### Tier 2 — Specialized Analyzers (1-2 days each)

#### `solid_audit`

Runs SOLID principle analysis over the entire graph, leveraging the unique
type-reference edges that CogniCode extracts.

```json
{ "tool": "solid_audit", "arguments": {} }
```

**Analysis per principle:**

| Principle | Detection | Edge types used |
|-----------|-----------|----------------|
| **SRP** | Fan-out > 10 with diverse caller communities | Calls, References |
| **OCP** | Classes extended vs modified (from git history) | Inherits, Implements |
| **LSP** | Subtypes with different signatures than parent | Inherits, Calls |
| **ISP** | Traits with > 5 methods and < 2 implementors | Contains, Implements |
| **DIP** | Dependencies on concrete types vs abstractions | References (param_type, return_type, field_type) |

**Output:** violations with severity, file location, and fix suggestions.

#### `iac_query`

Navigates the infrastructure graph. Unique to CogniCode — no other tool
combines code and IaC in a single queryable graph.

```json
{
  "tool": "iac_query",
  "arguments": {
    "resource_id": "tf:main.tf:aws_instance.web",
    "depth": 2
  }
}
```

**Algorithm:** BFS from the IaC node through References edges. Returns the
subgraph of resources that depend on or are depended upon by the target.

#### `graph_diff`

Compares two snapshots of the graph stored in `graph_reports`.

```json
{
  "tool": "graph_diff",
  "arguments": {
    "baseline_id": "report-20260601",
    "current": true
  }
}
```

**Algorithm:**
1. Load both `GraphReport` snapshots from `graph_reports` table
2. Diff communities, god nodes, dead code, health score
3. Return: added/removed/changed metrics with trend direction

#### `graph_timeline`

Temporal evolution of the graph.

```json
{
  "tool": "graph_timeline",
  "arguments": { "days": 30 }
}
```

**Algorithm:** Load last N reports from `graph_reports`, compute trend lines
for symbol_count, edge_count, health_score, community_count.

### Tier 3 — Future

#### `add_url`

Fetch external content (docs, papers) and ingest into the graph. Requires:
- HTTP client (reqwest)
- LLM for text extraction (Claude/Gemini/OpenAI gateway)
- Integration with the ingest pipeline

Deferred until multimodal extraction is implemented (Phase 2).

## Tool lifecycle

All new tools follow the ToolHandler registry pattern (ADR-010). Each tool is
a `dyn ToolHandler` registered by name. Tools require `build_graph` first and
read from `GraphCache::get()` (ArcSwap, lock-free).

## Consequences

- 8 new tools added to the MCP catalog.
- `codebase_map` and `project_insights` are the highest-value additions for
  AI agent workflows — they reduce token consumption and round-trips.
- `solid_audit` depends on type-ref extraction working correctly (ADR-018).
- `graph_diff` and `graph_timeline` depend on `graph_reports` being populated
  by the pipeline's Report stage (ADR-017).
- `review_pr` v2 requires git integration (future work).

## Alternatives Considered

- **All-in-one "analyze" tool:** rejected — too much output for a single
  response. Composites like `project_insights` are curated subsets.
- **Graphify's exact tool set:** rejected — CogniCode has unique capabilities
  (IaC, type-refs, LSP) that warrant dedicated tools.
- **No new tools, just consolidation:** rejected — consolidation alone doesn't
  add value. We need tools that leverage CogniCode's unique data (type-refs,
  IaC, graph_reports).
