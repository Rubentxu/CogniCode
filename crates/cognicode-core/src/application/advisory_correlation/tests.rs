use super::*;
use crate::domain::dependency_inventory::{SourceFile, UnitInput, inventory_unit};
use crate::domain::scm::RevisionHandle;

const MANIFEST: &str = "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\nlocal = { path = \"local\" }\n";

fn lock() -> String {
    format!(
        "version = 3\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\"local\", \"serde\"]\n\n[[package]]\nname = \"local\"\nversion = \"0.2.0\"\n\n[[package]]\nname = \"serde\"\nversion = \"1.0.100\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
        "a".repeat(64)
    )
}

fn inventory_answer(with_lock: bool) -> DependencyInventoryAnswer {
    let file = |path: &str, text: String| SourceFile {
        path: path.into(),
        bytes: text.into_bytes(),
    };
    let unit = inventory_unit(&UnitInput {
        root: String::new(),
        lock: with_lock.then(|| file("Cargo.lock", lock())),
        manifests: vec![file("Cargo.toml", MANIFEST.to_string())],
    });
    let handle = RevisionHandle::new("rev-1").expect("handle");
    DependencyInventoryAnswer {
        revision: RevisionAnchor {
            requested_ref: "HEAD".into(),
            revision: handle.as_str().into(),
            content_digest: "c".repeat(64),
        },
        coverage: SourceCoverage {
            cargo_files_tracked: 2,
            cargo_files_read: 2,
            skipped: Vec::new(),
            complete: true,
        },
        ecosystems: vec!["cargo".into()],
        units: vec![unit],
        dependents: None,
        comparison: None,
    }
}

const FEED: &str = r#"[
  {"id":"RUSTSEC-2099-0001","modified":"2099-01-10T00:00:00Z",
   "affected":[{"package":{"ecosystem":"crates.io","name":"serde"},
     "ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.0.150"}]}]}]},
  {"id":"PYSEC-1","modified":"2099-01-10T00:00:00Z",
   "affected":[{"package":{"ecosystem":"PyPI","name":"serde"}}]}
]"#;

/// 2099-01-10T00:00:00Z, the `modified` of the records of `FEED`.
const PUBLISHED: i64 = 4071686400;

fn query(feed: FeedSupply) -> AdvisoryQuery {
    AdvisoryQuery {
        revision: "HEAD".into(),
        path: None,
        feed,
        expected_digest: None,
        max_age_days: None,
        now_unix: PUBLISHED + DAY,
    }
}

fn supply(text: &str) -> FeedSupply {
    FeedSupply::Bytes {
        origin: "feed.json".into(),
        bytes: text.as_bytes().to_vec(),
    }
}

fn run(query: &AdvisoryQuery, with_lock: bool) -> AdvisoryAnswer {
    correlate_inventory(
        &inventory_answer(with_lock),
        evaluate_feed(query, &FeedLimits::default()),
    )
}

fn state_of(answer: &AdvisoryAnswer, name: &str) -> PackageState {
    answer.units[0]
        .packages
        .iter()
        .find(|package| package.name == name)
        .unwrap_or_else(|| panic!("{name} is in the unit"))
        .state
}

#[test]
fn an_affected_package_is_reported_and_workspace_members_are_not_evaluated() {
    let answer = run(&query(supply(FEED)), true);

    assert_eq!(answer.evaluation, AdvisoryEvaluation::Evaluated);
    assert_eq!(state_of(&answer, "serde"), PackageState::Affected);
    assert_eq!(state_of(&answer, "local"), PackageState::NotEvaluated);
    assert_eq!(state_of(&answer, "app"), PackageState::NotEvaluated);
    assert_eq!(answer.totals.affected, 1);
    assert_eq!(answer.totals.total, 3);
}

#[test]
fn the_result_carries_the_feed_digest_and_the_lock_and_resolved_digests() {
    let answer = run(&query(supply(FEED)), true);

    let feed_digest = ContentDigest::from_bytes(FEED.as_bytes());
    assert_eq!(answer.feed.digest.as_deref(), Some(feed_digest.as_hex()));
    let unit = &answer.units[0];
    assert_eq!(unit.feed_digest.as_deref(), Some(feed_digest.as_hex()));
    assert_eq!(
        unit.lockfile.as_ref().expect("lock").digest,
        ContentDigest::from_bytes(lock().as_bytes()).as_hex()
    );
    assert!(!unit.resolved_digest.is_empty());
    assert_eq!(answer.revision.revision, "rev-1");
}

