# Verification Report — cycle e66 — M7.5 Read-Set Foundation (U61)

> Cycle: A-lite | Milestone: M7 — Behavior Architecture | Phase: verify | Date: 2026-09-17

## Summary

| Item | Value |
|------|-------|
| Cycle | e66 — Read-Set Foundation |
| Path | A-lite |
| Base HEAD | `7df55d49` (post-e65 WU5) |
| Commits | `464c47ce`, `8cb98ed6`, `d2692b85`, `03b83113` (post-amend of `d4124e2d`) |
| Verify verdict | **PASS** |
| U61 | **CLOSED** (foundation) |

## Verification

### V-1 — Suite counts

| Suite | Result |
|-------|--------|
| `cargo test -p cognicode-core --lib readset` | 12 passed |
| `cargo test -p cognicode-core --test read_set_e2e` | 10 passed |
| `cargo test -p cognicode-core --test read_set_e2e --features evidence-kernel` | 5 passed (property-based) |
| `cargo test -p cognicode-core --lib readset --features evidence-kernel` | 12 passed |

### V-2 — The UAT-U61 acceptance

`domain::readset::tests::uat_u61_unrelated_change_does_not_invalidate`:

> E read A, B; C changes → not stale.

```text
is_stale(execution_read_set = {A, B}, changed_facts = [C]) == false
is_stale(execution_read_set = {A, B}, changed_facts = [A]) == true
is_stale(execution_read_set = {A, B}, changed_facts = [B]) == true
is_stale(execution_read_set = {A, B}, changed_facts = [A, C]) == true
```

The property-based invariants generalize this across 32–64 trials per property.

### V-3 — Lints & format

| Check | Result |
|-------|--------|
| `cargo fmt --check -p cognicode-core` | clean |
| `cargo clippy -p cognicode-core --all-targets -- -D warnings` | clean on touched paths |
| `cargo check --workspace --all-targets` | same shape as base (no regression) |

### V-4 — Edge-case coverage

The edge-case tests (`8cb98ed6`) exercise:

- `RecorderClosed` after `finalize` (record rejected with `Err(RecorderClosed)`).
- Bound = `usize::MIN` (truncation marker set on first insert — degenerate but well-defined).
- `FactId(u64::MAX)` does not collide with the truncation sentinel.
- `ReadSet: Send + Sync` (compile-time, no internal mutability that would break the contract).
- `Drop` without `finalize` is safe (no panic, no leaked resources; the truncation marker stays
  false because no record was rejected).
- `Display` for `ReadSetError` produces stable strings (no leaking of internal types).

## Regression (all green)

| Suite | Result |
|-------|--------|
| e64 regression (behavior authority E2E) | 6 / 6 passed |
| e65 regression (behavior budget E2E) | 6 / 6 passed |
| e62.4 regression (canonical grounding E2E) | 10 / 10 passed |
| `findings_ast_e2e` | 8 / 8 passed |
| `findings_graph_e2e` | 4 / 4 passed |
| `findings_dataflow_e2e` | 7 / 7 passed |
| `cargo check --workspace --all-targets` | 0 errors |

## Out of scope

See `archive-manifest.md` § "Out of scope".
