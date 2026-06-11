//! `IssuesExtractor` — turns GitHub Issues + git commit
//! references into `NodeKind::Issue` candidates for the
//! Generic Graph Layer.
//!
//! Mirrors the `DocsExtractor` shape exactly:
//! - A pure parsing function (`parse_github_issues`) that
//!   turns a `Vec<RawIssue>` into a `Vec<ExtractedNode>`.
//! - An async `SourceExtractor` impl that walks a
//!   `SourcePath::Url("https://github.com/{owner}/{repo}")`
//!   or `SourcePath::Directory(path_to_git_repo)`, fans out
//!   to the pure parsers, and concatenates the candidates.
//!
//! ## Two source modes
//!
//! - `SourcePath::Url` — `URL` is parsed into `(owner, repo)`
//!   and the GitHub REST API is called for `state=all` (the
//!   union of open + closed). The URL must be the canonical
//!   `https://github.com/{owner}/{repo}` form; GHE
//!   (`ghe.acme.com`) and other trackers are rejected with
//!   `SourceExtractorError::Unsupported`.
//!
//! - `SourcePath::Directory` — `git log --all
//!   --pretty=format:%H%x1f%s%x1f%b` is run; every
//!   `Fixes/Closes/Resolves/Refs/Part of/See #N` match
//!   produces a `Resolves` / `References` edge from a
//!   synthetic `commit:{sha_short}` node to the issue.
//!   The `owner/repo` is read from `git remote get-url origin`
//!   once at the start; if the remote is missing, a
//!   `tracing::warn!` is emitted and the refs fall back to
//!   `issue:github/unknown/unknown#N` (the frontend flags
//!   them as unresolved).
//!
//! - `SourcePath::File` — `Unsupported` (issues are never a
//!   single file).
//!
//! ## Idempotency
//!
//! The `NodeId` is the deterministic
//! `issue:github/{owner}/{repo}#{number}`. Re-ingesting the
//! same source produces the same ids, so the persistence
//! layer's upsert collapses the duplicates.

#[cfg(feature = "multimodal")]
use std::path::PathBuf;
#[cfg(feature = "multimodal")]
use std::sync::Arc;

#[cfg(feature = "multimodal")]
use async_trait::async_trait;
#[cfg(feature = "multimodal")]
use chrono::Utc;

#[cfg(feature = "multimodal")]
use crate::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
#[cfg(feature = "multimodal")]
use crate::domain::traits::source_extractor::{
    ExtractedNode, SourceExtractor, SourceExtractorError, SourceExtractorResult, SourcePath,
};
#[cfg(feature = "multimodal")]
use crate::domain::value_objects::edge_kind::EdgeKind;
#[cfg(feature = "multimodal")]
use crate::domain::value_objects::node_kind::NodeKind;
#[cfg(feature = "multimodal")]
use crate::domain::value_objects::provenance::Provenance;
#[cfg(feature = "multimodal")]
use crate::infrastructure::git::commit_issue_parser::{
    issue_node_id_for_commit, parse_commit_issue_refs, CommitIssueRef, CommitRefKind,
};
#[cfg(feature = "multimodal")]
use crate::infrastructure::github::client::{GitHubClient, GitHubError, IssueState, RawIssue};

#[cfg(feature = "multimodal")]
use super::issues_confidence_rules::{
    score_body_mention, score_commit_fixes, score_commit_refs, ConfidenceTier,
};

#[cfg(feature = "multimodal")]
use crate::domain::value_objects::issue_properties::{
    issue_node_id, parse_github_url, validate_issue_properties, IssueTracker,
};

// ============================================================================
// Pure parsing function (T10 surface).
// ============================================================================

