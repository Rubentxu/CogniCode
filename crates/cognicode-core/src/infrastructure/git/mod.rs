//! Git Infrastructure Module
//!
//! Provides git history integration for temporal indexing.
//! Used to retrieve file modification times from git commit history.

#[cfg(feature = "multimodal")]
pub mod commit_issue_parser;
pub mod git_history;
// E38 design D5: kernel RenameEvidencePort adapter, kernel-feature-gated
// like the domain port it resolves (evidence-kernel precedent).
#[cfg(feature = "evidence-kernel")]
pub mod rename_evidence;

pub use git_history::{file_mtime, get_file_mtime, git_log_mtime};
#[cfg(feature = "evidence-kernel")]
pub use rename_evidence::GitRenameEvidenceAdapter;
