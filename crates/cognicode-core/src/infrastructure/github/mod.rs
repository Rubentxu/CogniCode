//! `github` — GitHub REST API adapter (production + test impls).
//!
//! Provides:
//! - [`client`] — the `GitHubClient` trait + `RawIssue` DTO.
//! - [`octocrab_client`] — the production `OctocrabClient`
//!   (gated behind `multimodal`).
//! - [`mock_client`] — the in-memory `MockGitHubClient` for
//!   unit tests (`#[cfg(test)]`).
//!
//! Every submodule is `#[cfg(feature = "multimodal")]`-gated
//! because the whole stack is part of the `issue-tracker-adapter`
//! change. The default build is byte-for-byte unchanged.

#[cfg(feature = "multimodal")]
pub mod client;
#[cfg(all(test, feature = "multimodal"))]
pub mod mock_client;
#[cfg(feature = "multimodal")]
pub mod octocrab_client;

#[cfg(feature = "multimodal")]
pub use client::{GitHubClient, GitHubError, IssueState, RawIssue};
#[cfg(feature = "multimodal")]
pub use octocrab_client::OctocrabClient;
