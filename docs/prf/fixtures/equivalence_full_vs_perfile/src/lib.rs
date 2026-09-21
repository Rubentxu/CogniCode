//! Equivalence oracle fixture (F2.W3, R4).
//!
//! Symbols in this file:
//!   - `hello`           : unique simple symbol (scenario 1).
//!   - `shared`          : also defined in `nested/mod.rs` (scenario 2).
//!   - `caller`          : calls `callee` (scenario 6, cross-file edge).
//!   - `compute(x: u32)` : also defined in `nested/mod.rs::compute(s)` (scenario 9).

pub fn hello() -> u32 {
    42
}

pub fn shared() -> &'static str {
    "from lib"
}

pub fn caller() -> u32 {
    crate::nested::callee()
}

pub fn compute(x: u32) -> u32 {
    x * 2
}
