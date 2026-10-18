//! H1.2 (delivery-first-radical, package integration) — trait method
//! that surfaces the typed `BuildReport` on every strategy.
//!
//! Claim: a caller holding a `&dyn GraphStrategy` can ask for the
//! full graph AND its coverage status in one call. The seven CLI
//! commands in `interface/cli/commands.rs` already do this against
//! the concrete type `FullGraphStrategy::build_full_graph_report`
//! (line 1131, 1185, 1233, 1287, 1381, 1518, 1572 — added under
//! EXT-02 "honor the same Partial/Failed semantics as the MCP path").
//! Until this cycle the typed method was an INHERENT method on each
//! strategy, not a TRAIT method. A `&dyn GraphStrategy` (the shape
//! a generic dispatcher would have) had no access to it.
//!
//! H1.2 test strategy: pin the trait dispatch and the typed return.
//! For strategies that already implement `_report` (FullGraphStrategy,
//! PerFileStrategy), the report's `BuildStatus` is the typed one
//! from the inherent (not the `Complete` fallback). For strategies
//! that don't (LightweightStrategy, OnDemandStrategy), the trait
//! default synthesises a `Complete` report. The MCP/SDK dispatcher
//! can now consume `BuildReport` uniformly.

use cognicode_core::infrastructure::graph::per_file_graph::{
    BuildStatus, FileDisposition, WalkClassification, classify_walk_extension,
};
use cognicode_core::infrastructure::graph::{
    FullGraphStrategy, GraphStrategy, LightweightStrategy, PerFileStrategy,
};

fn rust_fixture_with_kt(dir: &std::path::Path) {
    // Mirrors the H0.* fixture: two Rust modules (≥ 2 symbols so
    // graph.full has content), plus a `.kt` file to force a
    // KnownUnsupportedGap, plus a `README.md` to force a
    // LegitExclusion. Together they exercise the three
    // H1.1 dispositions at once: Parsed (the .rs files),
    // UnsupportedRelevant (the .kt), ExcludedByPolicy (the README).
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("main.rs"),
        "mod alpha;\nmod beta;\nfn main() { alpha::helper(); beta::helper(); }\n",
    )
    .unwrap();
    std::fs::write(dir.join("src").join("alpha.rs"), "pub fn helper() -> u32 { 1 }\n").unwrap();
    std::fs::write(dir.join("src").join("beta.rs"), "pub fn helper() -> u32 { 2 }\n").unwrap();
    std::fs::write(dir.join("src").join("Other.kt"), "class Other { fun x() = 1 }\n").unwrap();
    std::fs::write(dir.join("README.md"), "# Fixture\n\nRust + .kt corpus for H1.2.\n").unwrap();
}

fn tempdir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "h12-strategies-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn full_graph_strategy_via_trait_dyn_returns_typed_build_report() {
    // The whole point of H1.2: a `&dyn GraphStrategy` (the shape a
    // generic dispatcher has) can ask for the typed report, not
    // just a CallGraph. If the trait method is missing, this fails
    // to compile.
    let tmp = tempdir();
    rust_fixture_with_kt(&tmp);
    let strategy: Box<dyn GraphStrategy> = Box::new(FullGraphStrategy::new());
    let report = strategy
        .build_full_graph_report(&tmp)
        .expect("build_full_graph_report succeeds");
    // The .kt file MUST surface as Partial; the FullGraphStrategy
    // inherent already does this — the trait method must NOT lose
    // it on the way out. (If it does, the propagation regressed
    // and the H1.1 invariant is broken at the seam.)
    match &report.status {
        BuildStatus::Partial { skipped } => {
            let kt_skips: Vec<&str> = skipped
                .iter()
                .filter(|s| s.path.ends_with("Other.kt"))
                .map(|s| s.path.as_str())
                .collect();
            assert!(
                !kt_skips.is_empty(),
                "FullGraphStrategy via &dyn must surface .kt as a Partial skip (H1.1 K0 honest gap), got {skipped:?}"
            );
        }
        BuildStatus::Complete => panic!(
            "FullGraphStrategy via &dyn returned Complete despite a .kt file in the corpus; the \
             H1.1 K0 gap (KnownUnsupportedGap) was lost on the way to the trait method. \
             status={:?}",
            report.status
        ),
    }
}

#[test]
fn per_file_strategy_via_trait_dyn_returns_typed_build_report() {
    // Same property for PerFileStrategy: the inherent already
    // classifies via WalkClassification and surfaces .kt as
    // KnownUnsupportedGap. The trait method must preserve it.
    let tmp = tempdir();
    rust_fixture_with_kt(&tmp);
    let strategy: Box<dyn GraphStrategy> = Box::new(PerFileStrategy::new());
    let report = strategy
        .build_full_graph_report(&tmp)
        .expect("build_full_graph_report succeeds");
    match &report.status {
        BuildStatus::Partial { skipped } => {
            assert!(
                skipped.iter().any(|s| s.path.ends_with("Other.kt")),
                "PerFileStrategy via &dyn must surface .kt as a Partial skip, got {skipped:?}"
            );
        }
        BuildStatus::Complete => panic!(
            "PerFileStrategy via &dyn returned Complete despite a .kt file; the K0 gap was lost. \
             status={:?}",
            report.status
        ),
    }
}

