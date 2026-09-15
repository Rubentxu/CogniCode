# Verification Report — cycle e57 — Detector Executor + AST backend

> Cycle: A-lite | Milestone: M6 | Phase: verify | Date: 2026-09-15

## Summary

| Item | Value |
|------|-------|
| Cycle | e57 — Detector Executor + AST backend |
| Path | A-lite |
| Base HEAD | `1e7aa4ab` (post-e56) |
| Type | new execution seam + first backend (additive, no consumers yet) |
| Verify verdict | **PASS** |

## Verification

### V-1 — Domain tests

```
cargo test -p cognicode-core --lib domain::findings
```

Result: **79 passed; 0 failed** (admission 7, assembler 5, ast_backend 4,
detector_ir 19, digest 5, execution 2, finding 14, namespaced 5, outcome 2,
ports 2, quality_projection 10, verifier 4). Plus 1 infrastructure test.

### V-2 — End-to-end vertical slice

```
cargo test -p cognicode-core --test findings_ast_e2e
```

Result: **5 passed; 0 failed**:

| Test | Property |
|------|----------|
| `u40_candidate_run_produces_a_finding_but_cannot_block` | IR executes; AI-claimed `Gated` forced to Candidate; finding produced; **cannot block** |
| `promoted_detector_blocks_and_keeps_semantic_identity` | promote → blocks; same `semantic_digest`, different `instance_digest` |
| `unresolved_evidence_blocks_even_a_gated_finding` | referential truth gates even a Gated finding |
| `u47_planning_fails_loud_when_no_backend_covers_capabilities` | missing capability fails before running |
| `registry_and_capability_sets_are_stable` | planner basics |

### V-3 — Build / purity / format / kernel

- `cargo check --workspace` → 0 errors.
- `cargo fmt -p cognicode-core --check` → clean.
- Domain purity: no `sqlx`/`tokio`/I/O imports in `domain/findings/`.
- Gated kernel unaffected: `--features evidence-kernel` + 82 `evidence_kernel`
  tests still green.

### V-4 — No stale digest usage

`grep` for `detector_digest` / `fnv1a64` in `domain/findings/` → only a
digest test asserting FNV-1a is now rejected.

## Known limitations (documented, not defects)

- AST backend consumes an abstract `AstInput`; a real tree-sitter extractor is
  a follow-up. The backend contract will not change.
- Severity/risk are backend defaults because the IR does not carry them yet.

## Conclusion

The first safe, executable traversal of the Detector IR is in place, with the
mandatory execution seam established. U40 and a real U43 (Candidate cannot
block end-to-end) pass. PASS.
