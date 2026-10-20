---
name: cognicode-developer
description: >
  Onboarding for CogniCode contributors. Trigger: When an agent or
  developer is working on the CogniCode project itself (graph store,
  MCP server, CLI, Explorer UI, distribution lifecycle) and needs
  the work-unit workflow, internal `just` recipes, architecture
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

1. `AGENTS.md` at the repository root — project conventions
   (recovery sequence, test discipline, commit messages, gates).
2. `docs/roadmap/ROADMAP.md` — the one agenda authority. What is
   open, and what each closed unit established.
3. `docs/roadmap/JOURNAL.md` — the append-only receipt of every
   closed unit: its SHA, its gates, its measurement.
4. `docs/adr/README.md` — table of contents for the 50+ ADRs. The
   ADRs are the source of design truth.

## Required process for any non-trivial change

A change of meaningful scope is a **work unit**: one contract, one
test that fails before and passes after, one receipt.

```text
read the agenda → write the failing test → fix it → run the gates → write the receipt
```

The unit's contract is stated before the code, and the receipt lands in
`docs/roadmap/JOURNAL.md` with the SHA, the gates that ran and the
measurement that closed it.

For tiny fixes (typos, single-line bug fixes, dependency bumps), a
direct commit is acceptable. Anything that touches public API,
distribution lifecycle, or product-surface contracts MUST go
through a work unit.

## Key development commands

```bash
# Build
just build                          # explorer API server (default)
just build-server                   # cogh + cognicode + cognicode-mcp only
just build-wasm                     # explorer frontend

# Test
just test-unit                      # 178+ unit tests, no Postgres
just test-pg                        # Postgres-backed integration tests
just check-known-failures           # known-failure baseline; local only, not a CI gate
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
- **Governance docs** (pushed to remote): `docs/roadmap/**` and
  `docs/adr/**`. Spanish.
- **Ephemeral notes** (working only, never pushed to remote):
  `AGENTS.local.md`, `CONTEXT.md`. Spanish.
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
- `docs/roadmap/ROADMAP.md` — the agenda authority.
- `docs/roadmap/JOURNAL.md` — the receipt of every closed unit.
- `docs/adr/README.md` — ADR index.
