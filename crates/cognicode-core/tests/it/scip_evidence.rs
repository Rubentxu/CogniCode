//! The S3 tier answers code-intelligence questions from a real SCIP index.
//!
//! The fixture is a rust-analyzer index of a two-function crate, produced
//! by an indexer outside the product: the product only reads the bytes. The
//! source the index was produced from travels with it, so every span the
//! index declares can be checked against the source it names — that is what
//! makes the UTF-8 mapping a fact and not a claim.
//!
//! The oracle for the reference comparison is that source, verified by
//! hand: `greet` is declared once and called once. The compiler is not run
//! as an oracle here, because a test that compiles is a build disguised as
//! a test.

use std::time::Instant;

use sha2::Digest;

use cognicode_core::domain::traits::code_intelligence::{
    PrecisionTier, ReferenceKind, TieredCodeIntelligenceProvider,
};
use cognicode_core::domain::value_objects::Location;
use cognicode_core::infrastructure::lsp::providers::composite::{CompositeProvider, TierPolicy};
use cognicode_core::infrastructure::scip::{DEFAULT_MAX_INDEX_BYTES, ScipIndex, ScipIndexBinding};

const REAL_INDEX: &[u8] = include_bytes!("../data/scip/rust-analyzer-minimal.scip");
const REAL_SOURCE: &str = include_str!("../data/scip/fixture/src/lib.rs");

/// The root the fixture index was produced for.
const PROJECT_ROOT: &str = "/tmp/cgh096-scip-fixture";
/// The document the fixture index covers.
const DOCUMENT: &str = "src/lib.rs";
/// The symbol of the function the fixture declares and calls.
const GREET: &str = "rust-analyzer cargo scip_fixture 0.1.0 greet().";

/// The identity envelope the fixture index was produced with.
fn fixture_binding() -> ScipIndexBinding {
    ScipIndexBinding {
        repository_id: "fixture-repo".to_string(),
        revision: "0123456789abcdef".to_string(),
        build_configuration: "cargo-default".to_string(),
        indexer: "rust-analyzer".to_string(),
        indexer_version: "1.96.0 (ac68faa 2026-05-25)".to_string(),
        project_root: format!("file://{PROJECT_ROOT}"),
        scope: vec![DOCUMENT.to_string()],
        payload_sha256: hex::encode(sha2::Sha256::digest(REAL_INDEX)),
    }
}

/// A bound index over the fixture.
fn bound_index() -> ScipIndex {
    let binding = fixture_binding();
    ScipIndex::parse_bound(REAL_INDEX, DEFAULT_MAX_INDEX_BYTES, &binding, &binding)
        .expect("the fixture index is bound to the envelope it was produced with")
}

/// The text the source holds at a published position, in UTF-8 bytes.
fn source_text_at(line: u32, column: u32, length: usize) -> String {
    let source_line = REAL_SOURCE
        .lines()
        .nth(line as usize - 1)
        .expect("the fixture source has the line the index names");
    let start = column as usize - 1;
    source_line
        .get(start..start + length)
        .expect("the fixture source has the span the index names")
        .to_string()
}

#[test]
fn the_index_maps_utf8_byte_spans_onto_the_source_it_was_produced_from() {
    let index = ScipIndex::parse(REAL_INDEX, DEFAULT_MAX_INDEX_BYTES).unwrap();
    let occurrences = index.occurrences(GREET).unwrap();

    assert_eq!(occurrences.len(), 2);

    let definition = occurrences
        .iter()
        .find(|occurrence| occurrence.is_definition)
        .expect("the fixture declares `greet`");
    assert_eq!(definition.range.start().line(), 1);
    assert_eq!(definition.range.start().column(), 8);
    assert_eq!(
        source_text_at(1, 8, 5),
        "greet",
        "the declared span names the function in the source"
    );

    let call = occurrences
        .iter()
        .find(|occurrence| !occurrence.is_definition)
        .expect("the fixture calls `greet`");
    assert_eq!(call.range.start().line(), 6);
    assert_eq!(call.range.start().column(), 5);
    assert_eq!(
        source_text_at(6, 5, 5),
        "greet",
        "the referenced span names the call in the source"
    );
}