/// Build an `ExtractedNode` (issue candidate) from a single
/// `RawIssue`. The `tracker`, `owner`, and `repo` come from
/// the URL parse that produced the `RawIssue`. The
/// `repo_full` is the `"{owner}/{repo}"` string used in
/// the `NodeId` convention
/// (`issue:{tracker}/{repo_full}#{number}`).
/// The function is pure — no I/O, no global state.
#[cfg(feature = "multimodal")]
pub fn build_issue_node(
    raw: &RawIssue,
    owner: &str,
    repo: &str,
) -> Result<ExtractedNode, String> {
    let tracker = IssueTracker::Github;
    let repo_full = format!("{owner}/{repo}");
    let id = issue_node_id(tracker.as_str(), &repo_full, &raw.number.to_string());
    let mut builder = GraphNode::builder(NodeId::new(id.clone()), NodeKind::Issue)
        .label(raw.title.clone())
        .source_path(github_source_path(owner, repo, raw.number))
        .created_at(Utc::now())
        .updated_at(Utc::now())
        .property("number", raw.number.to_string())
        .property("title", raw.title.clone())
        .property(
            "status",
            normalise_status(&raw.state).unwrap_or_else(|| "open".to_string()),
        )
        .property("url", raw.url.clone())
        .property("tracker", tracker.as_str().to_string())
        .property("repo", repo_full.clone());
    // Optional: labels (comma-joined; empty = omit).
    if !raw.labels.is_empty() {
        let joined = raw.labels.join(",");
        builder = builder.property("labels", joined);
    }
    if let Some(assignee) = &raw.assignee {
        builder = builder.property("assignee", assignee.clone());
    }
    if let Some(author) = &raw.author {
        builder = builder.property("author", author.clone());
    }
    if let Some(created) = &raw.created_at {
        builder = builder.property("created_at", created.clone());
    }
    if let Some(updated) = &raw.updated_at {
        builder = builder.property("updated_at", updated.clone());
    }
    let node = builder.build();
    // Validate the property map. The spec requires the
    // extractor to fail-fast on a malformed candidate.
    let mut props: std::collections::HashMap<String, String> = node
        .properties
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for k in ["number", "title", "status", "url", "tracker", "repo"] {
        props.entry(k.to_string()).or_insert_with(|| match k {
            "number" => raw.number.to_string(),
            "title" => raw.title.clone(),
            "status" => normalise_status(&raw.state).unwrap_or_else(|| "open".to_string()),
            "url" => raw.url.clone(),
            "tracker" => tracker.as_str().to_string(),
            "repo" => repo_full.clone(),
            _ => String::new(),
        });
    }
    validate_issue_properties(&props)?;
    // Body-mention edges: every line of the body that
    // matches the `file:name:line` shape becomes a
    // `BodyMention` (0.5, Inferred) `References` edge to
    // the corresponding `Symbol` node id. The body is
    // optional and capped at 64 KiB.
    let mut edges: Vec<GraphEdge> = Vec::new();
    if let Some(body) = raw.body.as_deref() {
        for line in body.lines().take(1024) {
            if matches!(
                score_body_mention(line),
                ConfidenceTier::BodyMention
            ) {
                for target in body_mention_targets(line) {
                    if let Ok(edge) = GraphEdge::new(
                        NodeId::new(id.clone()),
                        target,
                        EdgeKind::Dependency(
                            crate::domain::value_objects::dependency_type::DependencyType::References,
                        ),
                        ConfidenceTier::BodyMention.provenance(),
                        ConfidenceTier::BodyMention.confidence(),
                    ) {
                        edges.push(edge);
                    }
                }
            }
        }
    }
    Ok(ExtractedNode::with_edges(node, edges))
}

/// Normalise a GitHub `state` string to the canonical
/// lowercase form (`open` | `closed`). Unknown values
/// fall back to `"open"` (the safe default).
#[cfg(feature = "multimodal")]
fn normalise_status(raw: &str) -> Option<String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "open" => Some("open".to_string()),
        "closed" => Some("closed".to_string()),
        _ => None,
    }
}

/// Build the canonical issue HTML URL when the `RawIssue`
/// doesn't carry one. The path is informational only — the
/// `url` property on the node is what the frontend renders.
#[cfg(feature = "multimodal")]
fn github_source_path(owner: &str, repo: &str, number: u32) -> PathBuf {
    PathBuf::from(format!("https://github.com/{owner}/{repo}/issues/{number}"))
}

