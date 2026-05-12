//! Extracted rules from catalog.rs
//!
//! This module contains manually-implemented rules that were previously
//! defined directly in catalog.rs. The rules are organized by category:
//!
//! - **complexity.rs**: Rules related to code complexity (S138, S3776)
//! - **style.rs**: Rules related to code style (S2306, S1066, S1192)

pub mod complexity;
pub mod style;
pub mod rust;

pub use complexity::{S138Rule, S3776Rule};
pub use style::{S2306Rule, S1066Rule, S1192Rule};
pub use rust::{S1142Rule, S1214Rule, S1541Rule, S1244Rule, S2259Rule, S1197Rule, S1161Rule, S115Rule, S1151Rule, S1163Rule, S134Rule, S107Rule, S1135Rule, S2068Rule, S2589Rule, S4792Rule, S5122Rule};
