use super::*;
use crate::application::advisory_correlation::{FeedStatus, Freshness};
use crate::domain::advisory_matching::{PackageState, Reason};
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let none = root.join("no-git-configuration");
    let output = Command::new("git")
        .env("PATH", "/usr/bin:/bin")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", &none)
        .env("GIT_CONFIG_SYSTEM", &none)
        .output()
        .expect("git is installed");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

const MANIFEST: &str =
    "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n";

const LOCK: &str = "version = 3\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\"serde\"]\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.100\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"\n";

const FEED: &str = r#"[
  {"id":"RUSTSEC-2099-0001","modified":"2099-01-10T00:00:00Z","summary":"a flaw",
   "affected":[{"package":{"ecosystem":"crates.io","name":"serde"},
     "ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.0.150"}]}]}]}
]"#;

/// 2099-01-10T00:00:00Z, the `modified` of the record of `FEED`.
const PUBLISHED: i64 = 4071686400;

/// A repository tracking the lockfile; the feed is written but not tracked.
fn repository() -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("repository");
    let root = directory.path();
    git(root, &["init", "--quiet"]);
    git(root, &["config", "user.name", "CogniCode tests"]);
    git(root, &["config", "user.email", "tests@example.invalid"]);
    std::fs::write(root.join("Cargo.toml"), MANIFEST).expect("manifest");
    std::fs::write(root.join("Cargo.lock"), LOCK).expect("lock");
    git(root, &["add", "-A"]);
    git(root, &["commit", "--quiet", "-m", "base"]);
    std::fs::write(root.join("feed.json"), FEED).expect("feed");
    directory
}

fn input(feed: Option<&str>) -> AdvisoriesInput {
    let mut value = serde_json::json!({});
    if let Some(feed) = feed {
        value["feed"] = feed.into();
    }
    serde_json::from_value(value).expect("every argument has a default")
}

fn state(answer: &GoalAdvisories, name: &str) -> PackageState {
    answer.advisories.units[0]
        .packages
        .iter()
        .find(|package| package.name == name)
        .unwrap_or_else(|| panic!("{name} is resolved"))
        .state
}

fn digest_of(text: &str) -> String {
    ContentDigest::from_bytes(text.as_bytes())
        .as_hex()
        .to_string()
}

#[test]
fn a_feed_in_the_workspace_marks_the_locked_version_it_covers() {
    let repo = repository();

    let answer = advisories_at(repo.path(), &input(Some("feed.json")), PUBLISHED).expect("answer");

    assert_eq!(state(&answer, "serde"), PackageState::Affected);
    assert_eq!(state(&answer, "app"), PackageState::NotEvaluated);
    let feed = &answer.advisories.feed;
    assert_eq!(feed.status, FeedStatus::Usable);
    assert_eq!(feed.digest.as_deref(), Some(digest_of(FEED).as_str()));
    assert!(!feed.pinned);
    assert_eq!(answer.schema_version, "cognicode.goal.advisories/v1");
    assert!(answer.limits.iter().any(|l| l.contains("exploitability")));
}

#[test]
fn without_a_feed_nothing_is_evaluated_and_nothing_is_clean() {
    let repo = repository();

    let answer = advisories_at(repo.path(), &input(None), PUBLISHED).expect("answer");

    assert_eq!(answer.advisories.feed.status, FeedStatus::Absent);
    assert_eq!(answer.advisories.totals.not_found_in_scope, 0);
    assert_eq!(state(&answer, "serde"), PackageState::NotEvaluated);
}

#[test]
fn a_missing_feed_file_is_unreadable_not_clean() {
    let repo = repository();

    let answer =
        advisories_at(repo.path(), &input(Some("absent.json")), PUBLISHED).expect("answer");

    assert_eq!(answer.advisories.feed.status, FeedStatus::Unreadable);
    assert_eq!(answer.advisories.totals.not_found_in_scope, 0);
    assert!(
        answer.advisories.units[0].packages[1]
            .reasons
            .contains(&Reason::FeedUnavailable)
    );
}

