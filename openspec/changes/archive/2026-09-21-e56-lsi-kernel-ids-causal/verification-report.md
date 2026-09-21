# Verification Report — cycle e56 — kernel ids + structured causal

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e56 — kernel ids + structured causal |
| Path | A-lite |
| Base HEAD | `8097fc7c` (post-e55) |
| Type | domain refactor (id ownership) + M6-local model change |
| Verify verdict | **PASS** |

## Verification

### V-1 — Findings tests

```
cargo test -p cognicode-core --lib domain::findings
```

Result: **47 passed; 0 failed.**

### V-2 — Kernel id tests now on the default build

```
cargo test -p cognicode-core --lib kernel_ids
```

Result: **12 passed; 0 failed** (previously these ran only under
`--features evidence-kernel`).

### V-3 — Gated kernel still green through the shim

```
cargo check -p cognicode-core --features evidence-kernel
cargo test  -p cognicode-core --features evidence-kernel --lib evidence_kernel
```

Result: build OK; **82 passed; 0 failed**. Proves the re-export shim kept
every `evidence_kernel::ids::*` path working.

### V-4 — Workspace + format + purity

- `cargo check --workspace` → 0 errors.
- `cargo fmt -p cognicode-core --check` → clean.
- No `sqlx`/`tokio`/I/O imports in `domain/findings/`.

### V-5 — Pre-existing failures confirmed unrelated

`cargo test -p cognicode-core --lib` reports 41 failures in file_operations /
mcp handlers / workspace_session / program_analysis acceptance. Stashing e56
and re-running `interface::mcp::security::tests::test_workspace_boundary_enforcement`
still failed → pre-existing, environment/cwd-dependent, out of scope.

## Follow-ups

- Unify the kernel `RelationKind` (`ns:name`) onto the canonical
  `ns.name` grammar (tracked divergence from e55).
