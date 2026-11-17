//! The GitHub review adapter over a scripted `gh`: no network, no real
//! GitHub. Every call it makes is recorded so the tests can assert what
//! is (and is not) asked of GitHub.

use std::collections::VecDeque;
use std::sync::Mutex;

use cognicode_core::application::pull_request_review::{
    ReviewRequest, ReviewVerdict, review_pull_request,
};
use cognicode_core::domain::forge::{
    CheckConclusion, CheckPublisher, CheckReport, Evaluation, EvaluationError, EvaluationMode,
    EvaluationTarget, ExecutionGrant, ForgeError, HeadOrigin, MergeBase, MergePreview,
    PullRequestEvaluator, PullRequestId, ReviewReader, target_for_pr_diff,
};
use cognicode_core::domain::scm::{RevisionHandle, SourceRef};
use cognicode_core::infrastructure::github::{
    CommandCall, CommandOutput, CommandRunner, GhCliRunner, GitHubForge,
};

const HEAD: &str = "1111111111111111111111111111111111111111";
const HEAD_2: &str = "2222222222222222222222222222222222222222";
const BASE: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const MERGE: &str = "cccccccccccccccccccccccccccccccccccccccc";
const ANCESTOR: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// Answers by the `repos/...` path of the call. Several answers for one
/// path are served in order, the last one repeating.
struct ScriptedGh {
    routes: Mutex<Vec<(String, VecDeque<CommandOutput>)>>,
    calls: Mutex<Vec<CommandCall>>,
}

impl ScriptedGh {
    fn new() -> Self {
        Self {
            routes: Mutex::new(Vec::new()),
            calls: Mutex::new(Vec::new()),
        }
    }

    fn on(self, path: &str, output: CommandOutput) -> Self {
        {
            let mut routes = self.routes.lock().unwrap();
            match routes.iter_mut().find(|(known, _)| known == path) {
                Some((_, queue)) => queue.push_back(output),
                None => routes.push((path.to_string(), VecDeque::from([output]))),
            }
        }
        self
    }

