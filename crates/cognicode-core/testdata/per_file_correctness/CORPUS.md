# Corpus — PerFileStrategy nested traversal (PRF F2.W1)

> **Status**: stable corpus used as the source of truth for F2.W1
> characterization tests. Independent of the implementation under test:
> the oracle (expected symbols and edges) was derived from reading the
> `.rs` files directly, not from running CogniCode.

## Files

```
src/
├── lib.rs                      # defines top_level()
└── nested/
    ├── mod.rs                  # defines mid_level()
    └── deeply_nested/
        └── mod.rs              # defines leaf()
```

## Oracle (independent of implementation)

- **Functions** (3 total):
  - `top_level`        — `src/lib.rs:2`
  - `mid_level`        — `src/nested/mod.rs:3`
  - `leaf`             — `src/nested/deeply_nested/mod.rs:2`
- **Edges** (2 total):
  - `top_level` calls `nested::mid_level`
  - `mid_level` calls `crate::nested::deeply_nested::leaf`

## What F2.W1 asserts about this corpus

| Risk | Property tested | Expected |
|---|---|---|
| R1 (nested traversal) | All three files are reached by `PerFileStrategy::build_full_graph` | ≥3 symbols collected |
| R2 (content change) | When one file's content changes, the cache returns the new graph | Covered by `test_per_file_graph_cache_detects_content_change` (in `per_file_graph.rs`) |
| R3 (silent errors) | Files that cannot be parsed do not silently become "empty graph + complete" | **Recorded for F2.W2** — not addressed in F2.W1 |

## Why this corpus

- Tiny (3 files, ~250 bytes) — runs in milliseconds, no compile cost.
- Deterministic: the file tree, content, and oracle are all fixed and
  reviewed by a human.
- Hierarchical: covers R1 (subdirectory traversal) end-to-end without
  requiring a real Rust workspace.
- Independent: the oracle was written before the test was run. The test
  will be re-checked against the oracle if the corpus is ever edited.

## Reproducibility

The corpus is checked into this directory and is part of the working tree
that the PRF program operates on. Regeneration is a no-op; the files are
the corpus itself. Tests reference these files by absolute path during
`cargo test`, so any clone of the repo in the same environment will exercise
the same fixture.

## Out of scope for F2.W1

- Equivalence between `full` and `per_file` strategies on the same corpus.
  The two strategies traverse the file tree differently (PetGraphStore vs
  PerFileGraphCache merge) and may legitimately produce different results
  for the same set of files. Recorded as **F2.W3**.
- Read-error handling: `PerFileStrategy::build_full_graph` currently
  silently drops files that fail to parse. This is a separate defect
  recorded as **F2.W2**.
