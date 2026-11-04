//! E39 M4 conformance runner (design D6, ungated like the other LSI
//! harnesses): declared per-tier precision targets for Rust, TypeScript,
//! and Java (spec `provider-pipeline-conformance`).
//!
//! Declarations live in `sandbox/fixtures/lsi-providers/<language>/expected.json`
//! and are verified against the composite tier pipeline's observations —
//! no numeric thresholds, a contradiction fails the run naming the query,
//! the declared tier, and the observed tier.
//!
//! Availability gating: Rust/TS run with the LSP tier policy-disabled
//! (serverless-deterministic); Java probes the `jdtls` binary NAME on
//! `PATH` (a scan — no process is spawned by the probe) and executes the
//! matching branch: available → the declared S2 run; unavailable → the
//! declared-unavailable assertions, which must NOT fail.

#[path = "provider_conformance/harness.rs"]
mod harness;

use std::path::Path;
use std::time::Duration;

use cognicode_core::domain::traits::code_intelligence::{PrecisionTier, ProviderDiagnostic};
use cognicode_core::infrastructure::lsp::providers::composite::{CompositeProvider, TierPolicy};

use harness::{
    ExpectedManifest, ExpectedObservation, ExpectedOutcome, ExpectedQuery, JAVA_LANGUAGE,
    JAVA_SERVER_BINARY, Observation, RUST_LANGUAGE, TS_LANGUAGE,
};

/// The serverless pipeline: S2 policy-disabled (skipped without a
/// diagnostic or a spawn), S1 local resolver and S0 tree-sitter enabled.
fn serverless_provider(root: &Path) -> CompositeProvider {
    CompositeProvider::with_policy(
        root,
        TierPolicy {
            lsp: false,
            ..TierPolicy::all()
        },
    )
}

fn describe(observation: &Observation) -> String {
    format!(
        "{} -> {} {} (diagnostics: {})",
        observation.query,
        observation.outcome,
        observation.tier_text(),
        observation
            .diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" | ")
    )
}

/// Runs one serverless fixture cohort and asserts every declaration
/// matched. Shared by the Rust and TS scenarios.
async fn run_serverless(language: &str) {
    let manifest = harness::load_manifest(language);
    let root = harness::fixture_root(language);
    let provider = serverless_provider(&root);

    let observations = harness::observe_all(&provider, &root, &manifest).await;
    assert_eq!(
        observations.len(),
        manifest.queries.len(),
        "every declared query must be observed"
    );
    println!("== {language} provider conformance (serverless) ==");
    for observation in &observations {
        println!("{}", describe(observation));
    }

    let failures = harness::verify(&manifest, &observations);
    assert!(
        failures.is_empty(),
        "{language} conformance failed:\n{}",
        failures.join("\n")
    );

    // The serverless run must never have touched the LSP tier.
    for observation in &observations {
        assert!(
            observation.tier != Some(PrecisionTier::S2),
            "{}: the policy-disabled LSP tier must never serve",
            observation.query
        );
    }
}

/// Spec scenario "Matching declarations pass without servers" (Rust):
/// declared S0/S1 targets verify and pass with the LSP tier policy-off.
#[tokio::test]
async fn rust_matching_declarations_pass_without_servers() {
    run_serverless(RUST_LANGUAGE).await;
}

/// Spec scenario "Matching declarations pass without servers" (TypeScript).
#[tokio::test]
async fn ts_matching_declarations_pass_without_servers() {
    run_serverless(TS_LANGUAGE).await;
}

/// Spec scenario "Contradicting tier fails the run" (task 4.3): a fixture
/// declaring the LSP tier whose run observes a tree-sitter-declared result
/// FAILS, naming query, declared tier, and observed tier.
#[test]
fn contradicting_tier_fails_the_run() {
    let manifest = ExpectedManifest {
        schema_version: 1,
        language: "synthetic".to_string(),
        queries: vec![ExpectedQuery {
            op: "get_symbols".to_string(),
            file: "src/lib.rs".to_string(),
            line: None,
            column: None,
            expected: ExpectedObservation {
                tier: "S2".to_string(),
                outcome: ExpectedOutcome::Served,
                diagnostics: vec![],
            },
        }],
    };
    // The run observed a tree-sitter-declared result instead.
    let observations = vec![Observation {
        query: "get_symbols src/lib.rs".to_string(),
        tier: Some(PrecisionTier::S0),
        outcome: ExpectedOutcome::Served,
        diagnostics: vec![],
    }];

    let failures = harness::verify(&manifest, &observations);

    assert!(
        !failures.is_empty(),
        "a tier contradiction must fail the run"
    );
    let message = failures.join("\n");
    assert!(
        message.contains("get_symbols src/lib.rs"),
        "the failure must name the query: {message}"
    );
    assert!(
        message.contains("declared tier S2"),
        "the failure must name the declared tier: {message}"
    );
    assert!(
        message.contains("observed tier S0"),
        "the failure must name the observed tier: {message}"
    );
}

