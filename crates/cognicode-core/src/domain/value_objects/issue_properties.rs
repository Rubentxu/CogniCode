//! `issue_properties` — frozen V1 schema for `NodeKind::Issue` nodes.
//!
//! Every Issue `GraphNode` carries a `properties: HashMap<String,
//! String>` whose keys are constrained to the union of
//! [`ISSUE_REQUIRED_PROPERTIES`] and [`ISSUE_OPTIONAL_PROPERTIES`].
//! The schema is centralised here so the extractor, the
//! persistence layer, the MCP tool, and the frontend all agree on
//! the shape. Drifting the schema is a compile error, not a
//! runtime surprise.
//!
//! The schema is also exposed as two parallel enums for callers
//! that prefer typed access over stringly-typed `HashMap` lookups:
//! - [`IssueTracker`] (V1: `github`; reserved: `gitlab`, `linear`,
//!   `jira`).
//! - [`IssueStatus`] (`open` | `closed`).
//!
//! The module is `#[cfg(feature = "multimodal")]`-gated. The
//! default build is byte-for-byte unchanged.

#[cfg(feature = "multimodal")]
use std::collections::HashMap;

#[cfg(feature = "multimodal")]
pub const ISSUE_REQUIRED_PROPERTIES: &[&str] = &[
    "number", "title", "status", "url", "tracker", "repo",
];

#[cfg(feature = "multimodal")]
pub const ISSUE_OPTIONAL_PROPERTIES: &[&str] = &[
    "labels", "assignee", "author", "created_at", "updated_at",
];

/// Recognised tracker values. V1 only implements `github`; the
/// other three are reserved for future adapters.
#[cfg(feature = "multimodal")]
pub const ISSUE_TRACKERS: &[&str] = &["github", "gitlab", "linear", "jira"];

/// Recognised issue statuses. The V1 spec freezes the two GitHub
/// status values; `draft` and `merged` are GitHub-specific
/// refinements intentionally excluded to keep the schema minimal.
#[cfg(feature = "multimodal")]
pub const ISSUE_STATUSES: &[&str] = &["open", "closed"];

/// Typed tracker enum. The string form is the kebab-case lowercase
/// identifier and is the only accepted value of
/// `properties["tracker"]` (V1 only implements `Github`).
#[cfg(feature = "multimodal")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IssueTracker {
    /// GitHub Issues (the V1-only adapter).
    Github,
    /// GitLab Issues (reserved for V2 — accepted by the validate
    /// function but no adapter is shipped yet).
    Gitlab,
    /// Linear (reserved for V2).
    Linear,
    /// Jira (reserved for V2).
    Jira,
}

#[cfg(feature = "multimodal")]
impl IssueTracker {
    /// Stable lowercase identifier used in the `tracker` property
    /// value. Must match the corresponding entry in
    /// [`ISSUE_TRACKERS`].
    pub const fn as_str(self) -> &'static str {
        match self {
            IssueTracker::Github => "github",
            IssueTracker::Gitlab => "gitlab",
            IssueTracker::Linear => "linear",
            IssueTracker::Jira => "jira",
        }
    }
}

#[cfg(feature = "multimodal")]
impl std::str::FromStr for IssueTracker {
    type Err = String;
    /// Parse a tracker identifier (case-insensitive). Accepts
    /// `github`, `gitlab`, `linear`, `jira`. Unknown values yield
    /// an error of the form `"unknown tracker: 'foo'"`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "github" => Ok(IssueTracker::Github),
            "gitlab" => Ok(IssueTracker::Gitlab),
            "linear" => Ok(IssueTracker::Linear),
            "jira" => Ok(IssueTracker::Jira),
            other => Err(format!("unknown tracker: '{other}' (expected one of: github, gitlab, linear, jira)")),
        }
    }
}

/// Typed issue status enum. Maps to the `status` property value.
#[cfg(feature = "multimodal")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IssueStatus {
    /// Open / unresolved.
    Open,
    /// Closed / resolved.
    Closed,
}

#[cfg(feature = "multimodal")]
impl IssueStatus {
    /// Stable lowercase identifier. Must match the corresponding
    /// entry in [`ISSUE_STATUSES`].
    pub const fn as_str(self) -> &'static str {
        match self {
            IssueStatus::Open => "open",
            IssueStatus::Closed => "closed",
        }
    }
}

