//! Git Infrastructure Module
//!
//! Provides git history integration for temporal indexing.
//! Used to retrieve file modification times from git commit history.

pub mod git_history;

pub use git_history::{git_log_mtime, file_mtime, get_file_mtime};
