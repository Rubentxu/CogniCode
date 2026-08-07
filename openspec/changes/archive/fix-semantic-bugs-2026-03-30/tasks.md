# Tasks: fix-semantic-bugs-2026-03-30

## Completed

- [x] 1. Fix JavaScript class children extraction in outline.rs
  - Added `property_identifier` to `find_name_in_node` to handle JavaScript method names stored in `property_identifier` nodes
  
- [x] 2. Fix Python docstring extraction in symbol_code.rs
  - Fixed `extract_docstring` to properly check the line at `idx` position (docstring position) before checking `idx-1`
  - Handles both single-line (`"""..."""`) and multi-line Python docstrings
  
- [x] 3. Fix Rust docstring extraction in symbol_code.rs
  - Fixed `extract_docstring` to properly collect consecutive `///` comment lines
  - Changed blank-skipping logic to check `lines[idx]` instead of `lines[idx-1]`
  
- [x] 4. Fix single line comment extraction in symbol_code.rs
  - Fixed `extract_docstring` to properly find `//` or `#` comments directly above a symbol
  - Added proper handling for comment directly above with no blank lines in between

- [x] 5. Run tests to verify fixes: `cargo test --lib semantic`
  - All 22 semantic tests pass

## Summary

Fixed 4 bugs in semantic analysis implementation:

1. **JavaScript outline bug**: `find_name_in_node` was only checking for `identifier` nodes, but JavaScript method names are stored in `property_identifier` nodes. Added `property_identifier` as an alternative identifier type.

2. **Python docstring bug**: The `extract_docstring` function was checking `lines[idx-1]` (line above the docstring position) instead of `lines[idx]` (the docstring position itself). Fixed to check `idx` first, then fall back to `idx-1`.

3. **Rust docstring bug**: Same root cause as Python - incorrect indexing when looking for `///` comment lines. Fixed the blank-skipping logic and comment collection.

4. **Single line comment bug**: Related to the same indexing issue - comments directly above symbols weren't being found because of incorrect index calculations.
