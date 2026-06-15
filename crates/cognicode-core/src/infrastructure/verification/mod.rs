//! Verification infrastructure — concrete implementations of verification traits
//!
//! This module provides the concrete implementation for code verification,
//! including the RustVerifier which uses rustc for compilation checks.

mod rust_verifier;

pub use rust_verifier::RustVerifier;
