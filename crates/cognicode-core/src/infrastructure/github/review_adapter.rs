//! GitHub as a forge for the review pipeline: reads pull-request facts
//! and publishes check runs through `gh api`, behind an injected
//! [`CommandRunner`].
//!
//! ## What it does and does not do
//!
//! * It only issues REST calls (`gh api ...`) for METADATA: the pull
//!   request, the merge-base comparison, the merge commit and the
//!   check-run publication. It never fetches, checks out or runs the
//!   code of a pull request, so a fork's code never executes under the
//!   permissions of the base repository from here.
//! * Every identity placed in a request path (owner, repository, sha) is
//!   validated first; none can add a path segment, a query or an option.
//! * The head origin is derived from the repository the head lives in:
//!   the same repository as the base is `SameRepository`, another one is
//!   `Fork`, a missing one (deleted fork) is `Unknown`.
//! * The merge candidate is `Ready` only when GitHub reports the pull
//!   request mergeable AND the merge commit it names has exactly the
//!   current base tip and head as parents. A merge commit of any other
//!   pair, or one GitHub has not computed yet, is `Pending`.
//! * Check runs are published on the evaluated head and nowhere else; the
//!   receipt carries the revision GitHub recorded.
//!
//! Creating check runs requires a GitHub App installation token with
//! `checks: write` (a user or `GITHUB_TOKEN` of an Actions job also
//! works for its own repository). Without it publication fails with
//! [`ForgeError::PermissionDenied`]; it never falls back to commit
//! statuses or comments, which carry different review semantics.

use serde_json::{Value, json};

use crate::domain::forge::{
    CheckConclusion, CheckPublisher, CheckReport, ForgeCapabilities, ForgeError, HeadOrigin,
    MergeBase, MergePreview, PublishedCheck, PullRequestId, PullRequestState, ReviewReader,
};
use crate::domain::scm::RevisionHandle;

use super::gh_runner::{CommandCall, CommandOutput, CommandRunner};

const PROVIDER: &str = "github";
const ERROR_DETAIL_CHARS: usize = 400;

/// The GitHub implementation of [`ReviewReader`] and [`CheckPublisher`].
pub struct GitHubForge<R: CommandRunner> {
    runner: R,
}

/// An HTTP answer that was not a success, as `gh api` reports it.
struct HttpFailure {
    status: u16,
    message: String,
}

impl<R: CommandRunner> GitHubForge<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    fn call(&self, args: &[&str], stdin: Option<Vec<u8>>) -> Result<CommandOutput, ForgeError> {
        let mut argv = vec!["api".to_string(), "-H".to_string()];
        argv.push("Accept: application/vnd.github+json".to_string());
        argv.extend(args.iter().map(|arg| arg.to_string()));
        self.runner.run(&CommandCall { args: argv, stdin })
    }

    /// GET `path`; the outer error is a transport or protocol failure,
    /// the inner one an HTTP answer the caller may interpret.
    fn get(&self, path: &str) -> Result<Result<Value, HttpFailure>, ForgeError> {
        let output = self.call(&[path], None)?;
        parse_reply(&output)
    }

    fn pull_request_value(&self, id: &PullRequestId) -> Result<Value, ForgeError> {
        let project = project_of(id)?;
        let path = format!("repos/{project}/pulls/{}", id.number());
        match self.get(&path)? {
            Ok(value) => Ok(value),
            Err(failure) if failure.status == 404 => Err(ForgeError::PullRequestNotFound {
                pull_request: id.to_string(),
            }),
            Err(failure) => Err(refusal(failure)),
        }
    }
}

impl<R: CommandRunner> ReviewReader for GitHubForge<R> {
    fn capabilities(&self) -> ForgeCapabilities {
        ForgeCapabilities {
            merge_base: true,
            merge_candidate: true,
        }
    }

    fn pull_request(&self, id: &PullRequestId) -> Result<PullRequestState, ForgeError> {
        let value = self.pull_request_value(id)?;
        let head = revision(&value, &["head", "sha"])?;
        let base_tip = revision(&value, &["base", "sha"])?;
        let base_ref = text(&value, &["base", "ref"])?.to_string();
        let base_repo = text(&value, &["base", "repo", "full_name"])?;
        let origin = match value
            .pointer("/head/repo/full_name")
            .and_then(Value::as_str)
        {
            Some(head_repo) if head_repo.eq_ignore_ascii_case(base_repo) => {
                HeadOrigin::SameRepository
            }
            Some(_) => HeadOrigin::Fork,
            None => HeadOrigin::Unknown,
        };
        Ok(PullRequestState {
            id: id.clone(),
            head,
            base_ref,
            base_tip,
            origin,
        })
    }

