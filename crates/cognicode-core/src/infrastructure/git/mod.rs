//! Git Infrastructure Module
//!
//! Provides git history integration for temporal indexing.
//! Used to retrieve file modification times from git commit history.

pub mod git_history;

pub use git_history::{file_mtime, get_file_mtime, git_log_mtime};
