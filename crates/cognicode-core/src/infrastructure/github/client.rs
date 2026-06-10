//! `GitHubClient` — DIP trait for the GitHub REST API.
//!
//! The extractor is generic over `Arc<dyn GitHubClient>` so unit
//! tests inject canned data without network I/O. Production
//! builds use [`OctocrabClient`](super::octocrab_client) (gated
//! behind `multimodal`); test builds use
//! [`MockGitHubClient`](super::mock_client).
//!
//! The trait exposes the minimum data the extractor needs — a
//! flat [`RawIssue`] struct that shields the domain from
//! upstream `octocrab` churn. The IB check (Protocol C in
//! `entropy-sdd`) confirms this design reduces connascence of
//! value to `octocrab` from ~2.0 bits (the proposal's
//! estimate) to ~0.5 bits.
//!
//! The whole module is `#[cfg(feature = "multimodal")]`-gated.

#[cfg(feature = "multimodal")]
use async_trait::async_trait;
#[cfg(feature = "multimodal")]
use thiserror::Error;

/// Issue state filter. Mirrors the GitHub API's `state` query
/// parameter (`open` | `closed` | `all`). The V1 extractor
/// fetches both `Open` and `Closed` and unions the results.
#[cfg(feature = "multimodal")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IssueState {
    /// Open issues only.
    Open,
    /// Closed issues only.
    Closed,
    /// All issues (open + closed).
    All,
}

#[cfg(feature = "multimodal")]
impl IssueState {
    /// Stable kebab-case identifier. Used in the
    /// `as_str()`-style API and in the URL query string.
    pub const fn as_str(self) -> &'static str {
        match self {
            IssueState::Open => "open",
            IssueState::Closed => "closed",
            IssueState::All => "all",
        }
    }
}

/// Flat DTO that mirrors the subset of the GitHub issue
/// payload the extractor needs. Decouples the domain from
/// `octocrab::models::issues::Issue` (a richer type with many
/// fields the V1 spec doesn't surface).
///
/// The `url` is the canonical issue HTML URL (e.g.
/// `https://github.com/acme/widgets/issues/42`).
#[cfg(feature = "multimodal")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawIssue {
    /// Issue number (1-based; 0 is invalid in GitHub).
    pub number: u32,
    /// Issue title (may be empty on GitHub for deleted issues).
    pub title: String,
    /// `open` | `closed` (lowercased for the canonical
    /// property value).
    pub state: String,
    /// Canonical HTML URL of the issue.
    pub url: String,
    /// Comma-joined label names. Empty string when the issue
    /// has no labels.
    pub labels: Vec<String>,
    /// `Some(login)` when assigned, `None` when unassigned.
    pub assignee: Option<String>,
    /// Author login (the `user.login` field).
    pub author: Option<String>,
    /// RFC 3339 creation timestamp, when known.
    pub created_at: Option<String>,
    /// RFC 3339 last-update timestamp, when known.
    pub updated_at: Option<String>,
    /// Issue body (truncated to 64 KiB by the extractor to
    /// bound memory). `None` when the body is empty.
    pub body: Option<String>,
}

/// Errors raised by the GitHub client. The variants are
/// surface-level — the dispatch helper in `mcp.rs` maps them
/// to the `github_auth_required` / `github_rate_limited` /
/// `github_api_error` envelope codes.
#[cfg(feature = "multimodal")]
#[derive(Debug, Error)]
pub enum GitHubError {
    /// The supplied `GITHUB_TOKEN` is missing or the API
    /// returned 401. The dispatch helper maps this to
    /// `error_code: "github_auth_required"`.
    #[error("github api: token required (set GITHUB_TOKEN)")]
    AuthRequired,
    /// The API returned 403 with `X-RateLimit-Remaining: 0`.
    /// The dispatch helper maps this to
    /// `error_code: "github_rate_limited"`.
    #[error("github api: rate limit exceeded; set GITHUB_TOKEN to increase to 5000/hr")]
    RateLimited,
    /// Any other API error (5xx after retry, network timeout,
    /// malformed payload). The dispatch helper maps this to
    /// `error_code: "github_api_error"`.
    #[error("github api: {0}")]
    ApiError(String),
}

/// Pluggable port for the GitHub Issues REST API. The trait
/// is dyn-compatible: use `Box<dyn GitHubClient + Send + Sync>`
/// in tests, `Arc<dyn GitHubClient + Send + Sync>` in the
/// production extractor.
#[cfg(feature = "multimodal")]
#[async_trait]
pub trait GitHubClient: Send + Sync {
    /// List issues for `(owner, repo)` in the given state.
    /// The implementation handles pagination internally and
    /// returns the union of every page. Empty result is
    /// `Ok(vec![])`, not an error.
    async fn list_issues(
        &self,
        owner: &str,
        repo: &str,
        state: IssueState,
    ) -> Result<Vec<RawIssue>, GitHubError>;
}

#[cfg(all(test, feature = "multimodal"))]
mod tests {
    use super::*;

    /// `IssueState::as_str` returns the kebab-case identifier
    /// the GitHub API expects.
    #[test]
    fn issue_state_as_str() {
        assert_eq!(IssueState::Open.as_str(), "open");
        assert_eq!(IssueState::Closed.as_str(), "closed");
        assert_eq!(IssueState::All.as_str(), "all");
    }

    /// The `RawIssue` struct can be constructed with every
    /// field set.
    #[test]
    fn raw_issue_constructs() {
        let issue = RawIssue {
            number: 42,
            title: "Null pointer in render path".to_string(),
            state: "open".to_string(),
            url: "https://github.com/acme/widgets/issues/42".to_string(),
            labels: vec!["bug".to_string(), "p1".to_string()],
            assignee: Some("alice".to_string()),
            author: Some("bob".to_string()),
            created_at: Some("2026-06-10T13:00:00Z".to_string()),
            updated_at: Some("2026-06-10T15:30:00Z".to_string()),
            body: Some("Long body…".to_string()),
        };
        assert_eq!(issue.number, 42);
        assert_eq!(issue.labels.len(), 2);
    }

    /// The `GitHubError` variants produce distinct `Display`
    /// strings (so the dispatch helper's `error_code` mapping
    /// can match them by exact string).
    #[test]
    fn github_error_display_distinct() {
        assert_eq!(
            format!("{}", GitHubError::AuthRequired),
            "github api: token required (set GITHUB_TOKEN)"
        );
        assert_eq!(
            format!("{}", GitHubError::RateLimited),
            "github api: rate limit exceeded; set GITHUB_TOKEN to increase to 5000/hr"
        );
        let api = GitHubError::ApiError("500 internal".to_string());
        assert_eq!(
            format!("{api}"),
            "github api: 500 internal"
        );
    }
}