/// A declaration match never depends on a numeric threshold: an identical
/// observation passes; only a contradiction fails.
#[test]
fn matching_declaration_passes_without_thresholds() {
    let manifest = ExpectedManifest {
        schema_version: 1,
        language: "synthetic".to_string(),
        queries: vec![ExpectedQuery {
            op: "hover".to_string(),
            file: "src/lib.rs".to_string(),
            line: Some(3),
            column: Some(5),
            expected: ExpectedObservation {
                tier: "S1".to_string(),
                outcome: ExpectedOutcome::Served,
                diagnostics: vec![],
            },
        }],
    };
    let observations = vec![Observation {
        query: "hover src/lib.rs:3".to_string(),
        tier: Some(PrecisionTier::S1),
        outcome: ExpectedOutcome::Served,
        diagnostics: vec![],
    }];

    assert!(harness::verify(&manifest, &observations).is_empty());
}

/// A declared outcome mismatch (Served declared, Unresolved observed) also
/// fails the run, naming the query.
#[test]
fn declared_served_observed_unresolved_fails_the_run() {
    let manifest = ExpectedManifest {
        schema_version: 1,
        language: "synthetic".to_string(),
        queries: vec![ExpectedQuery {
            op: "get_definition".to_string(),
            file: "src/lib.rs".to_string(),
            line: Some(3),
            column: Some(5),
            expected: ExpectedObservation {
                tier: "S1".to_string(),
                outcome: ExpectedOutcome::Served,
                diagnostics: vec![],
            },
        }],
    };
    let observations = vec![Observation {
        query: "get_definition src/lib.rs:3".to_string(),
        tier: None,
        outcome: ExpectedOutcome::Unresolved,
        diagnostics: vec![ProviderDiagnostic::new(
            "local-resolver",
            PrecisionTier::S1,
            cognicode_core::domain::traits::code_intelligence::ProviderOutcome::Degraded,
            "no definition",
        )],
    }];

    let failures = harness::verify(&manifest, &observations);
    assert!(!failures.is_empty(), "an outcome contradiction must fail");
    assert!(
        failures.join("\n").contains("get_definition src/lib.rs:3"),
        "{failures:?}"
    );
}

/// Spec scenario "Java without its server degrades by declaration": with
/// `jdtls` absent from `PATH`, the Java suite asserts the declared
/// unavailability (an S2 `Unavailable` diagnostic, no LSP-tier result) and
/// passes. Gated on the PATH probe — on a machine WITH jdtls this branch
/// does not apply and the test reports the skip.
#[tokio::test]
async fn java_without_its_server_degrades_by_declaration() {
    if harness::binary_on_path(JAVA_SERVER_BINARY) {
        println!("skipped: {JAVA_SERVER_BINARY} is on PATH; the unavailable branch does not apply");
        return;
    }

    let manifest = harness::load_manifest(JAVA_LANGUAGE);
    let root = harness::fixture_root(JAVA_LANGUAGE);
    // The fixture-declared 5s bounded readiness (W3) for the unavailable
    // branch: the S2 attempt falls through at 5s.
    let provider = CompositeProvider::with_policy(
        &root,
        TierPolicy {
            fallback_readiness: Duration::from_secs(5),
            ..TierPolicy::all()
        },
    );

    let observations = harness::observe_all(&provider, &root, &manifest).await;
    println!("== java provider conformance (declared-unavailable) ==");
    for observation in &observations {
        println!("{}", describe(observation));
    }

    let failures = harness::verify_unavailable_degradation(&observations);
    assert!(
        failures.is_empty(),
        "the declared-unavailable path must pass:\n{}",
        failures.join("\n")
    );

    // The declared-unavailable path never fails: it asserts
    // unavailability, it does not demand a specific serving tier.
    for observation in &observations {
        assert_ne!(
            observation.tier,
            Some(PrecisionTier::S2),
            "{}: no result may declare the unavailable LSP tier",
            observation.query
        );
    }
}

/// The availability-gated counterpart: when `jdtls` IS available, the Java
/// suite runs the full S2-declared manifest. Without the server the branch
/// does not apply (reported as a skip) — never as a failure.
#[tokio::test]
async fn java_lsp_targets_verify_when_the_server_is_available() {
    if !harness::binary_on_path(JAVA_SERVER_BINARY) {
        println!(
            "skipped: {JAVA_SERVER_BINARY} is not on PATH; the declared-unavailable branch \
             covers this run"
        );
        return;
    }

    let manifest = harness::load_manifest(JAVA_LANGUAGE);
    let root = harness::fixture_root(JAVA_LANGUAGE);
    // Full readiness (W3): the bounded fallback equals the full 30s wait, so
    // a real jdtls cold start is never cut short and the declared-S2 run
    // holds.
    let provider = CompositeProvider::with_policy(
        &root,
        TierPolicy {
            fallback_readiness: Duration::from_secs(30),
            ..TierPolicy::all()
        },
    );

    let observations = harness::observe_all(&provider, &root, &manifest).await;
    println!("== java provider conformance (live {JAVA_SERVER_BINARY}) ==");
    for observation in &observations {
        println!("{}", describe(observation));
    }

    let failures = harness::verify(&manifest, &observations);
    assert!(
        failures.is_empty(),
        "java conformance with a live server failed:\n{}",
        failures.join("\n")
    );
}

/// The PATH probe itself is a pure scan: it must find an existing binary
/// (`cargo` in the test environment) and reject a name that cannot exist.
#[test]
fn path_probe_scans_without_spawning() {
    assert!(
        harness::binary_on_path("cargo"),
        "the probe must find an existing binary on PATH"
    );
    assert!(
        !harness::binary_on_path("cognicode-definitely-not-a-binary-xyz"),
        "the probe must reject a binary that cannot exist"
    );
}
