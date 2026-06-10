//! `MockGitHubClient` — in-memory `GitHubClient` for unit tests.
//!
//! The mock is constructed via
//! [`MockGitHubClient::with_issues`] (returns the canned vec on
//! every `list_issues` call) or [`MockGitHubClient::with_error`]
//! (returns the canned error on every call). The whole struct
//! is `#[cfg(test)]`-only — production builds never see it.

#[cfg(all(test, feature = "multimodal"))]
use std::sync::Mutex;

#[cfg(all(test, feature = "multimodal"))]
use async_trait::async_trait;

#[cfg(all(test, feature = "multimodal"))]
use super::client::{GitHubClient, GitHubError, IssueState, RawIssue};

/// Test impl of [`GitHubClient`]. The state is wrapped in a
/// `Mutex` so the trait methods can take `&self` while still
/// mutating on every call (the trait is `&self`).
#[cfg(all(test, feature = "multimodal"))]
pub struct MockGitHubClient {
    state: Mutex<MockState>,
}

#[cfg(all(test, feature = "multimodal"))]
enum MockState {
    /// Return the canned `Vec<RawIssue>` on every call.
    Issues(Vec<RawIssue>),
    /// Return the canned `GitHubError` on every call.
    Error(GitHubError),
}

#[cfg(all(test, feature = "multimodal"))]
impl MockGitHubClient {
    /// Build a mock that returns `issues` on every `list_issues`
    /// call. Useful for the happy-path tests.
    pub fn with_issues(issues: Vec<RawIssue>) -> Self {
        Self {
            state: Mutex::new(MockState::Issues(issues)),
        }
    }

    /// Build a mock that returns `err` on every call. Useful
    /// for the auth-required / rate-limited tests.
    pub fn with_error(err: GitHubError) -> Self {
        Self {
            state: Mutex::new(MockState::Error(err)),
        }
    }
}

#[cfg(all(test, feature = "multimodal"))]
#[async_trait]
impl GitHubClient for MockGitHubClient {
    async fn list_issues(
        &self,
        _owner: &str,
        _repo: &str,
        _state: IssueState,
    ) -> Result<Vec<RawIssue>, GitHubError> {
        let state = self.state.lock().expect("mock mutex poisoned");
        match &*state {
            MockState::Issues(v) => Ok(v.clone()),
            MockState::Error(e) => Err(match e {
                GitHubError::AuthRequired => GitHubError::AuthRequired,
                GitHubError::RateLimited => GitHubError::RateLimited,
                GitHubError::ApiError(s) => GitHubError::ApiError(s.clone()),
            }),
        }
    }
}