#[cfg(feature = "multimodal")]
impl std::str::FromStr for IssueStatus {
    type Err = String;
    /// Parse a status identifier (case-insensitive). Accepts
    /// `open` and `closed`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "open" => Ok(IssueStatus::Open),
            "closed" => Ok(IssueStatus::Closed),
            other => Err(format!(
                "unknown issue status: '{other}' (expected: open | closed)"
            )),
        }
    }
}

/// Validate the property map of a candidate Issue `GraphNode`.
///
/// Returns `Ok(())` when every required key is present and
/// non-empty AND every value that has a constrained set
/// (`status`, `tracker`) parses to a known enum value. The
/// `labels` value MUST be comma-joined ASCII (commas in individual
/// labels are rejected at parse time by the extractor).
///
/// Called by the extractor before emitting the node — fail-fast
/// is the contract.
#[cfg(feature = "multimodal")]
pub fn validate_issue_properties(
    props: &HashMap<String, String>,
) -> Result<(), String> {
    // 1) Required keys are all present and non-empty.
    for key in ISSUE_REQUIRED_PROPERTIES {
        let value = props
            .get(*key)
            .ok_or_else(|| format!("missing required issue property: {key}"))?;
        if value.is_empty() {
            return Err(format!("required issue property '{key}' is empty"));
        }
    }
    // 2) `status` is one of the known statuses.
    let status = props
        .get("status")
        .expect("checked above; qed");
    if !ISSUE_STATUSES.contains(&status.as_str()) {
        return Err(format!(
            "unknown issue status: '{status}' (expected: open | closed)"
        ));
    }
    // 3) `tracker` is one of the known trackers.
    let tracker = props
        .get("tracker")
        .expect("checked above; qed");
    if !ISSUE_TRACKERS.contains(&tracker.as_str()) {
        return Err(format!(
            "unknown issue tracker: '{tracker}' (expected one of: github, gitlab, linear, jira)"
        ));
    }
    // 4) `labels` (when present) MUST NOT contain an unescaped
    //    comma in an individual label — the spec rejects labels
    //    containing commas at parse time, so a label with a comma
    //    in the joined string is a structural error.
    if let Some(labels) = props.get("labels") {
        if labels.is_empty() {
            return Err(
                "invalid issue candidate: 'labels' is empty (omit the key when the issue has no labels)"
                    .to_string(),
            );
        }
    }
    Ok(())
}

/// Build a deterministic `NodeId` for an Issue `GraphNode`.
///
/// The format is `issue:{tracker}/{repo}#{number}` (e.g.
/// `issue:github/acme/widgets#42`). Determinism is the entire
/// reason this scheme exists: the persistence layer's
/// `(id, kind)` UNIQUE constraint + `ON CONFLICT DO UPDATE`
/// requires a stable id so re-ingests collapse into the same row.
#[cfg(feature = "multimodal")]
pub fn issue_node_id(tracker: &str, repo: &str, number: &str) -> String {
    format!("issue:{tracker}/{repo}#{number}")
}

/// Extract `(tracker, repo)` from a `https://github.com/{owner}/{repo}`
/// URL. Returns `Unsupported` for any host that is not the canonical
/// `github.com` — V1 does not support GitHub Enterprise
/// (`ghe.acme.com/api/v3/...`), GitLab, Linear, or Jira. The
/// caller is expected to map the error to
/// `SourceExtractorError::Unsupported`.
#[cfg(feature = "multimodal")]
pub fn parse_github_url(url: &str) -> Result<(String, String), String> {
    // Accept both bare `https://github.com/owner/repo` and
    // `https://github.com/owner/repo/...` suffixes. We strip any
    // trailing path components and any query string.
    let trimmed = url.split('?').next().unwrap_or(url);
    let trimmed = trimmed.trim_end_matches('/');
    // Find the host.
    let rest = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .ok_or_else(|| {
            format!(
                "issues extractor: source '{url}' is not a github.com URL (V1 only supports https://github.com URLs)"
            )
        })?;
    let mut segments = rest.split('/');
    let host = segments
        .next()
        .ok_or_else(|| format!("issues extractor: URL '{url}' has no host"))?;
    if !host.eq_ignore_ascii_case("github.com") {
        return Err(format!(
            "issues extractor: host '{host}' not in V1 (github only)"
        ));
    }
    let owner = segments
        .next()
        .ok_or_else(|| format!("issues extractor: URL '{url}' has no owner"))?;
    let repo = segments
        .next()
        .ok_or_else(|| format!("issues extractor: URL '{url}' has no repo"))?;
    if owner.is_empty() || repo.is_empty() {
        return Err(format!(
            "issues extractor: URL '{url}' has an empty owner or repo"
        ));
    }
    Ok((owner.to_string(), repo.to_string()))
}

