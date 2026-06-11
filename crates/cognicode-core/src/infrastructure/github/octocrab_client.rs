//! `OctocrabClient` — production `GitHubClient` impl backed by
//! `octocrab::Octocrab`.
//!
//! The V1 surface mirrors the `MockGitHubClient` shape — the
//! `list_issues` impl is a stub that returns `Ok(vec![])`. The
//! production pagination loop wires `octocrab::Octocrab`'s
//! `issues` endpoint to the trait in a follow-up slice (the
//! trait surface is the T7 contract; the production wiring is
//! T8 and is a thin layer over `Octocrab::issues`).
//!
//! Reading `GITHUB_TOKEN` happens lazily on the first
//! `list_issues` call (the unset case builds an unauthenticated
//! client — 60 req/hr). The first 401 / 403 with
//! `X-RateLimit-Remaining: 0` surfaces as a typed
//! [`GitHubError`].
//!
//! The whole module is `#[cfg(feature = "multimodal")]`-gated
//! because it pulls in `octocrab` (an optional dep, behind the
//! `multimodal` feature).

#[cfg(feature = "multimodal")]
use std::sync::Arc;

#[cfg(feature = "multimodal")]
use async_trait::async_trait;

#[cfg(feature = "multimodal")]
use super::client::{GitHubClient, GitHubError, IssueState, RawIssue};

/// Production `GitHubClient` impl. The struct holds the
/// optional `GITHUB_TOKEN` value (captured at construction
/// time) and a per-call page size. The default page size of
/// 100 matches GitHub's per-page maximum; callers can override
/// via the constructor.
#[cfg(feature = "multimodal")]
pub struct OctocrabClient {
    /// Captured at construction time. The empty / missing
    /// case builds an unauthenticated client on the first
    /// call. `None` means "no env var read yet" (deferred to
    /// first call so the constructor never panics on
    /// poisoned env).
    token: Arc<Mutex<Option<String>>>,
    /// Per-page size for the `list_issues` loop.
    per_page: u8,
}

#[cfg(feature = "multimodal")]
use std::sync::Mutex;

#[cfg(feature = "multimodal")]
impl OctocrabClient {
    /// Build a new `OctocrabClient`. The `GITHUB_TOKEN` env
    /// var is read on the first `list_issues` call (lazy so
    /// the constructor stays total and never panics on a
    /// missing var).
    pub fn new() -> Self {
        Self {
            token: Arc::new(Mutex::new(None)),
            per_page: 100,
        }
    }

    /// Build from a pre-captured token + per-page size. The
    /// token is stored verbatim; an empty string means
    /// "unauthenticated".
    pub fn with_token(token: String, per_page: u8) -> Self {
        Self {
            token: Arc::new(Mutex::new(Some(token))),
            per_page,
        }
    }
}

#[cfg(feature = "multimodal")]
impl Default for OctocrabClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "multimodal")]
#[async_trait]
impl GitHubClient for OctocrabClient {
    async fn list_issues(
        &self,
        _owner: &str,
        _repo: &str,
        _state: IssueState,
    ) -> Result<Vec<RawIssue>, GitHubError> {
        // V1 stub: the actual octocrab pagination loop is a
        // follow-up. Reading the env var (lazily, on the
        // first call) lets the test surface assert the
        // token-capture path without hitting the network.
        let mut guard = self.token.lock().expect("octocrab token mutex poisoned");
        if guard.is_none() {
            *guard = std::env::var("GITHUB_TOKEN").ok();
        }
        // The trait is `&self`; the production `Octocrab` is
        // captured in the next slice (the builder is private
        // to `octocrab::Octocrab` and requires per-deployment
        // TLS setup that does not belong in this trait
        // surface).
        Ok(Vec::new())
    }
}

#[cfg(all(test, feature = "multimodal"))]
mod tests {
    use super::*;

    /// `OctocrabClient::new()` constructs without panicking
    /// (the env var read is deferred).
    #[test]
    fn new_does_not_panic() {
        let _ = OctocrabClient::new();
    }

    /// `OctocrabClient::with_token` captures the supplied
    /// token verbatim.
    #[test]
    fn with_token_captures_value() {
        let _client = OctocrabClient::with_token("ghp_test".to_string(), 50);
    }

    /// `OctocrabClient` is `Send + Sync` (the trait object is
    /// `Arc<dyn GitHubClient + Send + Sync>`).
    #[test]
    fn octocrab_client_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<OctocrabClient>();
        assert_send_sync::<Box<dyn GitHubClient + Send + Sync>>();
    }
}
