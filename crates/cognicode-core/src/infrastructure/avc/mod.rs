//! Agent-Verifiable Context (AVC) Module
//!
//! Structured truth contracts for AI agents.
//!
//! Usage:
//! ```rust,ignore
//! use cognicode_core::infrastructure::avc::{AvcContract, AvcGenerator, AvcValidator};
//!
//! // Generate a contract from existing code
//! let contract = AvcGenerator::generate_from_source(
//!     source_code, "function_name", "file.rs"
//! ).unwrap();
//!
//! // Validate agent-generated code against the contract
//! let result = AvcValidator::validate(&contract, agent_code);
//! if !result.passed {
//!     for violation in &result.violations {
//!         eprintln!("{}: {}", violation.severity, violation.message);
//!     }
//! }
//! ```

pub mod contract;
pub mod generator;
pub mod validator;

pub use contract::*;
pub use generator::AvcGenerator;
pub use validator::AvcValidator;
