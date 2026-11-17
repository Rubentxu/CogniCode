//! `github` — GitHub REST API adapter (production + test impls).
//!
//! Provides:
//! - [`client`] — the `GitHubClient` trait + `RawIssue` DTO.
//! - [`octocrab_client`] — the production `OctocrabClient`
//!   (gated behind `multimodal`).
//! - `mock_client` — the in-memory `MockGitHubClient` for
//!   unit tests (`#[cfg(test)]`).
//!
//! The issue-tracker submodules are `#[cfg(feature = "multimodal")]`-gated
//! because that stack is part of the `issue-tracker-adapter` change.
//!
//! The review adapter ([`review_adapter`], over [`gh_runner`]) serves the
//! forge ports of the pull-request review and is not gated: its domain
//! ([`crate::domain::forge`]) is not either.

#[cfg(feature = "multimodal")]
pub mod client;
pub mod gh_runner;
#[cfg(all(test, feature = "multimodal"))]
pub mod mock_client;
#[cfg(feature = "multimodal")]
pub mod octocrab_client;
pub mod review_adapter;

#[cfg(feature = "multimodal")]
pub use client::{GitHubClient, GitHubError, IssueState, RawIssue};
#[cfg(feature = "multimodal")]
pub use octocrab_client::OctocrabClient;

pub use gh_runner::{CommandCall, CommandOutput, CommandRunner, GhCliRunner};
pub use review_adapter::GitHubForge;
