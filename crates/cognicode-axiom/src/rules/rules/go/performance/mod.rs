//! Go Performance Rules
//!
//! This module contains Go-specific performance rules.

pub mod p1_rule;       // String concat in loop
pub mod p2_rule;       // for i := 0; i < len(x); i++ pattern
pub mod p3_rule;       // append in loop without pre-allocation
pub mod p4_rule;       // fmt.Sprintf("%s", x)
pub mod p5_rule;       // Mutex lock ordering (deadlock)
pub mod p6_rule;       // Nil pointer dereference

pub use p1_rule::GO_S1700Rule;
pub use p2_rule::GO_S1736Rule;
pub use p3_rule::GO_S1943Rule;
pub use p4_rule::GO_S2111Rule;
pub use p5_rule::GO_S1860Rule;
pub use p6_rule::GO_S2259Rule;
