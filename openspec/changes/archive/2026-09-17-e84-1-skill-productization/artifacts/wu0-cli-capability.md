# WU0 — CLI capability truth (cognicode + cogh)

Captured from release binaries built at HEAD `46ca3c48`
(`target/release/cognicode`, `target/release/cogh`).

Raw `cognicode --help` and `cogh --help` transcripts:
`artifacts/cognicode-help.txt`, `artifacts/cogh-help.txt`.

## Status legend

- **REAL** — command works, has tests, has documentation.
- **PARTIAL** — works but only for some inputs / partial coverage.
- **STUB** — present in `--help`, returns early or "not implemented".
- **FEATURE-GATED** — compiled in only with a non-default Cargo feature.
- **INTERNAL** — not part of the public product surface; surfaced by
  accident or for debugging.

## `cognicode` (analysis CLI)

| Command | Status | Notes |
|---------|--------|-------|
| `analyze [PATH]` | REAL | Default `.`. Drives the LSP-based analyzer. |
| `serve [--port]` | REAL | Starts the MCP server (HTTP transport). |
| `refactor [op] <symbol> [new_name]` | REAL | Operations: `rename`, `extract`, `inline`, `move`. |
| `index build` | REAL | Lightweight symbol index. |
| `index query` | REAL | Query by symbol name. |
| `index outline` | REAL | Hierarchical file outline. |
| `index symbol-code` | REAL | Source of a symbol. |
| `graph on-demand` | REAL | On-demand call subgraph. |
| `graph per-file` | REAL | Per-file graph. |
| `graph full` | REAL | Full project graph. |
| `graph hot-paths` | REAL | Most-called functions. |
| `graph entry-points` | REAL | No incoming edges. |
| `graph leaf-functions` | REAL | No outgoing edges. |
| `graph trace-path` | REAL | Execution path between two symbols. |
| `graph mermaid` | REAL | Export to Mermaid. |
| `graph hierarchy` | REAL | Call hierarchy for a symbol. |
| `graph complexity` | REAL | Complexity metrics. |
| `graph impact` | REAL | Impact of changing a symbol. |
| `navigate definition` | REAL | LSP go-to-definition. |
| `navigate hover` | REAL | LSP hover. |
| `navigate references` | REAL | LSP find-references. |
| `doctor [--format] [--cwd]` | REAL | LSP server availability. |
| `docs-ingest` | FEATURE-GATED | Requires `multimodal` Cargo feature. |
| `issues-ingest` | FEATURE-GATED | Requires `multimodal` Cargo feature. |
| `help` | REAL | |

## `cogh` (lifecycle CLI)

| Command | Status | Notes |
|---------|--------|-------|
| `install <plugin>` | REAL | e86 resolver-driven install. |
| `uninstall --version <v> <plugin>` | REAL | |
| `list` | REAL | |
| `current` | REAL | Active version pin. |
| `latest [plugin] [--all] [--json]` | REAL | e86 resolver-driven latest. |
| `update [plugin] [--profile] [--dry-run]` | REAL | e86 resolver-driven update. |
| `reshim` | REAL | Regenerate shims. |
| `rollback [plugin]` | PARTIAL | Has pinned regression (T3 — populated-dir rollback fails). |
| `doctor` | REAL | Validate install. |
| `where <binary>` | REAL | |
| `init` | REAL | |
| `plugin add` | PARTIAL | "from git-url or registered marketplace" — marketplace registration not implemented. |
| `plugin remove` | REAL | |
| `plugin list` | REAL | |
| `plugin update` | REAL | |
| `skill validate` | REAL | Static skill bundle validator. |
| `ide detect` | REAL | |
| `ide install` | PARTIAL | Has pinned regression (T2b — stale shim on second install). |
| `ide uninstall` | REAL | |
| `version` | REAL | |

## Surprises / observations

1. **`docs-ingest` and `issues-ingest` are feature-gated.** They show
   up in `--help` but are absent from default builds. This is
   misleading for a public skill: the help text says "Unknown command"
   for users on the default binary. Either hide them from `--help` in
   default builds (so the help is honest) or document them as
   opt-in features. **Out of scope for e84.1; recorded in WU19.**

2. **`cogh rollback` is PARTIAL.** T3 pinned regression: rollback
   fails on a populated `CreatedDir` because `rmdir` errors on a
   non-empty directory and the journal does not record individual
   files extracted. **Pre-existing bug, pinned not fixed in e86
   followup; out of scope for e84.1.**

3. **`cogh ide install` is PARTIAL.** T2b pinned regression: stale
   shim EEXIST on second install. **Pre-existing bug, pinned not
   fixed in e86 followup; out of scope for e84.1.**

4. **`cogh plugin add` is PARTIAL.** Marketplace registration is
   not implemented. The command accepts a git-url. **Stub-by-design;
   out of scope for e84.1.**

5. **`cogh skill` has only `validate` today.** No `install`/`update`/
   `remove` (explicitly non-goal in the cycle brief: "do NOT
   implement `cogh skill install` yet"). The validator is the only
   portable skill surface in this cycle.

6. **`cogh update` and `cogh latest` are resolver-driven.** Both
   accept `--base-url` and `--staging` for air-gapped installs and
   tests. This is the surface the new ResolverFixture (e86 followup)
   exercises.

7. **`cogh version` and `cogh doctor` exist.** Useful for agents
   that want to verify the installed runtime version before invoking
   skills that depend on a specific version.
