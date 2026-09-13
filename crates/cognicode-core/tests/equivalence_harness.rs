#![cfg(feature = "evidence-kernel")]
//! E37 M2 equivalence harness (design D7) — structural equivalence between
//! the LEGACY graph oracle and the FACT-DERIVED projection over the golden
//! fixtures.
//!
//! Declared contract (spec `projection-equivalence-harness`, requirement
//! "Declared equivalence contract", stated BEFORE any comparison):
//!
//! - minimum structural equivalence threshold:
//!   [`harness::EQUIVALENCE_THRESHOLD`] = 0.99;
//! - comparison method: per-fixture Jaccard over normalized (stably sorted)
//!   node and edge multisets, where an edge multiset item is
//!   `(source, target, dependency-type, confidence-bits)`;
//! - quarantine: [`harness::KNOWN_UNSTABLE_SURFACES`] excludes declared
//!   unstable surfaces from scoring but NOT from reporting;
//! - the run fails naming any non-quarantined fixture below the threshold.
//!
//! The bridge path is reachable only behind the off-by-default
//! `evidence-kernel` feature (spec "No silent cutover"); the default-path
//! byte-stability gate remains e36's `just lsi-fixtures check`.

#[path = "equivalence_harness/harness.rs"]
mod harness;

use std::path::Path;

use harness::{
    EQUIVALENCE_THRESHOLD, FixtureReport, KNOWN_UNSTABLE_SURFACES, QUARANTINED_FIXTURES,
    SCORED_FIXTURES,
};

/// Locates the golden fixture root (`sandbox/fixtures`) from the crate.
fn fixtures_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures")
        .canonicalize()
        .expect("sandbox/fixtures resolves")
}

/// Spec scenario "All fixtures meet the threshold": every non-quarantined
/// fixture scores at or above the declared threshold AND the run passes
/// with a per-fixture report.
#[test]
fn all_fixtures_meet_the_threshold() {
    let run = harness::run(&fixtures_root());

    // The identity-convention pin must hold (deterministic fact identity).
    assert!(
        run.digest_ok,
        "entity-identity convention digest {} does not match the pinned baseline {}",
        run.digest,
        harness::PINNED_IDENTITY_DIGEST
    );

    println!("== E37 equivalence harness ==");
    println!("threshold: {EQUIVALENCE_THRESHOLD}");
    for report in &run.reports {
        println!("{}", report.describe());
    }

    for report in run.reports.iter().filter(|r| !r.quarantined) {
        assert!(
            report.min_score() >= EQUIVALENCE_THRESHOLD,
            "fixture {} below threshold: {}",
            report.name,
            report.describe()
        );
    }
    assert!(
        run.passed,
        "harness run failed: {:?}",
        run.failures.join(" | ")
    );
    // Both scored fixtures produced a real comparison.
    for name in SCORED_FIXTURES {
        let report = run
            .reports
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("missing report for scored fixture {name}"));
        assert!(
            report.legacy_nodes > 0,
            "{name}: legacy oracle produced no nodes"
        );
        assert!(report.fact_nodes > 0, "{name}: fact path produced no nodes");
        // E38.1 CP-2: the symbol-kind multiset joins the declared contract
        // — the legacy `Symbol` reconstruction and the fact-side
        // `kind=<SerdeName>` codec agree on every scored fixture.
        assert!(
            report.kind_score >= EQUIVALENCE_THRESHOLD,
            "{name}: kind multiset diverges below the threshold: {}",
            report.describe()
        );
    }
}

/// Spec scenario "Fixture below threshold fails the run": one
/// non-quarantined fixture below the threshold fails the run AND the
/// report names the fixture and its score.
#[test]
fn fixture_below_threshold_fails_the_run() {
    let reports = vec![FixtureReport {
        name: "python-hello".to_string(),
        quarantined: false,
        node_score: 1.0,
        edge_score: 0.5,
        kind_score: 1.0,
        legacy_nodes: 7,
        legacy_edges: 3,
        fact_nodes: 7,
        fact_edges: 2,
        fact_unresolved: 1,
    }];
    let (passed, failures) = harness::evaluate_reports(&reports, true);

    assert!(!passed, "a below-threshold fixture must fail the run");
    assert!(
        failures
            .iter()
            .any(|f| f.contains("python-hello") && f.contains("0.5")),
        "failure must name the fixture and its score: {:?}",
        failures
    );
}

