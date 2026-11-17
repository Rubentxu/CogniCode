//! The GitHub review adapter held to the same contracts as the
//! in-memory forge, over a model of GitHub that lives in memory: no
//! network, no `gh`, no credential. The model answers the REST paths
//! the adapter requests from the state a scenario puts it in, and it
//! records every check run like GitHub does, on the `head_sha` it was
//! sent.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::{CommandCall, CommandOutput, CommandRunner, GitHubForge};
use crate::application::pull_request_review_contract::run_review_contract;
use crate::domain::forge::conformance::{
    ForgeFixture, ForgeHarness, ForgeScenario, ProvisionedForge, Shared, run_forge_conformance,
};
use crate::domain::forge::{ForgeError, HeadOrigin};
use crate::domain::scm::{RevisionHandle, SourceRef};

const PROJECT: &str = "acme/widgets";

/// What GitHub knows about one pull request.
struct World {
    pull_request: u64,
    head: String,
    base_tip: String,
    base_ref: String,
    head_repo: Option<String>,
    merge_base: Option<String>,
    mergeable: Option<bool>,
    merge_commit: Option<(String, Vec<String>)>,
    candidate: String,
    check_runs: Vec<Value>,
}

/// The model of GitHub the adapter talks to through a [`CommandRunner`].
struct ModelOfGitHub(Mutex<World>);

fn ok(body: Value) -> CommandOutput {
    CommandOutput {
        code: 0,
        stdout: serde_json::to_vec(&body).expect("json"),
        stderr: Vec::new(),
    }
}

fn http_error(status: u16, message: &str) -> CommandOutput {
    CommandOutput {
        code: 1,
        stdout: serde_json::to_vec(&json!({ "message": message })).expect("json"),
        stderr: format!("gh: {message} (HTTP {status})").into_bytes(),
    }
}

impl World {
    fn pull_request_json(&self) -> Value {
        json!({
            "head": {
                "sha": self.head,
                "repo": self.head_repo.as_ref().map(|name| json!({ "full_name": name })),
            },
            "base": {
                "sha": self.base_tip,
                "ref": self.base_ref,
                "repo": { "full_name": PROJECT },
            },
            "mergeable": self.mergeable,
            "merge_commit_sha": self.merge_commit.as_ref().map(|(sha, _)| sha),
        })
    }

    fn answer(&mut self, path: &str, body: Option<Value>) -> CommandOutput {
        let route = path
            .strip_prefix(&format!("repos/{PROJECT}/"))
            .unwrap_or("");
        if route == format!("pulls/{}", self.pull_request) {
            return ok(self.pull_request_json());
        }
        if route == format!("compare/{}...{}?per_page=1", self.base_tip, self.head) {
            return match &self.merge_base {
                Some(sha) => ok(json!({ "merge_base_commit": { "sha": sha } })),
                None => http_error(
                    404,
                    &format!(
                        "No common ancestor between {} and {}.",
                        self.base_tip, self.head
                    ),
                ),
            };
        }
        if let Some(sha) = route.strip_prefix("commits/") {
            return match &self.merge_commit {
                Some((known, parents)) if known == sha => ok(
                    json!({ "parents": parents.iter().map(|p| json!({ "sha": p })).collect::<Vec<_>>() }),
                ),
                _ => http_error(404, "Not Found"),
            };
        }
        if route == "check-runs"
            && let Some(body) = body
        {
            self.check_runs.push(body.clone());
            let id = self.check_runs.len();
            return ok(json!({
                "id": id,
                "head_sha": body["head_sha"],
                "html_url": format!("https://github.example/{PROJECT}/runs/{id}"),
            }));
        }
        http_error(404, "Not Found")
    }
}

struct Gh(Arc<ModelOfGitHub>);

impl CommandRunner for Gh {
    fn run(&self, call: &CommandCall) -> Result<CommandOutput, ForgeError> {
        let path = call
            .args
            .iter()
            .find(|arg| arg.starts_with("repos/"))
            .ok_or_else(|| ForgeError::ProviderFailure {
                detail: format!("a call without a repository path: {:?}", call.args),
            })?;
        let body = call
            .stdin
            .as_ref()
            .map(|bytes| serde_json::from_slice(bytes).expect("a JSON request body"));
        Ok(self.0.0.lock().expect("lock").answer(path, body))
    }
}

impl ForgeScenario for ModelOfGitHub {
    fn move_head(&self, to: &RevisionHandle) {
        let mut world = self.0.lock().expect("lock");
        world.head = to.as_str().to_string();
        // GitHub recomputes mergeability after a push.
        world.mergeable = None;
    }

    fn compute_candidate(&self) {
        let mut world = self.0.lock().expect("lock");
        world.mergeable = Some(true);
        world.merge_commit = Some((
            world.candidate.clone(),
            vec![world.base_tip.clone(), world.head.clone()],
        ));
    }

    fn conflict(&self) {
        self.0.lock().expect("lock").mergeable = Some(false);
    }

    fn unrelate_histories(&self) {
        self.0.lock().expect("lock").merge_base = None;
    }
}

struct GitHubHarness;

impl ForgeHarness for GitHubHarness {
    fn source(&self) -> SourceRef {
        SourceRef::new("github", PROJECT).expect("a fixed source")
    }

    fn provision(&self, fixture: &ForgeFixture) -> ProvisionedForge {
        let state = &fixture.pull_request;
        let head_repo = match state.origin {
            HeadOrigin::SameRepository => Some(PROJECT.to_string()),
            HeadOrigin::Fork => Some("mallory/widgets".to_string()),
            HeadOrigin::Unknown => None,
        };
        let model = Arc::new(ModelOfGitHub(Mutex::new(World {
            pull_request: state.id.number(),
            head: state.head.as_str().to_string(),
            base_tip: state.base_tip.as_str().to_string(),
            base_ref: state.base_ref.clone(),
            head_repo,
            merge_base: Some(fixture.merge_base.as_str().to_string()),
            mergeable: None,
            merge_commit: None,
            candidate: fixture.candidate.as_str().to_string(),
            check_runs: Vec::new(),
        })));
        let forge = Arc::new(GitHubForge::new(Gh(Arc::clone(&model))));
        ProvisionedForge {
            reader: Arc::new(Shared(Arc::clone(&forge))),
            publisher: Arc::new(Shared(forge)),
            scenario: model,
        }
    }
}

#[test]
fn the_github_adapter_meets_the_same_forge_contract_as_the_in_memory_forge() {
    run_forge_conformance(&GitHubHarness).assert_conforms("GitHubForge over a modelled GitHub");
}

#[test]
fn a_review_over_the_github_adapter_keeps_the_promises_it_keeps_over_the_in_memory_forge() {
    run_review_contract(&GitHubHarness).assert_conforms("the review over GitHubForge");
}