#[test]
fn the_same_inputs_give_the_same_answer() {
    assert_eq!(
        run(&query(supply(FEED)), true),
        run(&query(supply(FEED)), true)
    );
}

#[test]
fn a_fixed_feed_that_does_not_cover_the_version_is_not_found_in_scope() {
    let feed = FEED.replace("1.0.150", "1.0.50");
    let answer = run(&query(supply(&feed)), true);
    assert_eq!(state_of(&answer, "serde"), PackageState::NotFoundInScope);
}

#[test]
fn an_absent_feed_evaluates_nothing_and_never_says_clean() {
    let answer = run(&query(FeedSupply::Absent), true);

    assert_eq!(answer.evaluation, AdvisoryEvaluation::NotEvaluated);
    assert_eq!(answer.feed.status, FeedStatus::Absent);
    assert_eq!(answer.totals.not_found_in_scope, 0);
    assert_eq!(answer.totals.affected, 0);
    assert_eq!(state_of(&answer, "serde"), PackageState::NotEvaluated);
    assert_eq!(answer.units[0].evaluation, AdvisoryEvaluation::NotEvaluated);
    assert!(
        answer.units[0]
            .reasons
            .contains(&AdvisoryReason::FeedUnavailable)
    );
}

#[test]
fn an_unreadable_malformed_or_oversized_feed_evaluates_nothing() {
    let unreadable = FeedSupply::Unreadable {
        reason: "no such file".into(),
    };
    let cases = [
        (unreadable, FeedStatus::Unreadable),
        (supply("{not json"), FeedStatus::Malformed),
        (supply("42"), FeedStatus::UnrecognisedShape),
    ];
    for (feed, status) in cases {
        let answer = run(&query(feed), true);
        assert_eq!(answer.feed.status, status);
        assert_eq!(
            answer.evaluation,
            AdvisoryEvaluation::NotEvaluated,
            "{status:?}"
        );
        assert_eq!(answer.totals.not_found_in_scope, 0, "{status:?}");
    }
    let tiny = FeedLimits {
        max_bytes: 3,
        ..FeedLimits::default()
    };
    let answer = correlate_inventory(
        &inventory_answer(true),
        evaluate_feed(&query(supply(FEED)), &tiny),
    );
    assert_eq!(answer.feed.status, FeedStatus::TooLarge);
    assert_eq!(answer.totals.not_found_in_scope, 0);
}

#[test]
fn a_feed_that_differs_from_the_pinned_digest_is_not_used() {
    let mut ask = query(supply(FEED));
    ask.expected_digest = Some(ContentDigest::from_bytes(b"something else"));

    let answer = run(&ask, true);

    assert_eq!(answer.feed.status, FeedStatus::DigestMismatch);
    assert!(answer.feed.pinned);
    assert_eq!(answer.evaluation, AdvisoryEvaluation::NotEvaluated);
    assert_eq!(state_of(&answer, "serde"), PackageState::NotEvaluated);
}

#[test]
fn a_feed_that_matches_the_pinned_digest_is_used() {
    let mut ask = query(supply(FEED));
    ask.expected_digest = Some(ContentDigest::from_bytes(FEED.as_bytes()));
    assert_eq!(run(&ask, true).feed.status, FeedStatus::Usable);
}

#[test]
fn a_stale_feed_keeps_its_matches_but_never_concludes_from_silence() {
    let feed = FEED.replace("1.0.150", "1.0.50");
    let mut ask = query(supply(&feed));
    ask.max_age_days = Some(30);
    ask.now_unix = PUBLISHED + 40 * DAY;

    let answer = run(&ask, true);

    assert_eq!(answer.feed.freshness, FeedFreshness::Stale);
    assert_eq!(state_of(&answer, "serde"), PackageState::Unknown);
    assert_eq!(answer.totals.not_found_in_scope, 0);
    assert!(answer.units[0].reasons.contains(&AdvisoryReason::FeedStale));

    let affected = FEED.replace("2099-01-10", "2098-01-10");
    let mut ask = query(supply(&affected));
    ask.max_age_days = Some(30);
    assert_eq!(
        state_of(&run(&ask, true), "serde"),
        PackageState::Affected,
        "a stale feed still reports what it matches"
    );
}

