//! Go-specific rules
//!
//! This module contains rules specifically for Go code, organized by category:
//! - **naming/**: Naming convention rules
//! - **bugs/**: Bug detection rules
//! - **security/**: Security vulnerability rules
//! - **smells/**: Code smell rules
//! - **performance/**: Performance rules

pub mod bugs;
pub mod naming;
pub mod performance;
pub mod security;
pub mod smells;
