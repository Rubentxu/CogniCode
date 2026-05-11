//! Python Bug Rules
//!
//! This module contains Python-specific bug rules for detecting code defects.

pub mod s1226_rule;      // Parameter reassigned
pub mod s1244_rule;      // Float equality
pub mod s1481_rule;      // Unused variable
pub mod s1656_rule;      // Self-assignment
pub mod s1751_rule;      // Loop with single iteration
pub mod s1764_rule;      // Identical operands
pub mod s1845_rule;      // Dead store
pub mod s1854_rule;      // Unused import
pub mod s1860_rule;      // Deadlock potential
pub mod s1994_rule;      // Loop counter modified inside
pub mod s2178_rule;      // is instead of == with literals
pub mod s2201_rule;      // Return value ignored
pub mod s2259_rule;      // None dereference
pub mod s2589_rule;      // Always-true condition
pub mod s2757_rule;      // Assignment vs comparison in condition

pub use s1226_rule::PY_S1226Rule;
pub use s1244_rule::PY_S1244Rule;
pub use s1481_rule::PY_S1481Rule;
pub use s1656_rule::PY_S1656Rule;
pub use s1751_rule::PY_S1751Rule;
pub use s1764_rule::PY_S1764Rule;
pub use s1845_rule::PY_S1845Rule;
pub use s1854_rule::PY_S1854Rule;
pub use s1860_rule::PY_S1860Rule;
pub use s1994_rule::PY_S1994Rule;
pub use s2178_rule::PY_S2178Rule;
pub use s2201_rule::PY_S2201Rule;
pub use s2259_rule::PY_S2259Rule;
pub use s2589_rule::PY_S2589Rule;
pub use s2757_rule::PY_S2757Rule;