    fn calls(&self) -> Vec<CommandCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl CommandRunner for &ScriptedGh {
    fn run(&self, call: &CommandCall) -> Result<CommandOutput, ForgeError> {
        self.calls.lock().unwrap().push(call.clone());
        let path = call
            .args
            .iter()
            .find(|arg| arg.starts_with("repos/"))
            .cloned()
            .unwrap_or_default();
        let mut routes = self.routes.lock().unwrap();
        let Some((_, queue)) = routes.iter_mut().find(|(known, _)| *known == path) else {
            return Err(ForgeError::ProviderFailure {
                detail: format!("unscripted call: {path}"),
            });
        };
        Ok(if queue.len() > 1 {
            queue.pop_front().unwrap()
        } else {
            queue.front().unwrap().clone()
        })
    }
}

fn ok(body: serde_json::Value) -> CommandOutput {
    CommandOutput {
        code: 0,
        stdout: serde_json::to_vec(&body).unwrap(),
        stderr: Vec::new(),
    }
}

fn http_error(status: u16, message: &str) -> CommandOutput {
    CommandOutput {
        code: 1,
        stdout: serde_json::to_vec(&serde_json::json!({ "message": message })).unwrap(),
        stderr: format!("gh: {message} (HTTP {status})").into_bytes(),
    }
}

fn pull(head: &str, head_repo: Option<&str>, mergeable: serde_json::Value) -> CommandOutput {
    ok(serde_json::json!({
        "head": { "sha": head, "repo": head_repo.map(|name| serde_json::json!({ "full_name": name })) },
        "base": { "sha": BASE, "ref": "main", "repo": { "full_name": "acme/widgets" } },
        "mergeable": mergeable,
        "merge_commit_sha": MERGE,
    }))
}

fn id() -> PullRequestId {
    PullRequestId::new(SourceRef::new("github", "acme/widgets").unwrap(), 7).unwrap()
}

fn rev(sha: &str) -> RevisionHandle {
    RevisionHandle::new(sha).unwrap()
}

const PULL: &str = "repos/acme/widgets/pulls/7";

#[test]
fn the_head_base_and_origin_come_from_the_pull_request() {
    for (head_repo, origin) in [
        (Some("acme/widgets"), HeadOrigin::SameRepository),
        (Some("Acme/Widgets"), HeadOrigin::SameRepository),
        (Some("mallory/widgets"), HeadOrigin::Fork),
        (None, HeadOrigin::Unknown),
    ] {
        let gh = ScriptedGh::new().on(PULL, pull(HEAD, head_repo, serde_json::Value::Null));
        let forge = GitHubForge::new(&gh);

        let state = forge.pull_request(&id()).unwrap();

        assert_eq!(state.head, rev(HEAD));
        assert_eq!(state.base_tip, rev(BASE));
        assert_eq!(state.base_ref, "main");
        assert_eq!(state.origin, origin, "{head_repo:?}");
    }
}

#[test]
fn only_metadata_is_requested_never_the_code_of_the_pull_request() {
    let gh = ScriptedGh::new()
        .on(PULL, pull(HEAD, Some("mallory/widgets"), true.into()))
        .on(
            &format!("repos/acme/widgets/compare/{BASE}...{HEAD}?per_page=1"),
            ok(serde_json::json!({ "merge_base_commit": { "sha": ANCESTOR } })),
        )
        .on(
            &format!("repos/acme/widgets/commits/{MERGE}"),
            ok(serde_json::json!({ "parents": [{ "sha": BASE }, { "sha": HEAD }] })),
        );
    let forge = GitHubForge::new(&gh);
    let state = forge.pull_request(&id()).unwrap();

    forge.merge_base(&state).unwrap();
    forge.merge_candidate(&state).unwrap();

    let calls = gh.calls();
    assert!(calls.len() >= 4);
    for call in calls {
        assert_eq!(call.args[0], "api", "{call:?}");
        assert!(
            call.args.iter().all(|arg| !arg.contains("checkout")
                && !arg.contains("clone")
                && !arg.contains("fetch")),
            "{call:?}"
        );
    }
}

#[test]
fn the_merge_base_is_read_and_unrelated_histories_are_absent() {
    let compare = format!("repos/acme/widgets/compare/{BASE}...{HEAD}?per_page=1");
    let found = ScriptedGh::new()
        .on(PULL, pull(HEAD, Some("acme/widgets"), true.into()))
        .on(
            &compare,
            ok(serde_json::json!({ "merge_base_commit": { "sha": ANCESTOR } })),
        );
    let forge = GitHubForge::new(&found);
    let state = forge.pull_request(&id()).unwrap();
    assert_eq!(
        forge.merge_base(&state).unwrap(),
        MergeBase::Found(rev(ANCESTOR))
    );

    let unrelated = ScriptedGh::new()
        .on(PULL, pull(HEAD, Some("acme/widgets"), true.into()))
        .on(
            &compare,
            http_error(
                404,
                &format!("No common ancestor between {BASE} and {HEAD}."),
            ),
        );
    let forge = GitHubForge::new(&unrelated);
    assert_eq!(forge.merge_base(&state).unwrap(), MergeBase::Absent);

    let missing = ScriptedGh::new().on(&compare, http_error(404, "Not Found"));
    let forge = GitHubForge::new(&missing);
    assert!(matches!(
        forge.merge_base(&state),
        Err(ForgeError::ProviderFailure { .. })
    ));
}

#[test]
fn the_merge_candidate_is_ready_only_for_a_merge_of_exactly_base_and_head() {
    let commit = format!("repos/acme/widgets/commits/{MERGE}");
    let cases = [
        (
            "merge of the current tips",
            true.into(),
            serde_json::json!({ "parents": [{ "sha": HEAD }, { "sha": BASE }] }),
            MergePreview::Ready(rev(MERGE)),
        ),
        (
            "merge of an older head",
            true.into(),
            serde_json::json!({ "parents": [{ "sha": BASE }, { "sha": ANCESTOR }] }),
            MergePreview::Pending,
        ),
        (
            "not a merge commit",
            true.into(),
            serde_json::json!({ "parents": [{ "sha": HEAD }] }),
            MergePreview::Pending,
        ),
        (
            "conflict",
            false.into(),
            serde_json::json!({}),
            MergePreview::Conflicting,
        ),
        (
            "not computed",
            serde_json::Value::Null,
            serde_json::json!({}),
            MergePreview::Pending,
        ),
    ];
    for (name, mergeable, parents, expected) in cases {
        let gh = ScriptedGh::new()
            .on(PULL, pull(HEAD, Some("acme/widgets"), mergeable))
            .on(&commit, ok(parents));
        let forge = GitHubForge::new(&gh);
        let state = forge.pull_request(&id()).unwrap();

        assert_eq!(forge.merge_candidate(&state).unwrap(), expected, "{name}");
    }
}

#[test]
fn a_candidate_for_a_head_that_moved_is_not_reused() {
    let gh = ScriptedGh::new()
        .on(PULL, pull(HEAD, Some("acme/widgets"), true.into()))
        .on(PULL, pull(HEAD_2, Some("acme/widgets"), true.into()));
    let forge = GitHubForge::new(&gh);
    let state = forge.pull_request(&id()).unwrap();

    assert_eq!(
        forge.merge_candidate(&state).unwrap(),
        MergePreview::Pending
    );
}

#[test]
fn github_refusals_keep_their_names() {
    let gh = ScriptedGh::new().on(PULL, http_error(404, "Not Found"));
    assert!(matches!(
        GitHubForge::new(&gh).pull_request(&id()),
        Err(ForgeError::PullRequestNotFound { .. })
    ));
    let gh = ScriptedGh::new().on(PULL, http_error(403, "Resource not accessible"));
    assert!(matches!(
        GitHubForge::new(&gh).pull_request(&id()),
        Err(ForgeError::PermissionDenied { .. })
    ));
    let gh = ScriptedGh::new().on(
        PULL,
        CommandOutput {
            code: 4,
            stdout: Vec::new(),
            stderr: b"gh: To use GitHub CLI, run: gh auth login".to_vec(),
        },
    );
    assert!(matches!(
        GitHubForge::new(&gh).pull_request(&id()),
        Err(ForgeError::ProviderFailure { .. })
    ));
    let gh = ScriptedGh::new().on(
        PULL,
        ok(serde_json::json!({ "head": { "sha": "not-a-sha" } })),
    );
    assert!(matches!(
        GitHubForge::new(&gh).pull_request(&id()),
        Err(ForgeError::MalformedResponse { .. })
    ));
}

#[test]
fn another_provider_or_a_hostile_repository_name_makes_no_call() {
    let gh = ScriptedGh::new();
    let forge = GitHubForge::new(&gh);

    let foreign =
        PullRequestId::new(SourceRef::new("bitbucket", "acme/widgets").unwrap(), 7).unwrap();
    assert!(matches!(
        forge.pull_request(&foreign),
        Err(ForgeError::ForeignProvider { .. })
    ));
    for hostile in [
        "acme/../admin",
        "acme/widgets/pulls",
        "acme",
        "-x/y",
        "a b/c",
    ] {
        let hostile = PullRequestId::new(SourceRef::new("github", hostile).unwrap(), 7).unwrap();
        assert!(
            matches!(
                forge.pull_request(&hostile),
                Err(ForgeError::MalformedIdentity { .. })
            ),
            "{hostile}"
        );
    }
    assert!(gh.calls().is_empty());
}

fn report(conclusion: CheckConclusion) -> CheckReport {
    let gh = ScriptedGh::new().on(PULL, pull(HEAD, Some("acme/widgets"), true.into()));
    let state = GitHubForge::new(&gh).pull_request(&id()).unwrap();
    let target = target_for_pr_diff(&state, MergeBase::Found(rev(ANCESTOR))).unwrap();
    CheckReport::for_target(&target, "cognicode", conclusion, "2 findings")
}

#[test]
fn a_check_is_published_on_the_evaluated_head_with_its_conclusion() {
    for (conclusion, wire) in [
        (CheckConclusion::Success, "success"),
        (CheckConclusion::Failure, "failure"),
        (CheckConclusion::ActionRequired, "action_required"),
        (CheckConclusion::Cancelled, "cancelled"),
    ] {
        let gh = ScriptedGh::new().on(
            "repos/acme/widgets/check-runs",
            ok(serde_json::json!({
                "id": 99, "head_sha": HEAD, "html_url": "https://github.com/acme/widgets/runs/99"
            })),
        );
        let forge = GitHubForge::new(&gh);

        let receipt = forge.publish(&report(conclusion)).unwrap();

        assert_eq!(receipt.attached_to, rev(HEAD));
        assert_eq!(receipt.reference, "https://github.com/acme/widgets/runs/99");
        let calls = gh.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(
            &calls[0].args[3..],
            [
                "-X",
                "POST",
                "repos/acme/widgets/check-runs",
                "--input",
                "-"
            ]
        );
        let body: serde_json::Value =
            serde_json::from_slice(calls[0].stdin.as_deref().unwrap()).unwrap();
        assert_eq!(body["head_sha"], HEAD);
        assert_eq!(body["status"], "completed");
        assert_eq!(body["conclusion"], wire);
        assert_eq!(body["name"], "cognicode");
        assert!(
            body["output"]["summary"]
                .as_str()
                .unwrap()
                .contains("2 findings")
        );
    }
}

#[test]
fn a_publication_without_permission_is_a_named_refusal() {
    let gh = ScriptedGh::new().on(
        "repos/acme/widgets/check-runs",
        http_error(403, "Resource not accessible by integration"),
    );

    let result = GitHubForge::new(&gh).publish(&report(CheckConclusion::Success));

    assert!(matches!(result, Err(ForgeError::PermissionDenied { .. })));
}

/// Approves whatever it is asked about.
struct Approver;

impl PullRequestEvaluator for Approver {
    fn evaluate(
        &self,
        target: &EvaluationTarget,
        _grant: &ExecutionGrant,
    ) -> Result<Evaluation, EvaluationError> {
        Ok(Evaluation {
            subject: target.subject().clone(),
            conclusion: CheckConclusion::Success,
            summary: "clean".to_string(),
        })
    }
}

fn request(mode: EvaluationMode) -> ReviewRequest {
    ReviewRequest {
        pull_request: id(),
        mode,
        check_name: "cognicode".to_string(),
    }
}

fn checks(gh: &ScriptedGh) -> Vec<serde_json::Value> {
    gh.calls()
        .into_iter()
        .filter(|call| call.stdin.is_some())
        .map(|call| serde_json::from_slice(call.stdin.as_deref().unwrap()).unwrap())
        .collect()
}

#[test]
fn a_merged_result_review_over_github_evaluates_the_merge_commit() {
    let gh = ScriptedGh::new()
        .on(PULL, pull(HEAD, Some("acme/widgets"), true.into()))
        .on(
            &format!("repos/acme/widgets/commits/{MERGE}"),
            ok(serde_json::json!({ "parents": [{ "sha": BASE }, { "sha": HEAD }] })),
        )
        .on(
            "repos/acme/widgets/check-runs",
            ok(serde_json::json!({ "id": 1, "head_sha": HEAD })),
        );
    let forge = GitHubForge::new(&gh);

    let outcome = review_pull_request(
        &forge,
        &forge,
        &Approver,
        &request(EvaluationMode::MergedResult),
    )
    .unwrap();

    assert!(matches!(outcome.verdict, ReviewVerdict::Evaluated { .. }));
    let published = checks(&gh);
    assert_eq!(published.len(), 1);
    assert_eq!(published[0]["head_sha"], HEAD);
    assert!(
        published[0]["output"]["summary"]
            .as_str()
            .unwrap()
            .contains(MERGE)
    );
}

#[test]
fn a_head_that_moves_on_github_during_analysis_is_published_as_cancelled_on_the_old_head() {
    // The first read sees HEAD; by the time the evaluation is over the
    // pull request reports HEAD_2.
    let gh = ScriptedGh::new()
        .on(PULL, pull(HEAD, Some("acme/widgets"), true.into()))
        .on(PULL, pull(HEAD_2, Some("acme/widgets"), true.into()))
        .on(
            &format!("repos/acme/widgets/compare/{BASE}...{HEAD}?per_page=1"),
            ok(serde_json::json!({ "merge_base_commit": { "sha": ANCESTOR } })),
        )
        .on(
            "repos/acme/widgets/check-runs",
            ok(serde_json::json!({ "id": 1, "head_sha": HEAD })),
        );
    let forge = GitHubForge::new(&gh);

    let outcome =
        review_pull_request(&forge, &forge, &Approver, &request(EvaluationMode::PrDiff)).unwrap();

    assert!(matches!(outcome.verdict, ReviewVerdict::Superseded { .. }));
    let published = checks(&gh);
    assert_eq!(published.len(), 1);
    assert_eq!(published[0]["head_sha"], HEAD);
    assert_eq!(published[0]["conclusion"], "cancelled");
}

#[test]
fn a_conflict_on_github_is_published_as_action_required() {
    let gh = ScriptedGh::new()
        .on(PULL, pull(HEAD, Some("acme/widgets"), false.into()))
        .on(
            "repos/acme/widgets/check-runs",
            ok(serde_json::json!({ "id": 1, "head_sha": HEAD })),
        );
    let forge = GitHubForge::new(&gh);

    let outcome = review_pull_request(
        &forge,
        &forge,
        &Approver,
        &request(EvaluationMode::MergedResult),
    )
    .unwrap();

    assert_eq!(outcome.report.conclusion(), CheckConclusion::ActionRequired);
    assert_eq!(checks(&gh)[0]["conclusion"], "action_required");
}

#[test]
fn a_receipt_for_another_revision_fails_the_review() {
    let gh = ScriptedGh::new()
        .on(PULL, pull(HEAD, Some("acme/widgets"), true.into()))
        .on(
            &format!("repos/acme/widgets/compare/{BASE}...{HEAD}?per_page=1"),
            ok(serde_json::json!({ "merge_base_commit": { "sha": ANCESTOR } })),
        )
        .on(
            "repos/acme/widgets/check-runs",
            ok(serde_json::json!({ "id": 1, "head_sha": HEAD_2 })),
        );
    let forge = GitHubForge::new(&gh);

    let result = review_pull_request(&forge, &forge, &Approver, &request(EvaluationMode::PrDiff));

    assert!(matches!(result, Err(ForgeError::ProviderFailure { .. })));
}

#[test]
fn a_missing_gh_program_is_a_named_failure_not_an_empty_answer() {
    let forge = GitHubForge::new(GhCliRunner::with_program("/nonexistent/cognicode-gh"));

    assert!(matches!(
        forge.pull_request(&id()),
        Err(ForgeError::ProviderFailure { .. })
    ));
}

#[test]
fn the_real_runner_hides_the_environment_feeds_stdin_and_bounds_time() {
    // `/bin/sh` stands in for `gh`: the runner, not the program, is under test.
    let runner =
        GhCliRunner::with_program("/bin/sh").with_timeout(std::time::Duration::from_millis(300));
    let call = |script: &str, stdin: Option<&[u8]>| CommandCall {
        args: vec!["-c".to_string(), script.to_string()],
        stdin: stdin.map(<[u8]>::to_vec),
    };

    let env = runner.run(&call("env", None)).unwrap();
    let env = String::from_utf8(env.stdout).unwrap();
    assert!(env.contains("GH_PROMPT_DISABLED=1"));
    assert!(
        !env.contains("CARGO_MANIFEST_DIR"),
        "the process environment must not leak into gh"
    );

    let echoed = runner.run(&call("cat", Some(b"{\"a\":1}"))).unwrap();
    assert_eq!(echoed.stdout, b"{\"a\":1}");

    let failed = runner.run(&call("echo boom >&2; exit 3", None)).unwrap();
    assert_eq!(failed.code, 3);
    assert_eq!(failed.stderr, b"boom\n");

    assert!(matches!(
        runner.run(&call("sleep 5", None)),
        Err(ForgeError::ProviderFailure { .. })
    ));
}