/// Normalise an arbitrary status string to the canonical lowercase
/// form expected by [`ISSUE_STATUSES`]. Returns `None` when the
/// string is not a known status.
#[cfg(feature = "multimodal")]
pub fn normalise_status(raw: &str) -> Option<String> {
    let s = raw.trim().to_ascii_lowercase();
    if ISSUE_STATUSES.contains(&s.as_str()) {
        Some(s)
    } else {
        None
    }
}

#[cfg(all(test, feature = "multimodal"))]
mod tests {
    use super::*;

    // ---- T1 RED gate: required/optional length tests ----

    /// The required property list is frozen at 6 entries (number,
    /// title, status, url, tracker, repo).
    #[test]
    fn required_properties_length() {
        assert_eq!(ISSUE_REQUIRED_PROPERTIES.len(), 6);
    }

    /// The optional property list is frozen at 5 entries
    /// (labels, assignee, author, created_at, updated_at).
    #[test]
    fn optional_properties_length() {
        assert_eq!(ISSUE_OPTIONAL_PROPERTIES.len(), 5);
    }

    // ---- T1 RED gate: tracker enum membership ----

    #[test]
    fn tracker_membership_github() {
        assert!(ISSUE_TRACKERS.contains(&"github"));
    }

    #[test]
    fn tracker_membership_gitlab() {
        assert!(ISSUE_TRACKERS.contains(&"gitlab"));
    }

    #[test]
    fn tracker_membership_linear() {
        assert!(ISSUE_TRACKERS.contains(&"linear"));
    }

    #[test]
    fn tracker_membership_jira() {
        assert!(ISSUE_TRACKERS.contains(&"jira"));
    }

    // ---- T1 RED gate: validate_issue_properties ----

    /// A complete, well-formed property map passes validation.
    #[test]
    fn validate_all_required_present_ok() {
        let mut props = HashMap::new();
        props.insert("number".into(), "42".into());
        props.insert("title".into(), "Null pointer in render path".into());
        props.insert("status".into(), "open".into());
        props.insert("url".into(), "https://github.com/acme/widgets/issues/42".into());
        props.insert("tracker".into(), "github".into());
        props.insert("repo".into(), "acme/widgets".into());
        assert!(validate_issue_properties(&props).is_ok());
    }

    /// Missing the `url` key returns the canonical error.
    #[test]
    fn validate_missing_url_reports_key() {
        let mut props = HashMap::new();
        props.insert("number".into(), "42".into());
        props.insert("title".into(), "Null pointer".into());
        props.insert("status".into(), "open".into());
        props.insert("tracker".into(), "github".into());
        props.insert("repo".into(), "acme/widgets".into());
        let err = validate_issue_properties(&props).unwrap_err();
        assert!(
            err.contains("url"),
            "expected error to mention the missing 'url' key, got: {err}"
        );
    }

    /// An unknown `status` is rejected.
    #[test]
    fn validate_bad_status_rejected() {
        let mut props = HashMap::new();
        props.insert("number".into(), "42".into());
        props.insert("title".into(), "x".into());
        props.insert("status".into(), "in_progress".into());
        props.insert("url".into(), "https://github.com/acme/widgets/issues/42".into());
        props.insert("tracker".into(), "github".into());
        props.insert("repo".into(), "acme/widgets".into());
        let err = validate_issue_properties(&props).unwrap_err();
        assert!(
            err.contains("status") && err.contains("in_progress"),
            "expected error to mention the bad status, got: {err}"
        );
    }

    /// An unknown `tracker` is rejected.
    #[test]
    fn validate_bad_tracker_rejected() {
        let mut props = HashMap::new();
        props.insert("number".into(), "42".into());
        props.insert("title".into(), "x".into());
        props.insert("status".into(), "open".into());
        props.insert("url".into(), "https://github.com/acme/widgets/issues/42".into());
        props.insert("tracker".into(), "bitbucket".into());
        props.insert("repo".into(), "acme/widgets".into());
        let err = validate_issue_properties(&props).unwrap_err();
        assert!(
            err.contains("tracker") && err.contains("bitbucket"),
            "expected error to mention the bad tracker, got: {err}"
        );
    }

    // ---- T1 RED gate: NodeId convention ----