    fn merge_base(&self, state: &PullRequestState) -> Result<MergeBase, ForgeError> {
        let project = project_of(&state.id)?;
        let path = format!(
            "repos/{project}/compare/{}...{}?per_page=1",
            sha_segment(&state.base_tip)?,
            sha_segment(&state.head)?
        );
        match self.get(&path)? {
            Ok(value) => Ok(MergeBase::Found(revision(
                &value,
                &["merge_base_commit", "sha"],
            )?)),
            Err(failure)
                if failure.status == 404 && failure.message.contains("No common ancestor") =>
            {
                Ok(MergeBase::Absent)
            }
            Err(failure) => Err(refusal(failure)),
        }
    }

    fn merge_candidate(&self, state: &PullRequestState) -> Result<MergePreview, ForgeError> {
        let value = self.pull_request_value(&state.id)?;
        // The candidate is a function of both tips: if either moved since
        // `state` was read, GitHub's answer is about another pair.
        if revision(&value, &["head", "sha"])? != state.head
            || revision(&value, &["base", "sha"])? != state.base_tip
        {
            return Ok(MergePreview::Pending);
        }
        match value.get("mergeable") {
            Some(Value::Bool(false)) => Ok(MergePreview::Conflicting),
            Some(Value::Bool(true)) => {
                let Some(candidate) = value
                    .get("merge_commit_sha")
                    .and_then(Value::as_str)
                    .and_then(|sha| RevisionHandle::new(sha).ok())
                else {
                    return Ok(MergePreview::Pending);
                };
                self.verify_candidate(state, candidate)
            }
            _ => Ok(MergePreview::Pending),
        }
    }
}

impl<R: CommandRunner> GitHubForge<R> {
    /// `Ready` only for a commit whose parents are exactly the base tip
    /// and the head.
    fn verify_candidate(
        &self,
        state: &PullRequestState,
        candidate: RevisionHandle,
    ) -> Result<MergePreview, ForgeError> {
        let project = project_of(&state.id)?;
        let path = format!("repos/{project}/commits/{}", sha_segment(&candidate)?);
        let commit = match self.get(&path)? {
            Ok(commit) => commit,
            // GitHub names a merge commit it has not materialized yet.
            Err(failure) if failure.status == 404 || failure.status == 422 => {
                return Ok(MergePreview::Pending);
            }
            Err(failure) => return Err(refusal(failure)),
        };
        let mut parents: Vec<&str> = commit
            .get("parents")
            .and_then(Value::as_array)
            .map(|parents| {
                parents
                    .iter()
                    .filter_map(|parent| parent.get("sha").and_then(Value::as_str))
                    .collect()
            })
            .unwrap_or_default();
        parents.sort_unstable();
        let mut expected = [state.base_tip.as_str(), state.head.as_str()];
        expected.sort_unstable();
        if parents == expected {
            Ok(MergePreview::Ready(candidate))
        } else {
            Ok(MergePreview::Pending)
        }
    }
}

impl<R: CommandRunner> CheckPublisher for GitHubForge<R> {
    fn publish(&self, report: &CheckReport) -> Result<PublishedCheck, ForgeError> {
        let project = project_of(report.pull_request())?;
        let head = sha_segment(report.attached_to())?;
        let body = check_run_body(report, head);
        let stdin = serde_json::to_vec(&body).map_err(|error| ForgeError::ProviderFailure {
            detail: format!("cannot encode the check run: {error}"),
        })?;
        let path = format!("repos/{project}/check-runs");
        let output = self.call(&["-X", "POST", &path, "--input", "-"], Some(stdin))?;
        let created = parse_reply(&output)?.map_err(refusal)?;
        let recorded = revision(&created, &["head_sha"])?;
        let reference = created
            .get("html_url")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or_else(|| created.get("id").map(|id| id.to_string()))
            .ok_or_else(|| ForgeError::MalformedResponse {
                detail: "the created check run has neither html_url nor id".to_string(),
            })?;
        Ok(PublishedCheck {
            attached_to: recorded,
            reference,
        })
    }
}

