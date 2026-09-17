---
name: cognicode-developer
description: >
  Onboarding for CogniCode contributors. Trigger: When an agent or
  developer is working on the CogniCode project itself (graph store,
  MCP server, CLI, Explorer UI, distribution lifecycle) and needs
  the SDDK/OpenSpec workflow, internal `just` recipes, architecture
  rules, and release gates. NOT for users of CogniCode — load
  `cognicode` or `cognicode-mcp` instead.
license: MIT
metadata:
  version: "1.0.0"
  maturity: stable
  author: CogniCode Team
  homepage: https://github.com/Rubentxu/CogniCode
---

# CogniCode developer onboarding

This skill is for **people contributing to the CogniCode project
itself**. If you are a user of CogniCode (you installed it via
`cogh` and want to analyse your own project), load
`skills/cognicode` or `skills/cognicode-mcp` instead.

## Required setup

- A working Rust toolchain (stable, 1.85+).
- A clone of this repository at the branch you intend to work on.
- `just` installed (https://github.com/casey/just).

## Required reading

1. `docs/adr/README.md` — table of contents for the 50+ ADRs. The
   ADRs are the source of design truth.
2. `openspec/CHANGES.md` (if present) or `openspec/changes/`
   directory — active and archived cycles.
3. `AGENTS.md` at the repository root — project conventions
   (architecture, ephemeral vs permanent docs, commit messages,
   testing).

## Required process for any non-trivial change

The project uses **SDDK + OpenSpec**. A change of meaningful scope
goes through a cycle:

```text
explore → propose → spec → design → tasks → apply → verify → release → archive
```

The exact path (B-direct / A-min / A-lite / A-full) is decided by
the orchestrator agent based on complexity.

For tiny fixes (typos, single-line bug fixes, dependency bumps), a
direct commit is acceptable. Anything that touches public API,
distribution lifecycle, or product-surface contracts MUST go
through a cycle.

## Key development commands

```bash
# Build
just build                          # explorer API server (default)
just build-server                   # cogh + cognicode + cognicode-mcp only
just build-wasm                     # explorer frontend

# Test
just test-unit                      # 178+ unit tests, no Postgres
just test-pg                        # Postgres-backed integration tests
just check-known-failures           # guard 41-entry baseline
just lint                           # clippy as errors

# Lifecycle
just dev                            # full dev mode (Postgres + API + UI)
just run                            # run the explorer API
just start                          # run without rebuilding

# Distribution
just build-release                  # release-mode binaries
```

## Repository conventions

- **Architecture:** hexagonal (ports & adapters) + SOLID, unless an
  ADR or task explicitly states otherwise.
- **Ephemeral docs** (working only, never pushed to remote):
  `docs/adr/**`, `docs/ROADMAP.md`, `CONTEXT.md`. Spanish.
- **Permanent docs** (pushed to remote): everything else.
- **Commit messages:** Conventional Commits format. **Never** add
  `Co-Authored-By: AI`.
- **Domain code** (`crates/cognicode-core/src/domain/`) MUST NOT
  import `sqlx`, `tokio`, or any I/O crate — go through ports.

## Distribution lifecycle (e84/e85/e86)

The distribution is split across three cycles:

- **e84 (closed):** contract — `cogh` lifecycle CLI + portable
  skill bundles.
- **e85:** release factory — Linux release pipeline (not yet
  implemented in this cycle).
- **e86 (closed):** lifecycle — `install` / `latest` / `update` /
  `rollback` for the user-facing runtime.

e84.1 (this skill's source cycle) added the **productization** of
the skill bundles: USER skills vs DEVELOPER skill separation,
public MCP catalog as source of truth, no dev-repo-specific paths
in user skills.

## What this skill does NOT do

- It does not teach you how to **use** CogniCode on a third-party
  project. Load `cognicode` or `cognicode-mcp`.
- It does not enumerate every `just` recipe. `just --list` is the
  source of truth.
- It does not document the LSI roadmap. That's in the ADRs.

## See also

- `AGENTS.md` at the repo root — full conventions list.
- `openspec/changes/` — current and past cycles.
- `docs/adr/README.md` — ADR index.