#[tokio::test]
async fn the_s3_tier_serves_the_references_the_source_declares() {
    let index = bound_index();
    let provider = CompositeProvider::with_scip_index(
        std::path::Path::new(PROJECT_ROOT),
        index,
        TierPolicy::all(),
    );

    let declaration = Location::new(format!("{PROJECT_ROOT}/{DOCUMENT}"), 1, 8);
    let outcome = provider.find_references_tiered(&declaration, true).await;

    assert_eq!(outcome.tier(), Some(PrecisionTier::S3));
    let references = outcome.value().expect("the tier served references");

    // The oracle: the source declares `greet` once and calls it once.
    let oracle = [(1u32, 8u32), (6u32, 5u32)];
    let observed = references
        .iter()
        .map(|reference| (reference.location.line(), reference.location.column()))
        .collect::<Vec<_>>();

    let matched = observed
        .iter()
        .filter(|position| oracle.contains(*position))
        .count();
    let precision = matched as f64 / observed.len() as f64;
    let recall = matched as f64 / oracle.len() as f64;
    assert_eq!(
        precision, 1.0,
        "every reference the tier served is in the oracle"
    );
    assert_eq!(recall, 1.0, "every reference of the oracle was served");

    let call = references
        .iter()
        .find(|reference| reference.location.line() == 6)
        .expect("the call site is among the references");
    assert_eq!(call.reference_kind, ReferenceKind::Call);
}

#[tokio::test]
async fn the_s3_tier_answers_without_reading_the_workspace_or_running_anything() {
    // Every other tier is disabled: with the language server, the local
    // resolver and the tree-sitter tier out of the pipeline, the answer can
    // only come from the index bytes — no file is read and no process runs.
    let index = bound_index();
    let provider = CompositeProvider::with_scip_index(
        std::path::Path::new(PROJECT_ROOT),
        index,
        TierPolicy {
            lsp: false,
            local_resolver: false,
            tree_sitter: false,
            scip: true,
            ..TierPolicy::all()
        },
    );

    let declaration = Location::new(format!("{PROJECT_ROOT}/{DOCUMENT}"), 1, 8);
    let outcome = provider.get_definition_tiered(&declaration).await;
    assert_eq!(outcome.tier(), Some(PrecisionTier::S3));
    let definition = outcome
        .value()
        .expect("the tier served a definition")
        .clone()
        .expect("the tier named the definition");
    assert_eq!(definition.line(), 1);
    assert_eq!(definition.column(), 8);
}

#[tokio::test]
async fn an_index_bound_to_another_context_serves_nothing() {
    let binding = fixture_binding();
    let mut other_revision = binding.clone();
    other_revision.revision = "fedcba9876543210".to_string();

    assert!(
        ScipIndex::parse_bound(
            REAL_INDEX,
            DEFAULT_MAX_INDEX_BYTES,
            &binding,
            &other_revision,
        )
        .is_err(),
        "an index bound to another revision is refused"
    );

    // A provider built with an index that does not cover the position
    // answers nothing at S3: the absence of index evidence is not evidence
    // of absence, so the pipeline continues with the lower tiers.
    let index = bound_index();
    let provider = CompositeProvider::with_scip_index(
        std::path::Path::new(PROJECT_ROOT),
        index,
        TierPolicy {
            lsp: false,
            local_resolver: false,
            tree_sitter: false,
            scip: true,
            ..TierPolicy::all()
        },
    );

    let outside = Location::new("/tmp/other-project/src/lib.rs", 1, 8);
    let outcome = provider.find_references_tiered(&outside, true).await;
    assert_ne!(outcome.tier(), Some(PrecisionTier::S3));
}

#[test]
fn the_cost_of_the_tier_is_bounded_by_the_size_of_the_index() {
    let started = Instant::now();
    let index = bound_index();
    let parse_cost = started.elapsed();

    let started = Instant::now();
    let declaration = Location::new(format!("{PROJECT_ROOT}/{DOCUMENT}"), 1, 8);
    let references = index.find_references_at(&declaration, true).unwrap();
    let query_cost = started.elapsed();

    assert_eq!(references.len(), 2);
    assert!(
        parse_cost.as_secs_f64() < 2.0,
        "decoding a {} byte index took {parse_cost:?}",
        REAL_INDEX.len()
    );
    assert!(
        query_cost.as_secs_f64() < 1.0,
        "answering from the index took {query_cost:?}"
    );
}