fn check_run_body(report: &CheckReport, head_sha: &str) -> Value {
    let conclusion = match report.conclusion() {
        CheckConclusion::Success => "success",
        CheckConclusion::Failure => "failure",
        CheckConclusion::ActionRequired => "action_required",
        CheckConclusion::Cancelled => "cancelled",
    };
    let mut summary = format!("Evaluation mode: {}", report.mode().as_str());
    if let Some(evaluated) = report.evaluated() {
        summary.push_str(&format!("\nEvaluated revision: {}", evaluated.as_str()));
    }
    summary.push_str("\n\n");
    summary.push_str(report.summary());
    json!({
        "name": report.name(),
        "head_sha": head_sha,
        "status": "completed",
        "conclusion": conclusion,
        "output": {
            "title": format!("{} ({})", report.name(), report.mode().as_str()),
            "summary": summary,
        },
    })
}

/// The repository path segment of a pull request, validated as
/// `owner/repo`, and the provider checked.
fn project_of(id: &PullRequestId) -> Result<&str, ForgeError> {
    let source = id.source();
    if source.provider().as_str() != PROVIDER {
        return Err(ForgeError::ForeignProvider {
            expected: PROVIDER,
            found: source.provider().as_str().to_string(),
        });
    }
    let project = source.project();
    let valid = match project.split_once('/') {
        Some((owner, repo)) => {
            name_ok(owner)
                && owner.starts_with(|c: char| c.is_ascii_alphanumeric())
                && name_ok(repo)
        }
        None => false,
    };
    if valid {
        Ok(project)
    } else {
        Err(ForgeError::MalformedIdentity {
            field: "github repository",
            value: project.to_string(),
        })
    }
}

fn name_ok(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// A revision as a path segment: hex only, so it cannot carry a slash,
/// a query or an option.
fn sha_segment(revision: &RevisionHandle) -> Result<&str, ForgeError> {
    let sha = revision.as_str();
    if is_sha(sha) {
        Ok(sha)
    } else {
        Err(ForgeError::MalformedIdentity {
            field: "git sha",
            value: sha.to_string(),
        })
    }
}

fn is_sha(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn text<'a>(value: &'a Value, path: &[&str]) -> Result<&'a str, ForgeError> {
    path.iter()
        .try_fold(value, |node, key| node.get(key))
        .and_then(Value::as_str)
        .ok_or_else(|| ForgeError::MalformedResponse {
            detail: format!("missing string field {}", path.join(".")),
        })
}

fn revision(value: &Value, path: &[&str]) -> Result<RevisionHandle, ForgeError> {
    let sha = text(value, path)?;
    if !is_sha(sha) {
        return Err(ForgeError::MalformedResponse {
            detail: format!("field {} is not a git sha: {sha:?}", path.join(".")),
        });
    }
    RevisionHandle::new(sha).map_err(|error| ForgeError::MalformedResponse {
        detail: error.to_string(),
    })
}

/// Interprets one `gh api` run: a success carries JSON on stdout; an
/// HTTP failure is `gh: <message> (HTTP <status>)` on stderr with the
/// API's JSON body on stdout; anything else is a transport failure.
fn parse_reply(output: &CommandOutput) -> Result<Result<Value, HttpFailure>, ForgeError> {
    if output.code == 0 {
        return serde_json::from_slice(&output.stdout)
            .map(Ok)
            .map_err(|error| ForgeError::MalformedResponse {
                detail: format!("gh did not return JSON: {error}"),
            });
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    match http_status(&stderr) {
        Some(status) => {
            let message = serde_json::from_slice::<Value>(&output.stdout)
                .ok()
                .and_then(|body| {
                    body.get("message")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| stderr.trim().to_string());
            Ok(Err(HttpFailure { status, message }))
        }
        None => Err(ForgeError::ProviderFailure {
            detail: format!(
                "gh exited with {}: {}",
                output.code,
                clip(stderr.trim(), ERROR_DETAIL_CHARS)
            ),
        }),
    }
}

/// The status in `... (HTTP 404)`.
fn http_status(stderr: &str) -> Option<u16> {
    let rest = &stderr[stderr.find("(HTTP ")? + "(HTTP ".len()..];
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn refusal(failure: HttpFailure) -> ForgeError {
    let detail = format!(
        "HTTP {}: {}",
        failure.status,
        clip(&failure.message, ERROR_DETAIL_CHARS)
    );
    match failure.status {
        401 | 403 => ForgeError::PermissionDenied { detail },
        _ => ForgeError::ProviderFailure { detail },
    }
}

fn clip(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}
