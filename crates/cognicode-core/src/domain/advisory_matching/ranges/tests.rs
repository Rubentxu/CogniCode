use super::*;
use crate::domain::advisory_matching::record::AdvisoryRange;
use crate::domain::advisory_matching::record::AffectedPackage;

fn events(list: &[(&str, &str)]) -> Vec<RangeEvent> {
    list.iter()
        .map(|(kind, version)| {
            let version = version.to_string();
            match *kind {
                "introduced" => RangeEvent::Introduced(version),
                "fixed" => RangeEvent::Fixed(version),
                "last_affected" => RangeEvent::LastAffected(version),
                _ => RangeEvent::Limit(version),
            }
        })
        .collect()
}

fn range(kind: RangeKind, list: &[(&str, &str)]) -> AdvisoryRange {
    AdvisoryRange {
        kind,
        events: Some(events(list)),
    }
}

fn entry(versions: &[&str], ranges: Vec<AdvisoryRange>) -> AffectedEntry {
    AffectedEntry {
        package: AffectedPackage::default(),
        versions: versions.iter().map(|v| v.to_string()).collect(),
        ranges,
    }
}

fn semver(list: &[(&str, &str)]) -> AffectedEntry {
    entry(&[], vec![range(RangeKind::Semver, list)])
}

#[test]
fn a_version_before_the_fix_is_affected_and_the_fix_itself_is_not() {
    let e = semver(&[("introduced", "0"), ("fixed", "1.2.3")]);
    assert_eq!(contains(&e, "0.0.1"), Containment::Affected);
    assert_eq!(contains(&e, "1.2.2"), Containment::Affected);
    assert_eq!(contains(&e, "1.2.3"), Containment::NotAffected);
    assert_eq!(contains(&e, "2.0.0"), Containment::NotAffected);
}

#[test]
fn a_version_before_the_introduction_is_not_affected() {
    let e = semver(&[("introduced", "1.5.0"), ("fixed", "1.7.0")]);
    assert_eq!(contains(&e, "1.4.9"), Containment::NotAffected);
    assert_eq!(contains(&e, "1.5.0"), Containment::Affected);
    assert_eq!(contains(&e, "1.6.9"), Containment::Affected);
    assert_eq!(contains(&e, "1.7.0"), Containment::NotAffected);
}

#[test]
fn last_affected_is_inclusive() {
    let e = semver(&[("introduced", "1.0.0"), ("last_affected", "1.4.0")]);
    assert_eq!(contains(&e, "1.4.0"), Containment::Affected);
    assert_eq!(contains(&e, "1.4.1"), Containment::NotAffected);
}

#[test]
fn an_open_range_covers_every_later_version() {
    let e = semver(&[("introduced", "2.0.0")]);
    assert_eq!(contains(&e, "1.9.9"), Containment::NotAffected);
    assert_eq!(contains(&e, "2.0.0"), Containment::Affected);
    assert_eq!(contains(&e, "99.0.0"), Containment::Affected);
}

#[test]
fn a_limit_closes_an_open_range() {
    let e = semver(&[("introduced", "1.0.0"), ("limit", "3.0.0")]);
    assert_eq!(contains(&e, "2.9.9"), Containment::Affected);
    assert_eq!(contains(&e, "3.0.0"), Containment::NotAffected);
}

#[test]
fn several_intervals_in_one_range_are_each_evaluated() {
    let e = semver(&[
        ("introduced", "0"),
        ("fixed", "1.2.0"),
        ("introduced", "1.5.0"),
        ("fixed", "1.5.4"),
    ]);
    assert_eq!(contains(&e, "1.1.0"), Containment::Affected);
    assert_eq!(contains(&e, "1.3.0"), Containment::NotAffected);
    assert_eq!(contains(&e, "1.5.2"), Containment::Affected);
    assert_eq!(contains(&e, "1.5.4"), Containment::NotAffected);
}

#[test]
fn events_out_of_order_are_ordered_by_version() {
    let e = semver(&[("fixed", "1.2.3"), ("introduced", "0")]);
    assert_eq!(contains(&e, "1.0.0"), Containment::Affected);
    assert_eq!(contains(&e, "1.2.3"), Containment::NotAffected);
}

#[test]
fn a_pre_release_sorts_before_its_release() {
    let fixed = semver(&[("introduced", "0"), ("fixed", "1.2.0")]);
    assert_eq!(
        contains(&fixed, "1.2.0-rc.1"),
        Containment::Affected,
        "a pre-release of the fixed version is older than the fix"
    );
    let introduced = semver(&[("introduced", "1.0.0"), ("fixed", "2.0.0")]);
    assert_eq!(
        contains(&introduced, "1.0.0-beta.2"),
        Containment::NotAffected,
        "a pre-release of the introduced version is older than the introduction"
    );
}

