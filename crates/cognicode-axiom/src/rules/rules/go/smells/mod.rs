//! Go Code Smell Rules
//!
//! This module contains Go-specific code smell rules.

pub mod sm1_rule;      // Long function (>60 lines)
pub mod sm2_rule;      // High complexity (>15)
pub mod sm3_rule;      // Switch without default
pub mod sm4_rule;      // TODO/FIXME comments
pub mod sm5_rule;      // Commented-out code
pub mod sm6_rule;      // Empty function
pub mod sm7_rule;      // Duplicate branches
pub mod sm8_rule;      // File too long (>500 lines)
pub mod sm9_rule;      // Variable assigned but never used
pub mod sm10_rule;     // Low comment ratio

pub use sm1_rule::GO_S138Rule;
pub use sm2_rule::GO_S3776Rule;
pub use sm3_rule::GO_S131Rule;
pub use sm4_rule::GO_S1135Rule;
pub use sm5_rule::GO_S125Rule;
pub use sm6_rule::GO_S1186Rule;
pub use sm7_rule::GO_S1871Rule;
pub use sm8_rule::GO_S122Rule;
pub use sm9_rule::GO_S1845Rule;
pub use sm10_rule::GO_S148Rule;
