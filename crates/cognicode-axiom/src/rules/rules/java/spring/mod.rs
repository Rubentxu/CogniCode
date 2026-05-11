//! Spring Boot rules module
//!
//! This module contains Spring Boot specific rules.

pub mod sp1_rule;
pub mod sp2_rule;
pub mod sp3_rule;
pub mod sp4_rule;
pub mod sp5_rule;
pub mod sp6_rule;
pub mod sp7_rule;
pub mod sp8_rule;
pub mod sp9_rule;
pub mod sp10_rule;

pub use sp1_rule::JAVA_SP1Rule;
pub use sp2_rule::JAVA_SP2Rule;
pub use sp3_rule::JAVA_SP3Rule;
pub use sp4_rule::JAVA_SP4Rule;
pub use sp5_rule::JAVA_SP5Rule;
pub use sp6_rule::JAVA_SP6Rule;
pub use sp7_rule::JAVA_SP7Rule;
pub use sp8_rule::JAVA_SP8Rule;
pub use sp9_rule::JAVA_SP9Rule;
pub use sp10_rule::JAVA_SP10Rule;