#[test]
fn pre_release_identifiers_follow_semver_precedence() {
    let e = semver(&[("introduced", "1.0.0-alpha.2"), ("fixed", "1.0.0-beta")]);
    assert_eq!(contains(&e, "1.0.0-alpha.1"), Containment::NotAffected);
    assert_eq!(contains(&e, "1.0.0-alpha.10"), Containment::Affected);
    assert_eq!(contains(&e, "1.0.0-alpha.beta"), Containment::Affected);
    assert_eq!(contains(&e, "1.0.0-beta"), Containment::NotAffected);
}

#[test]
fn build_metadata_never_takes_part() {
    let e = semver(&[("introduced", "0"), ("fixed", "1.2.3")]);
    assert_eq!(contains(&e, "1.2.3+build.5"), Containment::NotAffected);
    assert_eq!(contains(&e, "1.2.2+build.5"), Containment::Affected);
}

#[test]
fn ecosystem_ranges_for_crates_are_read_with_the_same_ordering() {
    let e = entry(
        &[],
        vec![range(
            RangeKind::Ecosystem,
            &[("introduced", "0"), ("fixed", "0.4.0")],
        )],
    );
    assert_eq!(contains(&e, "0.3.9"), Containment::Affected);
    assert_eq!(contains(&e, "0.4.0"), Containment::NotAffected);
}

#[test]
fn a_listed_version_is_affected_whatever_the_ranges_say() {
    let e = entry(&["1.0.0", "1.0.1+meta"], vec![]);
    assert_eq!(contains(&e, "1.0.0"), Containment::Affected);
    assert_eq!(contains(&e, "1.0.1"), Containment::Affected);
    assert_eq!(contains(&e, "1.0.2"), Containment::NotAffected);
}

#[test]
fn a_git_range_alone_cannot_be_decided() {
    let e = entry(
        &[],
        vec![range(RangeKind::Git, &[("introduced", "abc123")])],
    );
    assert_eq!(contains(&e, "1.0.0"), Containment::Unknown);
}

#[test]
fn a_git_range_next_to_a_version_list_does_not_block_the_list() {
    let e = entry(
        &["1.0.0"],
        vec![range(RangeKind::Git, &[("introduced", "abc123")])],
    );
    assert_eq!(contains(&e, "1.0.0"), Containment::Affected);
    assert_eq!(contains(&e, "1.0.1"), Containment::NotAffected);
}

#[test]
fn an_event_that_is_not_a_version_makes_the_range_unknown() {
    let e = semver(&[("introduced", "0"), ("fixed", "not-a-version")]);
    assert_eq!(contains(&e, "1.0.0"), Containment::Unknown);
}

#[test]
fn a_locked_version_that_is_not_semver_makes_the_ranges_unknown() {
    let e = semver(&[("introduced", "0"), ("fixed", "1.0.0")]);
    assert_eq!(contains(&e, "weird"), Containment::Unknown);
}

#[test]
fn events_that_are_not_in_the_schema_shape_are_unknown() {
    let e = entry(
        &[],
        vec![AdvisoryRange {
            kind: RangeKind::Semver,
            events: None,
        }],
    );
    assert_eq!(contains(&e, "1.0.0"), Containment::Unknown);
}

#[test]
fn an_unknown_range_kind_is_unknown() {
    let e = entry(
        &[],
        vec![range(
            RangeKind::Other("CALVER".into()),
            &[("introduced", "0")],
        )],
    );
    assert_eq!(contains(&e, "1.0.0"), Containment::Unknown);
}

#[test]
fn an_entry_with_nothing_to_evaluate_is_unknown() {
    assert_eq!(contains(&entry(&[], vec![]), "1.0.0"), Containment::Unknown);
}

#[test]
fn one_decidable_range_that_matches_wins_over_an_undecidable_one() {
    let e = entry(
        &[],
        vec![
            range(
                RangeKind::Semver,
                &[("introduced", "0"), ("fixed", "2.0.0")],
            ),
            range(RangeKind::Other("X".into()), &[("introduced", "0")]),
        ],
    );
    assert_eq!(contains(&e, "1.0.0"), Containment::Affected);
    assert_eq!(
        contains(&e, "3.0.0"),
        Containment::Unknown,
        "a clear decidable range does not settle an undecidable sibling"
    );
}

#[test]
fn a_huge_number_of_events_is_evaluated_without_blowing_up() {
    let list: Vec<(String, String)> = (0..256)
        .flat_map(|i| {
            [
                ("introduced".to_string(), format!("{i}.0.0")),
                ("fixed".to_string(), format!("{i}.5.0")),
            ]
        })
        .collect();
    let borrowed: Vec<(&str, &str)> = list.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let e = semver(&borrowed);
    assert_eq!(contains(&e, "100.2.0"), Containment::Affected);
    assert_eq!(contains(&e, "100.7.0"), Containment::NotAffected);
}
