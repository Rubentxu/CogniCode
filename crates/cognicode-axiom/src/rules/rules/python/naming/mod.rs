//! Python Naming Rules
//!
//! This module contains Python-specific naming convention rules for detecting
//! style violations related to naming conventions.

pub mod n1_rule;       // Function naming (snake_case)
pub mod n2_rule;       // Class naming (PascalCase)
pub mod n3_rule;       // Constant naming (UPPER_CASE)
pub mod n4_rule;       // Variable naming (snake_case)
pub mod n5_rule;       // Commented-out code
pub mod n6_rule;       // Too many methods in class (>20)
pub mod n7_rule;       // Too many fields in class (>15)
pub mod n8_rule;       // High cyclomatic complexity (>15)
pub mod n9_rule;       // Mutable default argument
pub mod n10_rule;      // print() in library code
pub mod n11_rule;      // Function too complex (>50 lines)
pub mod n12_rule;      // String concatenation in loop
pub mod n13_rule;      // Unused import
pub mod n14_rule;      // Missing type hints on public functions
pub mod n15_rule;      // f-string without interpolation

pub use n1_rule::PY_N1Rule;
pub use n2_rule::PY_N2Rule;
pub use n3_rule::PY_N3Rule;
pub use n4_rule::PY_N4Rule;
pub use n5_rule::PY_N5Rule;
pub use n6_rule::PY_N6Rule;
pub use n7_rule::PY_N7Rule;
pub use n8_rule::PY_N8Rule;
pub use n9_rule::PY_N9Rule;
pub use n10_rule::PY_N10Rule;
pub use n11_rule::PY_N11Rule;
pub use n12_rule::PY_N12Rule;
pub use n13_rule::PY_N13Rule;
pub use n14_rule::PY_N14Rule;
pub use n15_rule::PY_N15Rule;