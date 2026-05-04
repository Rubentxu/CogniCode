//! Extracted rules from catalog.rs
//!
//! This module contains manually-implemented rules that were previously
//! defined directly in catalog.rs. The rules are organized by category:
//!
//! - **complexity.rs**: Rules related to code complexity (S138, S3776)
//! - **style.rs**: Rules related to code style (S2306, S1066, S1192)

pub mod complexity;
pub mod style;

pub use complexity::{S138Rule, S3776Rule};
pub use style::{S2306Rule, S1066Rule, S1192Rule};
