# F3.W1.a CLI↔MCP equivalence corpus

Minimal Rust crate with **intra-file dependencies** so that
`PerFileStrategy::build_local_graph(&file)` produces non-empty
dependency output. This complements `docs/prf/fixtures/per_file_correctness/`
(F2.W1 corpus), which only exercises cross-file resolution.

The corpus has:

- `callee() -> u32` (line 2): a function with no dependencies.
- `caller() -> u32` (lines 4-6): depends on `callee` within the same file.
- `nested::inner_caller()` (line 10): depends on `caller()` via super path
  (intra-module dependency, cross-file at module level).

For `build_local_graph("src/lib.rs")`:
- Symbols discovered: 3 (`callee`, `caller`, `nested::inner_caller`).
- Intra-file dependencies: at least 1 (`caller -> callee`).

The corpus is intentionally **deterministic**: no external crates, no
I/O, no environment-dependent behavior.
