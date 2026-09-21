# Corpus — PerFileStrategy partial coverage (PRF F2.W2)

> **Status**: stable corpus used as the source of truth for F2.W2
> characterization tests on read/parse error handling. The oracles
> below are derived from reading the file tree directly, not from
> running CogniCode.

## Files

```
src/
├── good.rs                       # parses cleanly; defines good_fn
├── unreadable.rs                 # parses cleanly but the file is chmod 000
│                                  # (skipped due to read permission)
├── broken_syntax.rs              # unparsable; defines broken_fn (BAD SYNTAX)
├── unsupported.txt               # supported extension (.txt is not in
│                                  # the supported set)
└── unreadable_subdir/
    └── inside.rs                 # inside a chmod 000 directory
```

## Oracles (independent of implementation)

The supported extensions are `rs`, `py`, `js`, `ts`
(see `PerFileStrategy::build_full_graph` filter).

### Expected `Complete`/`Partial`/`Failed` classification

| File | Walked | Read | Parsed | Result for this file |
|---|---|---|---|---|
| `src/good.rs` | ✓ | ✓ | ✓ | included in graph |
| `src/unreadable.rs` | ✓ | ✗ (perm denied) | — | `Skipped { reason: Read }` |
| `src/broken_syntax.rs` | ✓ | ✓ | ✗ (parse error) | `Skipped { reason: Parse }` |
| `src/unsupported.txt` | ✓ (walked) | ✓ | n/a (unsupported ext) | `Skipped { reason: UnsupportedExtension }` (or silently dropped, see test) |
| `src/unreadable_subdir/inside.rs` | depends on platform | depends | depends | platform-dependent |

### Oracle for `BuildReport`

After `PerFileStrategy::build_full_graph_report(src/)`:

- **status**: `Partial` (at least `unreadable.rs` and `broken_syntax.rs`
  must be reported as skipped).
- **skipped.len()**: ≥ 2 (the read failure + the parse failure).
- **graph.symbol_count()**: ≥ 1 (the `good_fn` symbol must be present).
- **The system does NOT declare "complete coverage"** when files were
  skipped.

### Out of scope for this corpus

- Files that change during the walk (TOCTOU). Covered separately by the
  test `test_per_file_strategy_file_disappears_during_walk`.
- Permission errors at the directory level (`unreadable_subdir/inside.rs`
  is platform-dependent; the test asserts on the upper bound only).
- Symbolic link loops; not exercised here.

## Why this corpus

- Mirrors the F2.W1 corpus structure so the same helper resolves it.
- Each failure mode (read / parse / unsupported) is isolated in its own
  file so a regression points to a specific cause.
- The supported-extension set is the same one used in production
  (`rs|py|js|ts`), so an `unsupported.txt` file exercises the "is the
  strategy even attempting to walk non-supported files?" question.

## Reproducibility

The corpus is checked into `docs/prf/fixtures/per_file_partial_corpus/`
and is part of the working tree that PRF operates on. Tests reference
these files by absolute path during `cargo test`.

To exercise permission failures on Unix, the test chmods the file to
`0o000` at runtime and restores it. On non-Unix platforms the
permission-related assertions degrade gracefully (the test panics with
a clear "skipped on this platform" message).