#[test]
fn full_graph_strategy_classifier_routes_kt_to_unsupported_relevant() {
    // Pin the H1.1 invariant at the strategy boundary: the .kt
    // file lands in skipped with a reason that the H1.1 envelope
    // projects to UnsupportedRelevant, NOT ReadFailed (the read
    // succeeded) and NOT ExcludedByPolicy (it's a real source).
    let tmp = tempdir();
    rust_fixture_with_kt(&tmp);
    let strategy: Box<dyn GraphStrategy> = Box::new(FullGraphStrategy::new());
    let report = strategy
        .build_full_graph_report(&tmp)
        .expect("build_full_graph_report succeeds");
    let kt_skip = match &report.status {
        BuildStatus::Partial { skipped } => skipped
            .iter()
            .find(|s| s.path.ends_with("Other.kt"))
            .expect("the .kt file must be in skipped"),
        BuildStatus::Complete => panic!("expected Partial, got Complete"),
    };
    let disp = FileDisposition::from_skipped(kt_skip);
    assert!(
        matches!(disp, FileDisposition::UnsupportedRelevant { .. }),
        "the .kt file must project to UnsupportedRelevant, got {disp:?}"
    );
    assert!(
        !matches!(disp, FileDisposition::ReadFailed { .. }),
        "UnsupportedRelevant and ReadFailed are distinct: a .kt read succeeds"
    );
    assert!(
        !matches!(disp, FileDisposition::ExcludedByPolicy { .. }),
        "UnsupportedRelevant and ExcludedByPolicy are distinct: a .kt is a relevant source"
    );
}

#[test]
fn readme_in_corpus_root_does_not_become_a_skip() {
    // README.md is a legit exclusion (WalkClassification::LegitExclusion),
    // not a gap. It MUST NOT appear in skipped, and a Complete
    // corpus (no .kt) must end with BuildStatus::Complete. AC-01
    // §Invariantes: 'archivo fuente no soportado sí; README/assets
    // no afecta completitud.'
    let tmp = tempdir();
    std::fs::create_dir_all(tmp.join("src")).unwrap();
    std::fs::write(
        tmp.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(tmp.join("src").join("lib.rs"), "pub fn x() -> u32 { 0 }\n").unwrap();
    std::fs::write(tmp.join("README.md"), "# Fixture\n\nPure Rust corpus.\n").unwrap();
    let strategy: Box<dyn GraphStrategy> = Box::new(FullGraphStrategy::new());
    let report = strategy.build_full_graph_report(&tmp).expect("ok");
    assert_eq!(
        report.status,
        BuildStatus::Complete,
        "a pure-Rust corpus with a README must be Complete (the README is a legit exclusion, not a gap)"
    );
    // Belt and suspenders: the WalkClassification classifier must
    // route .md to LegitExclusion, never to KnownUnsupportedGap.
    let class = classify_walk_extension(Some("md"));
    assert_eq!(class, WalkClassification::LegitExclusion);
    let disp = FileDisposition::from_walk_classification(class, Some("md"));
    assert!(matches!(disp, FileDisposition::ExcludedByPolicy { .. }));
}

#[test]
fn trait_method_default_for_strategies_without_report_synthesises_complete() {
    // LightweightStrategy and OnDemandStrategy do not implement
    // `_report` (they don't walk + parse). The trait method's
    // default calls the existing `build_full_graph` and synthesises
    // a Complete report. This pins the "doesn't lie about coverage"
    // contract: a strategy that doesn't report must NOT pretend
    // to be Partial (the truth is "we didn't check", which the
    // H1.1 envelope projects to Complete at the strategy level,
    // with the annotation that the strategy didn't measure).
    //
    // We test the default by instantiating a stub trait object
    // that doesn't override `build_full_graph_report`. The simplest
    // path: just use the trait dispatch on a concrete type that
    // doesn't override (LightweightStrategy).
    let tmp = tempdir();
    let strategy: Box<dyn GraphStrategy> = Box::new(LightweightStrategy::new());
    let report = strategy
        .build_full_graph_report(&tmp)
        .expect("lightweight strategy build_full_graph_report default");
    // The default returns Ok(BuildStatus::Complete { .. }) because
    // the underlying build_full_graph returns Ok(CallGraph::new()).
    // The default MUST NOT synthesise Partial — that would be a
    // lie.
    assert_eq!(
        report.status,
        BuildStatus::Complete,
        "the trait default must report Complete for strategies that don't measure coverage"
    );
    assert!(
        !matches!(report.status, BuildStatus::Partial { .. }),
        "the trait default must not synthesise Partial — that's a lie about coverage"
    );
}
