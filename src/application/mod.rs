//! Application Layer - Use cases and application services
//!
//! This module contains the application services that orchestrate
//! domain logic and provide use case implementations.

pub mod commands;
pub mod dto;
pub mod error;
pub mod services;

// Re-export error types for convenience
pub use error::{AppError, AppResult};
