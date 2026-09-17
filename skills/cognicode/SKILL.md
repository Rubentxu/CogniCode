---
name: cognicode
description: >
  General entry point for using CogniCode on a project you didn't write.
  Trigger: When a user/agent wants to discover, navigate, or lightly
  analyze a codebase using CogniCode and needs to choose between the
  CLI and MCP surfaces. Applies to: project discovery, symbol lookup,
  deciding which CogniCode workflow to start with.
license: MIT
metadata:
  version: "1.0.0"
  maturity: stable
  author: CogniCode Team
  homepage: https://github.com/Rubentxu/CogniCode
---

# CogniCode — entry point

CogniCode is a code-analysis companion you can drive two ways:

- **CLI** — `cognicode` and `cogh` shell commands. Best for scripts,
  CI, and one-shot operations.
- **MCP** — a 73-tool server an agent drives through JSON-RPC. Best
  for multi-step, interactive reasoning.

This skill teaches the smallest set of commands and tools you need
to **get oriented** in a project. Deeper workflows live in
`cognicode-mcp`.

## When to load this skill

- A user says "what is this codebase?" or "find the function that
  does X".
- An agent is dropped into an unfamiliar repo and wants a map.
- A new install of CogniCode is being verified for the first time.

If the task is **multi-step graph exploration, impact analysis, or
safe refactoring**, load `cognicode-mcp` instead.

## Step 1 — verify the install

```bash
cogh version
cogh doctor
cognicode --version
cognicode doctor
```

If `cogh doctor` reports issues, fix them before proceeding.

## Step 2 — pick the surface

```text
quick scripted analysis         →  CLI
interactive agent reasoning     →  MCP
visual / graph exploration      →  Explorer (browser UI)
```

For an agent-driven session, **default to MCP**. The CLI is best
when the question is well-formed and the answer is small.

## Step 3 — first discovery

### Via the CLI

```bash
# Shallow discovery: counts and entry points
cognicode analyze .

# Per-file outline
cognicode index build
cognicode index outline path/to/file.rs

# Where does this symbol live?
cognicode index query MySymbol
```

### Via the MCP

Ask the server for an overview in three calls:

```json
{"method": "tools/call", "params": {"name": "project_overview",
 "arguments": {"directory": ".", "level": "quick"}}}
```

```json
{"method": "tools/call", "params": {"name": "project_insights",
 "arguments": {"directory": "."}}}
```

```json
{"method": "tools/call", "params": {"name": "codebase_map",
 "arguments": {"directory": ".", "format": "compact"}}}
```

If a tool returns an error, the most common cause is that the
workspace path is wrong. Re-issue with an absolute path.

## Step 4 — locate code

For a symbol you can name:

```bash
cognicode index query MySymbol
cognicode index symbol-code MySymbol
```

Or via MCP:

```json
{"method": "tools/call", "params": {"name": "query_symbol_index",
 "arguments": {"name": "MySymbol"}}}
{"method": "tools/call", "params": {"name": "get_symbol_code",
 "arguments": {"file_path": "src/foo.rs", "line": 42, "column": 5}}}
```

## Step 5 — when to load the deeper skill

Stop here and load `cognicode-mcp` when any of these is true:

- You need the **call graph** of the project (`build_graph`,
  `graph_insights`, `graph_communities`).
- You are about to **change code** and need an impact analysis.
- You need to **audit quality** (SOLID violations, god functions).
- You need to **safely mutate** code (refactor + contract
  validation).

## What this skill does NOT do

- It does not enumerate MCP tools. The runtime catalog is the source
  of truth; query `tools/list` on the running server.
- It does not teach refactor workflows. Load `cognicode-mcp`.
- It does not assume you have a CogniCode source checkout. You
  shouldn't need one.

## Conventions

- **Paths in this skill are portable.** No Cargo build paths, no
  local-machine absolute paths.
- **Versions are not pinned.** `cogh`/`cognicode` commands work
  against the version you have installed. Use `cogh version` to
  check what you have.
- **Read before mutate.** This skill only reads. If you need to
  modify the user's code, load `cognicode-mcp` and follow its
  safe-change workflow.