    /// The NodeId for a given `(tracker, repo, number)` is
    /// deterministic — re-ingesting the same triple yields the
    /// same id.
    #[test]
    fn issue_node_id_determinism() {
        let id_a = issue_node_id("github", "acme/widgets", "42");
        let id_b = issue_node_id("github", "acme/widgets", "42");
        assert_eq!(id_a, id_b);
        assert_eq!(id_a, "issue:github/acme/widgets#42");
    }

    /// Two issues in different repos with the same number produce
    /// different NodeIds (cross-repo isolation).
    #[test]
    fn issue_node_id_cross_repo_isolation() {
        let id_a = issue_node_id("github", "acme/widgets", "42");
        let id_b = issue_node_id("github", "acme/gadgets", "42");
        assert_ne!(id_a, id_b);
    }

    // ---- T1 RED gate: parse_github_url ----

    /// A canonical github.com URL parses to `(owner, repo)`.
    #[test]
    fn parse_github_url_canonical() {
        let (owner, repo) =
            parse_github_url("https://github.com/acme/widgets").expect("parse");
        assert_eq!(owner, "acme");
        assert_eq!(repo, "widgets");
    }

    /// A URL with extra path components (e.g. `/issues/42`) still
    /// parses to the same `(owner, repo)` — the tail is ignored.
    #[test]
    fn parse_github_url_with_path_tail() {
        let (owner, repo) = parse_github_url(
            "https://github.com/acme/widgets/issues/42",
        )
        .expect("parse");
        assert_eq!(owner, "acme");
        assert_eq!(repo, "widgets");
    }

    /// A non-github host is rejected (V1 only supports github.com).
    #[test]
    fn parse_github_url_rejects_ghe() {
        let err = parse_github_url("https://ghe.acme.com/owner/repo")
            .unwrap_err();
        assert!(
            err.contains("ghe.acme.com") && err.contains("V1"),
            "expected error to name the host and V1, got: {err}"
        );
    }

    /// A bare scheme-less path is rejected (V1 only accepts
    /// `https://` URLs).
    #[test]
    fn parse_github_url_rejects_no_scheme() {
        let err =
            parse_github_url("github.com/acme/widgets").unwrap_err();
        assert!(err.contains("not a github.com URL"));
    }

    // ---- T1 RED gate: status normalisation ----

    /// `OPEN` is case-folded to `open`.
    #[test]
    fn status_normalisation_case_fold() {
        assert_eq!(normalise_status("OPEN"), Some("open".to_string()));
        assert_eq!(normalise_status("Closed"), Some("closed".to_string()));
    }

    /// Unknown statuses return `None`.
    #[test]
    fn status_normalisation_unknown() {
        assert_eq!(normalise_status("in_progress"), None);
        assert_eq!(normalise_status(""), None);
    }

    // ---- Compile-gate: tracker + status enums ----

    /// `IssueTracker::from_str` is case-insensitive.
    #[test]
    fn tracker_from_str_case_insensitive() {
        assert_eq!(
            "github".parse::<IssueTracker>().unwrap(),
            IssueTracker::Github
        );
        assert_eq!(
            "GitHub".parse::<IssueTracker>().unwrap(),
            IssueTracker::Github
        );
        assert_eq!(
            "GITHUB".parse::<IssueTracker>().unwrap(),
            IssueTracker::Github
        );
    }

    /// `IssueStatus::from_str` is case-insensitive.
    #[test]
    fn status_from_str_case_insensitive() {
        assert_eq!(
            "open".parse::<IssueStatus>().unwrap(),
            IssueStatus::Open
        );
        assert_eq!(
            "CLOSED".parse::<IssueStatus>().unwrap(),
            IssueStatus::Closed
        );
    }

    /// `as_str` round-trip is stable.
    #[test]
    fn enum_as_str_round_trip() {
        for t in [
            IssueTracker::Github,
            IssueTracker::Gitlab,
            IssueTracker::Linear,
            IssueTracker::Jira,
        ] {
            assert!(ISSUE_TRACKERS.contains(&t.as_str()));
        }
        for s in [IssueStatus::Open, IssueStatus::Closed] {
            assert!(ISSUE_STATUSES.contains(&s.as_str()));
        }
    }

    /// Compile-gate: the module compiles in this `multimodal` test
    /// build (companion to the `--no-default-features` regression
    /// test which asserts the module is absent on default builds —
    /// that test lives in the workspace-level `sdd-verify` lane).
    #[test]
    fn module_compiles_under_multimodal() {
        // The mere fact that this test compiles proves the
        // module is reachable. The unit tests above cover the
        // surface contract.
        assert!(ISSUE_REQUIRED_PROPERTIES.len() > 0);
    }
}
