// Fixture: file with intentionally broken syntax.
//
// The Rust parser (tree-sitter) will fail to extract symbols from this
// file. The PerFileStrategy must report this as a `Skipped { reason:
// Parse }` rather than dropping it silently and presenting the
// resulting graph as if the corpus were complete.

pub fn broken_fn( -> u32 {
    // missing closing parenthesis above; intentionally broken.
    42
}
