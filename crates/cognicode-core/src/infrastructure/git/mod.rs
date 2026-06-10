//! Git Infrastructure Module
//!
//! Provides git history integration for temporal indexing.
//! Used to retrieve file modification times from git commit history.

pub mod git_history;
#[cfg(feature = "multimodal")]
pub mod commit_issue_parser;

pub use git_history::{file_mtime, get_file_mtime, git_log_mtime};
