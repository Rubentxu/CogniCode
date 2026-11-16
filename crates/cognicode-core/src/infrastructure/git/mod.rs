//! Git Infrastructure Module
//!
//! Provides git history integration for temporal indexing.
//! Used to retrieve file modification times from git commit history.

#[cfg(feature = "multimodal")]
pub mod commit_issue_parser;
pub mod git_history;
// CAA-03/CGH-021: Git adapter over the neutral SCM ports (domain::scm
// is ungated, so this adapter is ungated too).
#[cfg(feature = "evidence-kernel")]
pub mod rename_evidence;
pub mod scm_adapter;

pub use git_history::{file_mtime, get_file_mtime, git_log_mtime};
#[cfg(feature = "evidence-kernel")]
pub use rename_evidence::GitRenameEvidenceAdapter;
pub use scm_adapter::GitScmAdapter;
