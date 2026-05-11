//! Go Naming Rules
//!
//! This module contains Go-specific naming convention rules.

pub mod n1_rule;       // Function naming (camelCase)
pub mod n2_rule;       // Type naming (PascalCase)
pub mod n3_rule;       // Constant naming (UPPER_CASE)
pub mod n4_rule;       // Variable naming (camelCase)
pub mod n5_rule;       // Unused import
pub mod n6_rule;       // Missing doc comment on exported function
pub mod n7_rule;       // Too many function parameters (>6)
pub mod n8_rule;       // Deep nesting (>3 levels)

pub use n1_rule::GO_S100Rule;
pub use n2_rule::GO_S101Rule;
pub use n3_rule::GO_S115Rule;
pub use n4_rule::GO_S117Rule;
pub use n5_rule::GO_S170Rule;
pub use n6_rule::GO_S173Rule;
pub use n7_rule::GO_S107Rule;
pub use n8_rule::GO_S134Rule;
