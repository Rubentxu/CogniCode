# F3.W3 get_outline CLI↔MCP equivalence corpus

Minimal Rust crate with a **hierarchical structure** to exercise the
outline builder: top-level functions, a nested module with its own
symbols, and a private function (must be excluded when
`include_private=false`).

The corpus has:

- `outline_alpha()` (line 4): top-level public function.
- `outline_beta()` (line 8): top-level public function.
- `_outline_private()` (line 12): private (underscore-prefixed); must
  be excluded when `include_private=false`.
- `nested::outline_gamma()` (line 18): nested module symbol.

For `build_outline(source, "lib.rs", Rust, include_private=false, include_tests=true)`:
- Top-level symbols: `outline_alpha`, `outline_beta` (private excluded).
- Nested symbols (under `outline_alpha` or `outline_beta` as children):
  none in this corpus; the test only compares top-level names to keep
  the comparison surface narrow.

The corpus uses **unique-prefix names** (`outline_*`) so it does not
collide with other test corpora in the repo.

Default flags for both CLI (`IndexCommand::Outline`) and MCP
(`OutlineInput`) MUST match for equivalence: `include_private=false,
include_tests=true`. The CLI uses `false, true` by default; the MCP
uses `true, true` by default. **The test pins both to `(false, true)`
explicitly** — equivalence must be verified at the same flag
configuration, not by accident of differing defaults.