/// Spec scenario "Quarantined divergence is excluded and reported": a
/// known-unstable surface is excluded from the equivalence score AND is
/// listed as quarantined in the report.
#[test]
fn quarantined_divergence_is_excluded_and_reported() {
    let run = harness::run(&fixtures_root());

    for name in QUARANTINED_FIXTURES {
        let report = run
            .reports
            .iter()
            .find(|r| r.name == *name)
            .unwrap_or_else(|| panic!("quarantined fixture {name} must still be reported"));
        assert!(report.quarantined, "{name} must be flagged quarantined");
        println!("quarantined: {}", report.describe());
    }

    // Exclusion: a quarantined report scoring zero does NOT fail the run…
    let quarantined = FixtureReport {
        name: KNOWN_UNSTABLE_SURFACES[0].to_string(),
        quarantined: true,
        node_score: 0.0,
        edge_score: 0.0,
        kind_score: 0.0,
        legacy_nodes: 40,
        legacy_edges: 4,
        fact_nodes: 12,
        fact_edges: 1,
        fact_unresolved: 3,
    };
    let reports = vec![quarantined];
    let (passed, failures) = harness::evaluate_reports(&reports, true);
    assert!(
        passed,
        "quarantined divergence must not fail the run: {failures:?}"
    );

    // …but the run() report carries its measured scores for honesty.
    let measured = run
        .reports
        .iter()
        .find(|r| r.name == KNOWN_UNSTABLE_SURFACES[0])
        .expect("quarantined fixture measured and reported");
    assert!(
        measured.legacy_nodes > 0,
        "quarantined fixture is measured, not skipped"
    );
}

/// Spec scenario "Unquarantined divergence fails": a divergence on a
/// surface NOT listed in the quarantine fails the run and the diverging
/// surface is reported.
#[test]
fn unquarantined_divergence_fails() {
    let reports = vec![
        FixtureReport {
            name: "rust-hello".to_string(),
            quarantined: false,
            node_score: 1.0,
            edge_score: 1.0,
            kind_score: 1.0,
            legacy_nodes: 5,
            legacy_edges: 2,
            fact_nodes: 5,
            fact_edges: 2,
            fact_unresolved: 0,
        },
        FixtureReport {
            name: "unquarantined-surface".to_string(),
            quarantined: false,
            node_score: 0.75,
            edge_score: 1.0,
            kind_score: 1.0,
            legacy_nodes: 8,
            legacy_edges: 2,
            fact_nodes: 6,
            fact_edges: 2,
            fact_unresolved: 0,
        },
    ];
    let (passed, failures) = harness::evaluate_reports(&reports, true);

    assert!(!passed, "an unquarantined divergence must fail the run");
    assert!(
        failures
            .iter()
            .any(|f| f.contains("unquarantined-surface") && f.contains("0.75")),
        "the diverging surface must be named: {failures:?}"
    );
}

/// E38.1 CP-6 self-checks: the pinned convention text actually states the
/// declared line rule, grammar, and typed reference — cheap guards against
/// an accidental truncated or stale convention edit (the digest alone
/// cannot distinguish a deliberate re-pin from an accidental one).
#[test]
fn identity_convention_states_the_declared_rules() {
    assert!(
        harness::IDENTITY_CONVENTION.contains("with a 1-BASED line"),
        "the convention text must state the 1-based line rule"
    );
    assert!(
        harness::IDENTITY_CONVENTION.contains("'{file}:{name}:{line}'"),
        "the convention text must state the identity grammar"
    );
    assert!(
        harness::IDENTITY_CONVENTION.contains("SymbolFqn"),
        "the convention text must reference the typed grammar SymbolFqn"
    );
    harness::verify_identity_pin(&harness::identity_convention_digest())
        .expect("the pinned digest must match the current convention text");
}

/// Requirement "Deterministic fact identity", scenario "Convention change
/// requires explicit re-baseline": a changed entity-identity convention
/// fails the run until the affected goldens are explicitly re-pinned.
#[test]
fn convention_change_requires_explicit_re_baseline() {
    // The current convention matches the pin…
    let current = harness::identity_convention_digest();
    harness::verify_identity_pin(&current)
        .expect("current convention must match the pinned baseline");

    // …and ANY convention change fails with a re-baseline instruction.
    let changed =
        harness::identity_convention_digest_for("e37 entity-identity convention v2: hashed FQNs");
    let err = harness::verify_identity_pin(&changed)
        .expect_err("a changed convention must require an explicit re-baseline");
    assert!(
        err.contains("re-baseline") || err.contains("re-pin"),
        "failure must instruct re-baselining: {err}"
    );
}

/// Umbrella R2 (rebuildability) specialized to the CallGraph projection
/// (design D7): rebuilding the projection from RE-READ facts (fresh store,
/// fresh commit) equals the prior projection EXACTLY as sorted multisets.
#[test]
fn rebuild_from_reread_facts_equals_prior_projection() {
    let root = fixtures_root().join("rust-hello");
    let facts = harness::fixture_facts(&root);

    let first = harness::fact_projection_from_committed(&facts);
    let second = harness::fact_projection_from_committed(&facts);

    assert_eq!(
        harness::node_multiset(&first),
        harness::node_multiset(&second),
        "rebuilt node multiset must equal the prior one exactly"
    );
    assert_eq!(
        harness::edge_multiset(&first),
        harness::edge_multiset(&second),
        "rebuilt edge multiset must equal the prior one exactly"
    );
}
