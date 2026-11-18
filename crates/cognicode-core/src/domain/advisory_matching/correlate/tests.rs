use super::*;
use crate::domain::advisory_matching::feed::{FeedLimits, parse_feed};

fn records(json: &str) -> Vec<AdvisoryRecord> {
    parse_feed(json.as_bytes(), &FeedLimits::default())
        .expect("feed")
        .records
}

fn subject(name: &str, version: &str) -> Subject {
    Subject {
        id: format!("{name}@{version}"),
        name: name.into(),
        version: version.into(),
        purl: Some(format!("pkg:cargo/{name}@{version}")),
        from_crates_io: true,
    }
}

const FEED: &str = r#"[
  {"id":"RUSTSEC-2024-0001","modified":"2024-05-01T00:00:00Z","aliases":["CVE-2024-1","GHSA-aaaa"],
   "summary":"buffer overflow",
   "affected":[{"package":{"ecosystem":"crates.io","name":"Foo_Bar"},
     "ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.2.3"}]}]}]},
  {"id":"GHSA-aaaa","modified":"2024-05-02T00:00:00Z","aliases":["RUSTSEC-2024-0001"],
   "affected":[{"package":{"ecosystem":"crates.io","name":"foo-bar"},
     "ranges":[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"1.2.3"},{"introduced":"2.0.0"},{"fixed":"2.0.5"}]}]}]},
  {"id":"RUSTSEC-2024-0002","modified":"2024-06-01T00:00:00Z",
   "affected":[{"package":{"ecosystem":"crates.io","name":"git-only"},
     "ranges":[{"type":"GIT","events":[{"introduced":"abc"}]}]}]}
]"#;

#[test]
fn an_affected_version_is_reported_with_its_advisory() {
    let out = correlate(
        &records(FEED),
        &[subject("foo-bar", "1.0.0")],
        Absence::Conclusive,
    );
    assert_eq!(out[0].state, PackageState::Affected);
    assert_eq!(out[0].advisories.len(), 1);
    assert_eq!(out[0].advisories[0].certainty, Certainty::Affected);
}

#[test]
fn a_record_and_its_alias_record_are_one_advisory() {
    let out = correlate(
        &records(FEED),
        &[subject("foo-bar", "1.0.0")],
        Absence::Conclusive,
    );
    let hit = &out[0].advisories[0];
    assert_eq!(hit.id, "GHSA-aaaa");
    assert_eq!(hit.aliases, vec!["CVE-2024-1", "RUSTSEC-2024-0001"]);
    assert_eq!(hit.fixed_in, vec!["1.2.3", "2.0.5"]);
    assert_eq!(hit.modified, "2024-05-02T00:00:00Z");
}

#[test]
fn names_are_compared_the_way_cargo_does() {
    let out = correlate(
        &records(FEED),
        &[subject("FOO_BAR", "1.0.0")],
        Absence::Conclusive,
    );
    assert_eq!(out[0].state, PackageState::Affected);
}

#[test]
fn a_fixed_version_is_not_found_in_scope() {
    let out = correlate(
        &records(FEED),
        &[subject("foo-bar", "1.2.3")],
        Absence::Conclusive,
    );
    assert_eq!(out[0].state, PackageState::NotFoundInScope);
    assert!(out[0].advisories.is_empty());
}

#[test]
fn an_unrelated_crate_is_not_found_in_scope() {
    let out = correlate(
        &records(FEED),
        &[subject("serde", "1.0.200")],
        Absence::Conclusive,
    );
    assert_eq!(out[0].state, PackageState::NotFoundInScope);
}

#[test]
fn an_inconclusive_feed_never_yields_not_found() {
    for reason in [AdvisoryReason::FeedIncomplete, AdvisoryReason::FeedStale] {
        let out = correlate(
            &records(FEED),
            &[subject("serde", "1.0.200")],
            Absence::Inconclusive(reason),
        );
        assert_eq!(out[0].state, PackageState::Unknown, "{reason:?}");
        assert_eq!(out[0].reasons, vec![reason]);
    }
}

#[test]
fn a_feed_without_the_ecosystem_leaves_packages_not_evaluated() {
    let out = correlate(
        &[],
        &[subject("serde", "1.0.200")],
        Absence::Inconclusive(AdvisoryReason::EcosystemNotCovered),
    );
    assert_eq!(out[0].state, PackageState::NotEvaluated);
}

#[test]
fn an_inconclusive_feed_still_reports_what_it_does_match() {
    let out = correlate(
        &records(FEED),
        &[subject("foo-bar", "1.0.0")],
        Absence::Inconclusive(AdvisoryReason::FeedStale),
    );
    assert_eq!(out[0].state, PackageState::Affected);
}

#[test]
fn an_undecidable_advisory_is_unknown_not_clean() {
    let out = correlate(
        &records(FEED),
        &[subject("git-only", "1.0.0")],
        Absence::Conclusive,
    );
    assert_eq!(out[0].state, PackageState::Unknown);
    assert_eq!(out[0].reasons, vec![AdvisoryReason::UndecidableRange]);
    assert_eq!(out[0].advisories[0].certainty, Certainty::Undecided);
}

#[test]
fn packages_outside_crates_io_are_not_evaluated() {
    let mut path = subject("foo-bar", "1.0.0");
    path.from_crates_io = false;
    path.purl = None;
    let out = correlate(&records(FEED), &[path], Absence::Conclusive);
    assert_eq!(out[0].state, PackageState::NotEvaluated);
    assert_eq!(out[0].reasons, vec![AdvisoryReason::SourceNotInEcosystem]);
}

#[test]
fn a_record_keyed_by_purl_matches_too() {
    let json = r#"[{"id":"X-1","modified":"2024-01-01T00:00:00Z",
      "affected":[{"package":{"purl":"pkg:cargo/bar"},"versions":["1.0.0"]}]}]"#;
    let out = correlate(
        &records(json),
        &[subject("bar", "1.0.0")],
        Absence::Conclusive,
    );
    assert_eq!(out[0].state, PackageState::Affected);
}

#[test]
fn results_keep_the_order_of_the_subjects() {
    let out = correlate(
        &records(FEED),
        &[subject("zzz", "1.0.0"), subject("foo-bar", "1.0.0")],
        Absence::Conclusive,
    );
    assert_eq!(out[0].name, "zzz");
    assert_eq!(out[1].name, "foo-bar");
}

#[test]
fn the_result_is_the_same_whatever_the_order_of_the_records() {
    let mut forward = records(FEED);
    let subjects = [subject("foo-bar", "1.0.0"), subject("git-only", "1.0.0")];
    let a = correlate(&forward, &subjects, Absence::Conclusive);
    forward.reverse();
    let b = correlate(&forward, &subjects, Absence::Conclusive);
    assert_eq!(a, b);
}