#[test]
fn a_feed_outside_the_workspace_is_refused_as_unreadable() {
    let repo = repository();
    let outside = tempfile::tempdir().expect("outside");
    let feed = outside.path().join("feed.json");
    std::fs::write(&feed, FEED).expect("feed");

    let answer = advisories_at(
        repo.path(),
        &input(Some(&feed.to_string_lossy())),
        PUBLISHED,
    )
    .expect("answer");

    assert_eq!(answer.advisories.feed.status, FeedStatus::Unreadable);
    assert!(answer.advisories.feed.digest.is_none());
    assert_eq!(state(&answer, "serde"), PackageState::NotEvaluated);
}

#[test]
fn a_pinned_digest_that_matches_is_reported_as_pinned() {
    let repo = repository();
    let ask = AdvisoriesInput {
        expected_digest: Some(format!("sha256:{}", digest_of(FEED))),
        ..input(Some("feed.json"))
    };

    let answer = advisories_at(repo.path(), &ask, PUBLISHED).expect("answer");

    assert!(answer.advisories.feed.pinned);
    assert_eq!(answer.advisories.feed.status, FeedStatus::Usable);
}

#[test]
fn a_pinned_digest_that_differs_discards_the_feed() {
    let repo = repository();
    let ask = AdvisoriesInput {
        expected_digest: Some("0".repeat(64)),
        ..input(Some("feed.json"))
    };

    let answer = advisories_at(repo.path(), &ask, PUBLISHED).expect("answer");

    assert_eq!(answer.advisories.feed.status, FeedStatus::DigestMismatch);
    assert_eq!(state(&answer, "serde"), PackageState::NotEvaluated);
}

#[test]
fn a_malformed_digest_is_a_refusal_naming_the_argument() {
    let repo = repository();
    let ask = AdvisoriesInput {
        expected_digest: Some("not-a-digest".into()),
        ..input(Some("feed.json"))
    };

    let error = advisories_at(repo.path(), &ask, PUBLISHED).expect_err("refused");

    assert!(error.contains("expected_digest"), "{error}");
}

#[test]
fn the_window_is_judged_against_the_clock_the_goal_is_given() {
    let repo = repository();
    let ask = AdvisoriesInput {
        max_age_days: Some(30),
        ..input(Some("feed.json"))
    };

    let fresh = advisories_at(repo.path(), &ask, PUBLISHED + 86_400).expect("answer");
    let stale = advisories_at(repo.path(), &ask, PUBLISHED + 90 * 86_400).expect("answer");

    assert_eq!(fresh.advisories.feed.freshness, Freshness::Fresh);
    assert_eq!(stale.advisories.feed.freshness, Freshness::Stale);
}

#[test]
fn an_oversized_feed_file_is_read_only_up_to_the_limit_and_refused() {
    let repo = repository();
    let limits = FeedLimits::default();
    let big = vec![b' '; limits.max_bytes + 10];
    std::fs::write(repo.path().join("big.json"), &big).expect("big");

    let answer = advisories_at(repo.path(), &input(Some("big.json")), PUBLISHED).expect("answer");

    assert_eq!(answer.advisories.feed.status, FeedStatus::TooLarge);
    assert_eq!(state(&answer, "serde"), PackageState::NotEvaluated);
}

#[test]
fn the_same_inputs_give_the_same_answer() {
    let repo = repository();

    let first = advisories_at(repo.path(), &input(Some("feed.json")), PUBLISHED).expect("first");
    let second = advisories_at(repo.path(), &input(Some("feed.json")), PUBLISHED).expect("second");

    assert_eq!(
        serde_json::to_value(&first).expect("first"),
        serde_json::to_value(&second).expect("second")
    );
}

#[test]
fn the_rendering_names_the_feed_digest_and_the_state_of_each_package() {
    let repo = repository();
    let answer = advisories_at(repo.path(), &input(Some("feed.json")), PUBLISHED).expect("answer");
    let value = serde_json::to_value(&answer).expect("value");

    let text = render(&value);

    assert!(text.contains(&digest_of(FEED)), "{text}");
    assert!(text.contains("serde 1.0.100 Affected"), "{text}");
    assert!(text.contains("RUSTSEC-2099-0001"), "{text}");
}
