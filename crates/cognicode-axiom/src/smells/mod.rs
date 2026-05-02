//! Code smell catalog — separated from rule engine
//!
//! Re-exports all code smell rules from the catalog for organizational clarity.
//! The rules themselves remain in rules/catalog.rs for inventory auto-discovery.

pub use crate::rules::catalog::*;
