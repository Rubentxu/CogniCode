//! CogniCode rig-core Tools
//!
//! Provides Tool trait implementations for rig-core agent framework.
//! Each tool wraps a WorkspaceSession operation.

#[cfg(feature = "rig")]
mod tools;

#[cfg(feature = "rig")]
pub use tools::*;
