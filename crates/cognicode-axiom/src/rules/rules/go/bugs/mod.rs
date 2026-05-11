//! Go Bug Rules
//!
//! This module contains Go-specific bug detection rules.

pub mod b1_rule;       // panic() in library code
pub mod b2_rule;       // Empty error handling
pub mod b3_rule;       // Dead store
pub mod b4_rule;       // Unused variable (blank identifier)
pub mod b5_rule;       // Self-assignment
pub mod b6_rule;       // Identical operands
pub mod b7_rule;       // = vs == in condition
pub mod b8_rule;       // Float equality
pub mod b9_rule;       // Return value ignored
pub mod b10_rule;      // log.Fatal in library code
pub mod b11_rule;      // defer file.Close() missing
pub mod b12_rule;      // Error returned but not checked

pub use b1_rule::GO_S1148Rule;
pub use b2_rule::GO_S108Rule;
pub use b3_rule::GO_S185Rule;
pub use b4_rule::GO_S1481Rule;
pub use b5_rule::GO_S1656Rule;
pub use b6_rule::GO_S1764Rule;
pub use b7_rule::GO_S2757Rule;
pub use b8_rule::GO_S1244Rule;
pub use b9_rule::GO_S2201Rule;
pub use b10_rule::GO_S2221Rule;
pub use b11_rule::GO_S2095Rule;
pub use b12_rule::GO_S1160Rule;
