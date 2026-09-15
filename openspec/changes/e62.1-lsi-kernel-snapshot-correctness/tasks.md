# Tasks — cycle e62.1 — kernel snapshot correctness

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

| WU | Content |
|----|---------|
| WU-0 | Characterization test: cross-snapshot evidence leak (written first) |
| WU-1a | Pin `EvidenceStore::add`/`get` and `FactStore::get` to `(ws, snapshot)` |
| WU-1b | Re-key `InMemoryEvidenceStore` by snapshot on both axes |
| WU-1c | `KernelError::EvidenceIdCollision` (atomic rejection, no merge) |
| WU-1d | Correct the misleading pinning doc comment |

## Acceptance gate
- `cargo test -p cognicode-core --features evidence-kernel --lib in_memory`
  → 29 passed (incl. the characterization, id-collision and both `get` pins).
- Gated core: same 41 known failures as the baseline (no regressions).
- `cargo check --workspace --all-targets` 0 errors; fmt clean.
