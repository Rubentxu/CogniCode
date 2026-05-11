//! Python-specific rules
//!
//! This module contains rules specifically for Python code, organized by category:
//! - **security/**: Security vulnerability rules
//! - **bugs/**: Bug detection rules
//! - **code_smells/**: Code smell rules
//! - **error_handling/**: Error handling rules
//! - **performance/**: Performance rules

pub mod bugs;
pub mod code_smells;
pub mod error_handling;
pub mod naming;
pub mod performance;
pub mod security;
pub mod testing;
