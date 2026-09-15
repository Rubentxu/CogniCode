# Exploration Report — cycle e58 — known-failure baseline

> Cycle: B-direct (housekeeping) | Phase: explore | Date: 2026-09-15

## Trigger

`cargo test -p cognicode-core --lib` has 41 long-standing failures in
unrelated areas (file_operations, MCP file handlers, security boundary,
workspace_session, refactor/validation handlers, one M5 acceptance test).
They are environment/cwd-dependent and confirmed pre-existing by A/B stash
(see e56). Left as "usual red", they would mask a real regression.

## Evidence

```

module

cause

application::services::file_operations

workspace-root/cwd resolution (temp paths rejected)

application::workspace_session

workspace-root/cwd resolution

interface::mcp::file_ops_handlers

workspace-root/cwd via MCP file handlers

interface::mcp::mcp_roundtrip_tests

depends on file ops / workspace root

interface::mcp::security

workspace boundary validator depends on cwd

interface::mcp::handlers::refactor_handlers

cwd / tree-sitter fixture resolution

application::program_analysis::acceptance_evidence

cwd / fixture resolution

```

41 failures, 0 new since e55/e56.

## Strategy

1. `scripts/known_failures.yaml` — machine-readable baseline
   (`test`, `category`, `first_seen`, `reason`), environment-scoped.
2. `scripts/check_known_failures.py` — runs the target and compares the
   FAILED set: exit 1 on a NEW failure *or* on an unexpectedly-fixed one.
3. `just check-known-failures` / `just update-known-failures` recipes.

## Out of scope

- Fixing the 41 failures (they need a workspace-root fixture refactor).
- Making the baseline CI-portable across different roots.
