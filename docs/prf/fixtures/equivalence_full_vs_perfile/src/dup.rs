//! Equivalence oracle fixture (F2.W3, R4) — duplicate symbol names
//! within the same file.
//!
//! Scenario 3: same function name declared twice in the same file
//! (allowed in Rust when they live in different modules/namespaces,
//! here both are top-level so this is actually a compile error in a
//! real project, but the parser is expected to enumerate the two
//! declarations before that error matters). We wrap the second one
//! inside a `mod inner {}` to keep the file syntactically valid.

pub fn same_name() -> u32 {
    1
}

pub mod inner {
    pub fn same_name() -> u32 {
        2
    }
}
