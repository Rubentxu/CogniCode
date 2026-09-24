# F3.W2 query_symbol_index CLI↔MCP equivalence corpus

Minimal Rust crate with **unique symbols** to query. The CLI path
(`cognicode index query <symbol>`) and the MCP path
(`query_symbol_index(symbol_name, directory)`) must both locate these
symbols with the same `(file, line, column)` tuples.

The corpus has:

- `unique_alpha()` (line 2): a function with a name unlikely to match
  any other symbol in the test corpus.
- `unique_beta()` (line 6): another unique symbol.
- `nested::unique_gamma()` (line 11): a third unique symbol nested in a
  submodule (the per-file path must discover nested symbols via `walkdir`).

For `query_symbols("unique_alpha")` over `src/`:
- Expected matches: `[{src/lib.rs, 2, 0}]` (1 match).

For `query_symbols("unique_beta")`:
- Expected matches: `[{src/lib.rs, 6, 0}]` (1 match).

For `query_symbols("unique_gamma")`:
- Expected matches: `[{src/lib.rs, 11, 4}]` (1 match — nested module path).

The corpus is intentionally **deterministic**: no external crates, no
I/O, no environment-dependent behavior. Symbol names use unique
prefixes (`unique_*`) so that other test corpora in the same repo do
not collide.
