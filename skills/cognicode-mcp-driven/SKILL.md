---
name: cognicode-mcp-driven
description: >
  Drive CogniCode engineering workflows via the 68-tool MCP server.
  Trigger: When analyzing, refactoring, navigating, or auditing a
  Rust+LadybugDB codebase (CogniCode and similar projects).
  Applies to: exploration, graph analysis, E2E pipeline, sandbox runs.
license: MIT
metadata:
  version: "0.92.0"
  maturity: stable
  author: CogniCode Team
  homepage: https://github.com/Rubentxu/CogniCode
  output-schema: mcp-call-result-v1
---

# Objective

Drive CogniCode engineering workflows through the 68-tool MCP server.
This skill is IDE-agnostic — the same body works in OpenCode, ZCode,
Claude Code, Codex, and future agentic IDEs.

# Required process

When a development task requires MCP-driven analysis, follow this loop:

1. Locate the workspace:
   ```bash
   pwd
   # or for the local Cognitive repo:
   cd /var/home/rubentxu/Proyectos/rust/CogniCode
   ```

2. Spin up the MCP server (use the local dev binary):
   ```bash
   target/debug/cognicode-mcp --cwd .
   ```

3. Probe the tool surface:
   ```bash
   target/debug/cognicode-mcp --cwd . <<EOF
   {"jsonrpc":"2.0","id":1,"method":"tools/list"}
   EOF
   ```

4. Pick the right tool for the question (graph, search, edit, etc.).

5. Validate findings against the test plan (see `docs/TEST-PLAN.md`).

# Required conventions

- **Naming**: portable skill. No `compatibility: opencode`. Drop this
  field from forked skills.
- **Layout**: `SKILL.md` (body) + `manifest.yaml` (cogh metadata) +
  `references/` (scripts) + `assets/` (data files).
- **Frontmatter**: `name`, `description`, `license`, `metadata.version`
  mandatory. `maturity` ∈ {experimental, beta, stable, deprecated}.

# Common tools to reach for

- `build_graph` — initial project graph
- `get_file_symbols` — symbols per file
- `get_call_hierarchy` — caller/callee of a symbol
- `trace_path` — execution path between two symbols
- `graph_explain` — composite deep-dive on a symbol
- `search_content` — fast grep over a directory
- `smart_search` — semantic search
- `query_symbol_index` — symbol location lookup
- `detect_drift` — docstring vs body diff
- `find_pattern_by_intent` — natural-language pattern match
- `solid_audit` — SOLID violations
- `reparse_on_edit` — incremental reindex after file edits

# Errors and recovery

- **Tool errors**: check the `isError` field. If true, the `content[0].text`
  contains the error message (often operational, not a bug).
- **Cold cache**: if `build_graph` returns fewer nodes than expected,
  re-run with `--directory` pointing at a subdirectory, not ".".
- **Path mismatch**: `read_file` uses `path` (not `file_path`); other
  tools use `file_path`. See `cogh doctor` for tool schema validation.

# CI gates (use `cogh` automation)

```bash
just post-e31-audit                # 41 invariants
just ci-t6                       # T6 regression test (LOCAL only)
just scorecard-nightly           # 5-night T7 cadence
just scorecard-streak            # 3-run E31-G counter
```

# Cross-references

- `docs/TEST-PLAN.md` — full test strategy
- `docs/RELEASE-1.0.0-PLAN.md` — scorecard framework
- `docs/adr/ADR-031-release-1.0.0-definition.md` — v1.0.0 source-of-truth
- `docs/specs/portable-skill-bundle/spec.md` — portable skill format
