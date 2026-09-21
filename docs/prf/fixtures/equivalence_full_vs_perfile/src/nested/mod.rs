//! Equivalence oracle fixture (F2.W3, R4) — nested module.
//!
//! Symbols in this file:
//!   - `shared`           : same name as `lib.rs::shared` (scenario 2).
//!   - `callee`           : called from `lib.rs::caller` (scenario 6).
//!   - `compute(s: &str)` : overload of `lib.rs::compute(x: u32)` (scenario 9).

pub fn shared() -> &'static str {
    "from nested"
}

pub fn callee() -> u32 {
    7
}

pub fn compute(s: &str) -> String {
    s.to_string()
}