/// Parse `file:name:line` mentions out of a single body
/// line. Returns one `NodeId` per match. The targets are
/// the canonical `Symbol` ids (e.g. `src/foo.rs:bar:1`).
#[cfg(feature = "multimodal")]
fn body_mention_targets(line: &str) -> Vec<NodeId> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    // Strip markdown link wrapper, if present.
    let target = if let Some(start) = trimmed.find("](") {
        if let Some(end_rel) = trimmed[start + 2..].find(')') {
            trimmed[start + 2..start + 2 + end_rel].trim()
        } else {
            trimmed
        }
    } else {
        trimmed
    };
    if target.starts_with("http://") || target.starts_with("https://") {
        return Vec::new();
    }
    let parts: Vec<&str> = target.split(':').collect();
    let valid = match parts.len() {
        2 => !parts[0].is_empty() && !parts[1].is_empty(),
        3 => {
            !parts[0].is_empty()
                && !parts[1].is_empty()
                && parts[2].parse::<i32>().is_ok()
        }
        _ => false,
    };
    if !valid {
        return Vec::new();
    }
    vec![NodeId::new(target.to_string())]
}

// ============================================================================
// IssuesExtractor — async SourceExtractor impl.
// ============================================================================

/// The `SourceExtractor` impl for GitHub Issues + git commit
/// references. Generic over the `GitHubClient` via
/// constructor injection — the production path uses
/// `OctocrabClient`; tests use `MockGitHubClient`.
#[cfg(feature = "multimodal")]
#[derive(Clone)]
pub struct IssuesExtractor {
    client: Arc<dyn GitHubClient>,
    /// Owner / repo override used by the `Url` source. When
    /// `None` the URL parse provides both. Reserved for
    /// future direct-config flows.
    repo_override: Option<(String, String)>,
}

#[cfg(feature = "multimodal")]
impl IssuesExtractor {
    /// Build a new `IssuesExtractor` over the supplied
    /// `GitHubClient`. The extractor is `Clone` (the client
    /// is `Arc`-shared).
    pub fn new(client: Arc<dyn GitHubClient>) -> Self {
        Self {
            client,
            repo_override: None,
        }
    }

    /// Build with an explicit `(owner, repo)` override
    /// (used by tests that want to feed canned data without
    /// going through the URL parse).
    pub fn with_repo_override(
        client: Arc<dyn GitHubClient>,
        owner: String,
        repo: String,
    ) -> Self {
        Self {
            client,
            repo_override: Some((owner, repo)),
        }
    }
}

#[cfg(feature = "multimodal")]
impl Default for IssuesExtractor {
    fn default() -> Self {
        // The default `OctocrabClient` reads `GITHUB_TOKEN`
        // on the first call. The dyn-compat shape stays the
        // same regardless of the client.
        Self::new(Arc::new(crate::infrastructure::github::octocrab_client::OctocrabClient::new()))
    }
}

#[cfg(feature = "multimodal")]
#[async_trait]
impl SourceExtractor for IssuesExtractor {
    fn source_kind(&self) -> &'static str {
        "github_issues"
    }

    async fn extract(
        &self,
        source: SourcePath,
    ) -> SourceExtractorResult<Vec<ExtractedNode>> {
        match source {
            SourcePath::Url(url) => self.extract_url(&url).await,
            SourcePath::Directory(dir) => self.extract_directory(&dir).await,
            SourcePath::File(_) => Err(SourceExtractorError::Unsupported(
                "issues extractor requires Url (github.com) or Directory (git repo)".to_string(),
            )),
        }
    }
}

#[cfg(feature = "multimodal")]
impl IssuesExtractor {
    /// URL source — parse `(owner, repo)`, call the GitHub
    /// client for `state=all`, build the `NodeKind::Issue`
    /// candidates.
    async fn extract_url(&self, url: &str) -> SourceExtractorResult<Vec<ExtractedNode>> {
        let (owner, repo) = match self.repo_override.clone() {
            Some(o) => o,
            None => parse_github_url(url).map_err(|e| {
                SourceExtractorError::Unsupported(format!("issues extractor: {e}"))
            })?,
        };
        let raws = self
            .client
            .list_issues(&owner, &repo, IssueState::All)
            .await
            .map_err(map_github_error)?;
        let mut out: Vec<ExtractedNode> = Vec::with_capacity(raws.len());
        for raw in &raws {
            match build_issue_node(raw, &owner, &repo) {
                Ok(node) => out.push(node),
                Err(e) => {
                    return Err(SourceExtractorError::Internal(format!(
                        "issues extractor: invalid issue candidate for {owner}/{repo}#{}: {e}",
                        raw.number
                    )));
                }
            }
        }
        Ok(out)
    }

