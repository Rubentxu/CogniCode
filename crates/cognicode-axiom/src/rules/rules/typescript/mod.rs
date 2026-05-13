//! TypeScript advanced rules module
//!
//! This module contains TypeScript-specific linting rules.

pub mod adv1_rule;
pub mod adv2_rule;
pub mod adv3_rule;
pub mod adv4_rule;
pub mod adv5_rule;
pub mod adv6_rule;
pub mod adv7_rule;
pub mod adv8_rule;

pub use adv1_rule::TS_ADV1Rule;
pub use adv2_rule::TS_ADV2Rule;
pub use adv3_rule::TS_ADV3Rule;
pub use adv4_rule::TS_ADV4Rule;
pub use adv5_rule::TS_ADV5Rule;
pub use adv6_rule::TS_ADV6Rule;
pub use adv7_rule::TS_ADV7Rule;
pub use adv8_rule::TS_ADV8Rule;
