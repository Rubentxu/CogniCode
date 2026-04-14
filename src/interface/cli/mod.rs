//! CLI Interface - Command-line interface implementation

pub mod commands;
pub mod doctor;

pub use commands::{Cli, CommandExecutor};