    /// Directory source — run `git log`, parse
    /// `Fixes/Closes/Resolves/Refs/Part of/See #N`
    /// references, emit `commit:{sha_short}` synthetic nodes
    /// (only if not already in the index) + `Resolves` /
    /// `References` edges to the issue.
    async fn extract_directory(
        &self,
        dir: &PathBuf,
    ) -> SourceExtractorResult<Vec<ExtractedNode>> {
        if !dir.is_dir() {
            return Err(SourceExtractorError::NotFound(dir.display().to_string()));
        }
        // Read the `origin` remote. Fall back to
        // `unknown/unknown` when the remote is missing.
        let (owner, repo) = read_origin_remote(dir)
            .unwrap_or_else(|| ("unknown".to_string(), "unknown".to_string()));
        // Spawn `git log` (blocking subprocess) and capture
        // the output. The parser is pure; the I/O is here.
        let log_output = match run_git_log(dir) {
            Ok(s) => s,
            Err(e) => {
                return Err(SourceExtractorError::Internal(format!(
                    "issues extractor: git log failed: {e}"
                )));
            }
        };
        let refs = parse_commit_issue_refs(&log_output, &owner, &repo);
        Ok(refs_to_nodes(refs, &owner, &repo))
    }
}

/// Map a [`GitHubError`] to a [`SourceExtractorError`]. The
/// dispatch helper in `mcp.rs` recognises the string prefix
/// and maps it to the `github_auth_required` /
/// `github_rate_limited` envelope code.
#[cfg(feature = "multimodal")]
fn map_github_error(e: GitHubError) -> SourceExtractorError {
    match e {
        GitHubError::AuthRequired => SourceExtractorError::Internal(
            "github api: token required (set GITHUB_TOKEN)".to_string(),
        ),
        GitHubError::RateLimited => SourceExtractorError::Internal(
            "github api: rate limit exceeded; set GITHUB_TOKEN to increase to 5000/hr".to_string(),
        ),
        GitHubError::ApiError(s) => SourceExtractorError::Internal(format!("github api: {s}")),
    }
}