#[test]
fn a_feed_inside_the_window_is_fresh() {
    let mut ask = query(supply(FEED));
    ask.max_age_days = Some(30);
    ask.now_unix = PUBLISHED + 29 * DAY;
    assert_eq!(run(&ask, true).feed.freshness, FeedFreshness::Fresh);
}

#[test]
fn rejected_records_make_silence_inconclusive() {
    let feed = FEED.replace("1.0.150", "1.0.50").replace(
        "  {\"id\":\"PYSEC-1\"",
        "  {\"id\":\"BROKEN\",\"modified\":\"x\",\"affected\":[{\"package\":{\"ecosystem\":\"crates.io\",\"name\":\"z\"}}]},\n  {\"id\":\"PYSEC-1\"",
    );
    let answer = run(&query(supply(&feed)), true);

    assert_eq!(answer.feed.records_rejected, 1);
    assert_eq!(answer.feed.rejected[0].id.as_deref(), Some("BROKEN"));
    assert_eq!(state_of(&answer, "serde"), PackageState::Unknown);
    assert!(
        answer.units[0]
            .reasons
            .contains(&AdvisoryReason::FeedIncomplete)
    );
}

#[test]
fn a_feed_without_crates_io_advisories_cannot_speak_about_cargo() {
    let feed = r#"[{"id":"PYSEC-1","modified":"2099-01-10T00:00:00Z","affected":[{"package":{"ecosystem":"PyPI","name":"x"}}]}]"#;
    let answer = run(&query(supply(feed)), true);

    assert_eq!(state_of(&answer, "serde"), PackageState::NotEvaluated);
    assert_eq!(answer.totals.not_found_in_scope, 0);
    assert_eq!(answer.coverage.advisories_considered, 0);
    assert_eq!(answer.coverage.ecosystems_not_considered, vec!["PyPI"]);
}

#[test]
fn coverage_names_what_was_and_was_not_considered() {
    let answer = run(&query(supply(FEED)), true);

    assert_eq!(answer.coverage.ecosystems_considered, vec!["crates.io"]);
    assert_eq!(answer.coverage.ecosystems_not_considered, vec!["PyPI"]);
    assert_eq!(answer.coverage.advisories_considered, 1);
    assert_eq!(answer.feed.records_other_ecosystem, 1);
    assert!(answer.coverage.inventory.complete);
}

#[test]
fn a_unit_without_a_lockfile_is_not_evaluated() {
    let answer = run(&query(supply(FEED)), false);

    assert_eq!(answer.units[0].evaluation, AdvisoryEvaluation::NotEvaluated);
    assert!(
        answer.units[0]
            .reasons
            .contains(&AdvisoryReason::NoLockfile)
    );
    assert!(answer.units[0].packages.is_empty());
}

#[test]
fn an_inventory_with_no_units_is_not_evaluated() {
    let mut empty = inventory_answer(true);
    empty.units.clear();
    let answer = correlate_inventory(
        &empty,
        evaluate_feed(&query(supply(FEED)), &FeedLimits::default()),
    );
    assert_eq!(answer.evaluation, AdvisoryEvaluation::NotEvaluated);
}

#[test]
fn the_rejection_list_is_capped_but_the_total_is_not() {
    let broken: Vec<String> = (0..150)
        .map(|i| format!(r#"{{"id":"B-{i}","modified":"x","affected":[{{"package":{{"ecosystem":"crates.io","name":"z"}}}}]}}"#))
        .collect();
    let answer = run(&query(supply(&format!("[{}]", broken.join(",")))), true);
    assert_eq!(answer.feed.records_rejected, 150);
    assert_eq!(answer.feed.rejected.len(), 100);
}
