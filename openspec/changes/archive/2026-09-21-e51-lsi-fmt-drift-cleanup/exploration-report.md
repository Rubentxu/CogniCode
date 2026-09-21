# Exploration Report — cycle e51 — rustfmt drift cleanup

> Cycle: B-direct (housekeeping) | Phase: explore | Date: 2026-09-15

## Trigger

`cargo fmt --all --check` (the repo's Format gate) was RED on `main`
with formatting drift in 4 files, accumulated across earlier cycles.

## Evidence

```
$ cargo fmt --all --check
Diff in crates/cognicode-cli/tests/cogh_cli.rs
Diff in crates/cognicode-core/src/application/fact_bridge/lsp_facts.rs
Diff in crates/cognicode-core/src/application/program_analysis/conformance.rs
Diff in crates/cognicode-core/tests/equivalence_harness/generic_graph.rs
```

The drift is purely cosmetic rustfmt output (line-joining of short
method chains, an import wrapped onto one line, a missing trailing
newline). No semantic content.

## Strategy

Apply `cargo fmt --all` and commit the result. Verify:
- `cargo fmt --all --check` is GREEN.
- `cargo check -p cognicode-core --tests` still compiles.

## Impact model

| Artifact | Impact | Confidence |
|----------|--------|-----------|
| 4 files (test/source) | whitespace only | KNOWN |
| behaviour | none | KNOWN |

## Out of scope

- Pre-existing clippy warnings (tracked separately).