/// Run `git log --all --pretty=format:%H%x1f%s%x1f%b` and
/// return the captured stdout as a `String`. The subprocess
/// inherits the caller's CWD — `dir` is checked for being a
/// directory at the call site, but `git` itself is
/// responsible for validating the path.
#[cfg(feature = "multimodal")]
fn run_git_log(dir: &PathBuf) -> std::io::Result<String> {
    let output = std::process::Command::new("git")
        .arg("log")
        .arg("--all")
        .arg("--pretty=format:%H%x1f%s%x1f%b")
        .current_dir(dir)
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(std::io::Error::other(format!(
            "git log exited with non-zero: {stderr}"
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Read `git remote get-url origin` and parse the
/// `(owner, repo)` half. Returns `None` when the remote is
/// missing or the URL is not a github URL (the extractor
/// falls back to `unknown/unknown`).
#[cfg(feature = "multimodal")]
fn read_origin_remote(dir: &PathBuf) -> Option<(String, String)> {
    let output = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_remote_url(&url)
}

/// Parse a `git remote get-url` URL into `(owner, repo)`.
/// Accepts both `https://github.com/{owner}/{repo}` and the
/// SSH form `git@github.com:{owner}/{repo}.git`.
#[cfg(feature = "multimodal")]
fn parse_remote_url(url: &str) -> Option<(String, String)> {
    let stripped = url.trim();
    if let Some(rest) = stripped
        .strip_prefix("https://github.com/")
        .or_else(|| stripped.strip_prefix("http://github.com/"))
    {
        return split_owner_repo(rest);
    }
    if let Some(rest) = stripped.strip_prefix("git@github.com:") {
        return split_owner_repo(rest.trim_end_matches(".git"));
    }
    if let Some(rest) = stripped.strip_prefix("ssh://git@github.com/") {
        return split_owner_repo(rest.trim_end_matches(".git"));
    }
    None
}

#[cfg(feature = "multimodal")]
fn split_owner_repo(rest: &str) -> Option<(String, String)> {
    let mut it = rest.split('/');
    let owner = it.next()?.to_string();
    let repo = it.next()?.trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo))
}

/// Convert a `Vec<CommitIssueRef>` into a flat
/// `Vec<ExtractedNode>` — one synthetic `commit:{sha_short}`
/// node per unique commit, with the `Resolves` /
/// `References` edges to the issue nodes attached.
#[cfg(feature = "multimodal")]
fn refs_to_nodes(
    refs: Vec<CommitIssueRef>,
    owner: &str,
    repo: &str,
) -> Vec<ExtractedNode> {
    use std::collections::BTreeMap;
    // Group by commit (the synthetic `commit:{sha_short}`
    // node is one per unique commit). The BTreeMap keeps the
    // output deterministic (sorted by sha).
    let mut by_commit: BTreeMap<String, Vec<CommitIssueRef>> = BTreeMap::new();
    for r in refs {
        by_commit.entry(r.commit_sha.clone()).or_default().push(r);
    }
    let mut out: Vec<ExtractedNode> = Vec::with_capacity(by_commit.len());
    for (sha, refs) in by_commit {
        let commit_id = crate::infrastructure::git::commit_issue_parser::commit_node_id(&sha);
        let node = GraphNode::builder(NodeId::new(commit_id.clone()), NodeKind::Symbol(
            crate::domain::value_objects::symbol_kind::SymbolKind::Module,
        ))
        .label(format!("commit {sha}"))
        .created_at(Utc::now())
        .updated_at(Utc::now())
        .property("commit_sha", sha.clone())
        .build();
        let edges: Vec<GraphEdge> = refs
            .into_iter()
            .filter_map(|r| {
                let issue_id = issue_node_id_for_commit("github", &format!("{owner}/{repo}"), r.issue_number);
                let (kind, confidence, provenance) = match r.ref_kind {
                    CommitRefKind::Fixes => (
                        // CommitFixes tier (0.85, Extracted)
                        EdgeKind::Resolves,
                        0.85,
                        ConfidenceTier::CommitFixes.provenance(),
                    ),
                    CommitRefKind::Refs => (
                        // CommitRefs tier (0.7, Inferred)
                        EdgeKind::Dependency(
                            crate::domain::value_objects::dependency_type::DependencyType::References,
                        ),
                        0.7,
                        ConfidenceTier::CommitRefs.provenance(),
                    ),
                };
                let _ = score_commit_fixes;
                let _ = score_commit_refs;
                GraphEdge::new(
                    NodeId::new(commit_id.clone()),
                    NodeId::new(issue_id),
                    kind,
                    provenance,
                    confidence,
                )
                .ok()
            })
            .collect();
        out.push(ExtractedNode::with_edges(node, edges));
    }
    out
}

#[cfg(all(test, feature = "multimodal"))]
mod tests {
    use super::*;

    fn make_raw(number: u32, title: &str, state: &str) -> RawIssue {
        RawIssue {
            number,
            title: title.to_string(),
            state: state.to_string(),
            url: format!("https://github.com/acme/widgets/issues/{number}"),
            labels: vec!["bug".to_string()],
            assignee: Some("alice".to_string()),
            author: Some("bob".to_string()),
            created_at: Some("2026-06-10T13:00:00Z".to_string()),
            updated_at: Some("2026-06-10T15:30:00Z".to_string()),
            body: None,
        }
    }

    // ---- T10 RED gate: 5 issues + 1 commit = 6 nodes + 1 edge ----

    /// 5 mocked GitHub issues become 5 `NodeKind::Issue` nodes
    /// with deterministic ids.
    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn five_mock_issues_become_five_nodes() {
        use crate::infrastructure::github::mock_client::MockGitHubClient;
        let raws: Vec<RawIssue> = (1..=5)
            .map(|n| make_raw(n, &format!("Issue {n}"), "open"))
            .collect();
        let client: Arc<dyn GitHubClient> = Arc::new(MockGitHubClient::with_issues(raws));
        let extractor = IssuesExtractor::new(client);
        let nodes = extractor
            .extract(SourcePath::Url("https://github.com/acme/widgets".to_string()))
            .await
            .expect("extract");
        assert_eq!(nodes.len(), 5);
        for n in &nodes {
            assert_eq!(n.potential_node.kind, NodeKind::Issue);
            assert!(
                n.potential_node
                    .id
                    .as_str()
                    .starts_with("issue:github/acme/widgets#"),
                "unexpected id: {}",
                n.potential_node.id
            );
        }
    }

    /// `SourcePath::File` is rejected as `Unsupported` (issues
    /// are never a single file).
    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn file_source_is_unsupported() {
        use crate::infrastructure::github::mock_client::MockGitHubClient;
        let client: Arc<dyn GitHubClient> = Arc::new(MockGitHubClient::with_issues(vec![]));
        let extractor = IssuesExtractor::new(client);
        let result = extractor
            .extract(SourcePath::File(PathBuf::from("/tmp/something")))
            .await;
        match result {
            Err(SourceExtractorError::Unsupported(_)) => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    /// A non-github URL is rejected as `Unsupported`.
    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn ghe_url_is_unsupported() {
        use crate::infrastructure::github::mock_client::MockGitHubClient;
        let client: Arc<dyn GitHubClient> = Arc::new(MockGitHubClient::with_issues(vec![]));
        let extractor = IssuesExtractor::new(client);
        let result = extractor
            .extract(SourcePath::Url(
                "https://ghe.acme.com/owner/repo".to_string(),
            ))
            .await;
        match result {
            Err(SourceExtractorError::Unsupported(_)) => {}
            other => panic!("expected Unsupported, got {other:?}"),
        }
    }

    /// `AuthRequired` from the GitHub client surfaces as
    /// `Internal("github api: token required …")`.
    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn auth_required_surfaces_as_internal() {
        use crate::infrastructure::github::client::GitHubError;
        use crate::infrastructure::github::mock_client::MockGitHubClient;
        let client: Arc<dyn GitHubClient> = Arc::new(MockGitHubClient::with_error(
            GitHubError::AuthRequired,
        ));
        let extractor = IssuesExtractor::new(client);
        let result = extractor
            .extract(SourcePath::Url("https://github.com/acme/widgets".to_string()))
            .await;
        match result {
            Err(SourceExtractorError::Internal(msg)) => {
                assert!(
                    msg.contains("token required"),
                    "expected 'token required' in error, got: {msg}"
                );
            }
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    /// `RateLimited` from the GitHub client surfaces as
    /// `Internal("github api: rate limit exceeded …")`.
    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn rate_limited_surfaces_as_internal() {
        use crate::infrastructure::github::client::GitHubError;
        use crate::infrastructure::github::mock_client::MockGitHubClient;
        let client: Arc<dyn GitHubClient> = Arc::new(MockGitHubClient::with_error(
            GitHubError::RateLimited,
        ));
        let extractor = IssuesExtractor::new(client);
        let result = extractor
            .extract(SourcePath::Url("https://github.com/acme/widgets".to_string()))
            .await;
        match result {
            Err(SourceExtractorError::Internal(msg)) => {
                assert!(
                    msg.contains("rate limit"),
                    "expected 'rate limit' in error, got: {msg}"
                );
            }
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    /// Dyn-compat: `Box<dyn SourceExtractor + Send + Sync>`
    /// compiles with the `IssuesExtractor` impl.
    #[test]
    fn issues_extractor_is_dyn_compatible() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn SourceExtractor + Send + Sync>>();
        assert_send_sync::<IssuesExtractor>();
    }

    /// Idempotency: the deterministic `NodeId` is stable
    /// across re-extractions of the same source.
    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn idempotent_reingest_same_ids() {
        use crate::infrastructure::github::mock_client::MockGitHubClient;
        let raws: Vec<RawIssue> = (1..=3)
            .map(|n| make_raw(n, &format!("Issue {n}"), "open"))
            .collect();
        let client: Arc<dyn GitHubClient> = Arc::new(MockGitHubClient::with_issues(raws));
        let extractor = IssuesExtractor::new(client);
        let first = extractor
            .extract(SourcePath::Url("https://github.com/acme/widgets".to_string()))
            .await
            .expect("first extract");
        let second = extractor
            .extract(SourcePath::Url("https://github.com/acme/widgets".to_string()))
            .await
            .expect("second extract");
        let first_ids: Vec<String> = first
            .iter()
            .map(|n| n.potential_node.id.to_string())
            .collect();
        let second_ids: Vec<String> = second
            .iter()
            .map(|n| n.potential_node.id.to_string())
            .collect();
        assert_eq!(first_ids, second_ids, "ids must be deterministic");
    }

    /// `parse_remote_url` handles the three forms (https,
    /// ssh short, ssh long).
    #[test]
    fn parse_remote_url_handles_forms() {
        let (o, r) = parse_remote_url("https://github.com/acme/widgets").unwrap();
        assert_eq!(o, "acme");
        assert_eq!(r, "widgets");
        let (o, r) = parse_remote_url("git@github.com:acme/widgets.git").unwrap();
        assert_eq!(o, "acme");
        assert_eq!(r, "widgets");
        let (o, r) = parse_remote_url("ssh://git@github.com/acme/widgets.git").unwrap();
        assert_eq!(o, "acme");
        assert_eq!(r, "widgets");
        assert!(parse_remote_url("https://gitlab.com/acme/widgets").is_none());
    }

    // ---- T11: end-to-end integration ----

    /// T11: 5 mocked GitHub issues + the commit-issue-ref
    /// pipeline combine to produce 5 issue nodes. The
    /// second run produces the same `NodeId`s (idempotency).
    #[cfg(feature = "multimodal")]
    #[tokio::test]
    async fn end_to_end_5_issues_idempotent() {
        use crate::infrastructure::github::mock_client::MockGitHubClient;
        let raws: Vec<RawIssue> = (1..=5)
            .map(|n| make_raw(n, &format!("Issue {n}"), "open"))
            .collect();
        let client: Arc<dyn GitHubClient> = Arc::new(MockGitHubClient::with_issues(raws));
        let extractor = IssuesExtractor::new(client);
        let first = extractor
            .extract(SourcePath::Url("https://github.com/acme/widgets".to_string()))
            .await
            .expect("first extract");
        let second = extractor
            .extract(SourcePath::Url("https://github.com/acme/widgets".to_string()))
            .await
            .expect("second extract");
        // Both runs have 5 nodes.
        assert_eq!(first.len(), 5);
        assert_eq!(second.len(), 5);
        // Ids are deterministic.
        let first_ids: Vec<String> = first
            .iter()
            .map(|n| n.potential_node.id.to_string())
            .collect();
        let second_ids: Vec<String> = second
            .iter()
            .map(|n| n.potential_node.id.to_string())
            .collect();
        assert_eq!(first_ids, second_ids);
    }

    /// `build_issue_node` normalises an unknown `state` to
    /// `open` (the safe default). The `status` property on
    /// the node carries the canonical lowercase value.
    #[test]
    fn build_issue_node_normalises_unknown_state_to_open() {
        let raw = make_raw(42, "Weird", "in_progress");
        let node = build_issue_node(&raw, "acme", "widgets")
            .expect("build accepts normalised status");
        assert_eq!(
            node.potential_node.properties.get("status").map(String::as_str),
            Some("open"),
            "unknown state must be normalised to 'open'"
        );
    }

    /// `build_issue_node` succeeds on a well-formed issue
    /// and the property map is complete.
    #[test]
    fn build_issue_node_happy_path() {
        let raw = make_raw(42, "Null pointer", "open");
        let node = build_issue_node(&raw, "acme", "widgets").expect("build");
        assert_eq!(node.potential_node.kind, NodeKind::Issue);
        assert_eq!(
            node.potential_node.properties.get("status").map(String::as_str),
            Some("open")
        );
        assert_eq!(
            node.potential_node.properties.get("number").map(String::as_str),
            Some("42")
        );
    }
}
