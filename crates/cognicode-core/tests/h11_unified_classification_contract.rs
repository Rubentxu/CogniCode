//! H1.1 (delivery-first-radical, package integration) — unified
//! classification envelope.
//!
//! Claim: there is exactly one logical classification for every file
//! that the walk encounters. The five-variant `FileDisposition` enum
//! in `infrastructure/graph/per_file_graph` is the *envelope* over the
//! existing taxonomy (`WalkClassification` + `SkipReason` + presence-
//! in-graph). It is NOT a replacement: AC-01 says "reutilizar los
//! tipos que ya existan, evolucionar solo si es preciso". The new
//! type is the *projection* that lets one invariant — "every file
//! lands in exactly one of the five states" — be expressed at the
//! type level instead of being scattered across three call sites
//! with three implicit vocabularies.
//!
//! The five variants are the ones named in the spec
//! (`docs/cognicode-delivery-first-radical-2026-10-17/.../01-HITOS-Y-TODO.md`
//! §H1.1 and `specs/AC-01-COBERTURA-Y-RESULTADOS.md`):
//!
//!   - `Parsed` — the file was read and contributed to the graph.
//!   - `ExcludedByPolicy` — `WalkFilter` pruned or skipped the path,
//!     OR the file is a known-irrelevant asset (README, image, lockfile).
//!     Never affects `BuildStatus`.
//!   - `UnsupportedRelevant` — the file is a *source* in a language
//!     we know about but have not yet wired (K0 honest gap). Affects
//!     `BuildStatus` → `Partial`.
//!   - `ReadFailed` — `std::fs::read` failed. Affects `BuildStatus` →
//!     `Partial`.
//!   - `ParseFailed` — the parser could not extract symbols. Affects
//!     `BuildStatus` → `Partial`.
//!
//! H1.1 test strategy: pin the mapping from the existing taxonomy to
//! the envelope. The five variants are exhaustive (covered by a
//! `match` in the helper). The 3 callsite changes (route the trait
//! through `_report`, dedupe `USAGE_SKIP_DIRS`, route
//! `build_graph_per_file` through the classifier) are H1.2/H1.3 —
//! they are propagation, not unification, and they are pinned by
//! separate test files. This file pins only the *type-level*
//! unification: a function `FileDisposition::from_walk_outcome`
//! exists, accepts the existing pieces, and never invents a sixth
//! state.

use cognicode_core::domain::value_objects::{WalkDecision, WalkFilter};
use cognicode_core::infrastructure::graph::per_file_graph::{
    FileDisposition, KNOWN_UNSUPPORTED_EXTENSIONS, SkipReason, SkippedFile, WalkClassification,
    classify_walk_extension,
};
use std::path::Path;

fn skipped_with(reason: SkipReason) -> SkippedFile {
    SkippedFile {
        path: "fixture/src/example.rs".to_string(),
        reason,
    }
}

fn filter_prunes_git_dir() -> WalkFilter {
    WalkFilter::default().with_security_blocklist()
}

fn filter_keeps_target_descended() -> WalkFilter {
    WalkFilter::new()
}

#[test]
fn file_disposition_has_exactly_the_five_spec_variants() {
    // The spec names these five and only these five. If a sixth
    // state ("Unknown", "Failed", "Pending", "Irrelevant", …) ever
    // appears, this test fails and the H1.1 invariant is broken
    // silently — the new variant is a "we don't know" escape hatch
    // that would re-introduce the bug the envelope was created to
    // prevent.
    let all = FileDisposition::all();
    assert_eq!(all.len(), 5, "H1.1 forbids a 6th state");
    let names: Vec<&'static str> = all.iter().map(|d| d.name()).collect();
    assert_eq!(
        names,
        vec![
            "Parsed",
            "ExcludedByPolicy",
            "UnsupportedRelevant",
            "ReadFailed",
            "ParseFailed",
        ],
        "the variant names are part of the public contract; AC-01 \
         names them by name and downstream tools may match on them"
    );
}

#[test]
fn kotlin_is_analysed_now_not_a_known_unsupported_gap() {
    // MEDIDO: la gramática de Kotlin aterrizó en el
    // registro, así que `kt`/`kts` ya no son un gap: se analizan. Antes
    // eran "lenguaje conocido sin soporte" y el fichero acababa en
    // `skipped` con `UnsupportedExtension`; ahora se parsean.
    assert!(
        KNOWN_UNSUPPORTED_EXTENSIONS.is_empty(),
        "K1: la lista de gap queda vacía cuando la gramática aterriza; \
         si una extensión vuelve a aparecer, es un lenguaje nuevo sin \
         soporte, y su propia línea debe añadirla con su motivo"
    );
    for ext in ["kt", "kts"] {
        let class = classify_walk_extension(Some(ext));
        assert_eq!(
            class,
            WalkClassification::Analyze,
            "K1: `{ext}` está en el registro y se analiza"
        );
        let disp = FileDisposition::from_walk_classification(class, Some(ext));
        assert!(
            matches!(disp, FileDisposition::Parsed),
            "K1: `{ext}` se parsea; no es una disposición de gap, got {disp:?}"
        );
    }
}

#[test]
fn the_known_unsupported_gap_still_maps_to_unsupported_relevant() {
    // El mecanismo de K0 sigue en pie aunque hoy ninguna extensión caiga
    // en él: cuando el próximo lenguaje aterrice sin gramática, su
    // fichero tiene que salir como "relevante sin soporte" y no como
    // exclusión por política, porque la exclusión por política no
    // degrada el estado del build y esta sí.
    let disp = FileDisposition::from_walk_classification(
        WalkClassification::KnownUnsupportedGap,
        Some("kt"),
    );
    assert!(
        matches!(disp, FileDisposition::UnsupportedRelevant { .. }),
        "el gap conocido sigue siendo UnsupportedRelevant, no ExcludedByPolicy, got {disp:?}"
    );
}

#[test]
fn readme_and_assets_project_to_excluded_by_policy_not_unsupported_relevant() {
    // The other half of the AC-01 §Invariantes note: README/assets
    // are LEGITIMATELY excluded (the file is walkable, has content,
    // but the extension is irrelevant to the graph). They MUST NOT
    // raise UnsupportedRelevant (that would falsely demote
    // BuildStatus to Partial) and MUST NOT appear in `skipped`.
    for ext in ["md", "png", "jpg", "lock", "toml"] {
        let class = classify_walk_extension(Some(ext));
        assert_eq!(
            class,
            WalkClassification::LegitExclusion,
            "`{ext}` is a known-irrelevant asset; classify must return LegitExclusion"
        );
        let disp = FileDisposition::from_walk_classification(class, Some(ext));
        assert!(
            matches!(disp, FileDisposition::ExcludedByPolicy { .. }),
            "asset `{ext}` must surface as ExcludedByPolicy (no gap), got {disp:?}"
        );
        // Belt and suspenders: NEVER matches the unsupported gap.
        assert!(
            !matches!(disp, FileDisposition::UnsupportedRelevant { .. }),
            "asset `{ext}` must never be confused with UnsupportedRelevant"
        );
    }
}

#[test]
fn walk_filter_pruned_path_projects_to_excluded_by_policy() {
    // The security blocklist (`.git/credentials`, `id_rsa`, …) means
    // "don't even read the file" — the walk never produces a
    // `SkippedFile` for it. The disposition is still
    // ExcludedByPolicy (this path is outside the analysis scope, by
    // policy). This pins the "no path is left unclassified" invariant
    // at the WalkFilter boundary.
    //
    // `WalkFilter::should_walk` matches by PATH COMPONENT, not by
    // full path: the test passes the directory `.git` itself, the
    // way the existing `should_walk_prunes_git_directory` unit test
    // does. The H1.1 invariant is the same for any path inside the
    // pruned directory.
    let filter = filter_prunes_git_dir();
    assert_eq!(
        filter.should_walk(Path::new(".git")),
        WalkDecision::Prune,
        "precondition: .git is in the security blocklist (component match)"
    );
    let disp = FileDisposition::from_walk_decision(filter.should_walk(Path::new(".git")));
    assert!(
        matches!(disp, FileDisposition::ExcludedByPolicy { .. }),
        "a pruned path is ExcludedByPolicy by definition, got {disp:?}"
    );
    assert!(
        !matches!(disp, FileDisposition::ReadFailed { .. }),
        "ExcludedByPolicy and ReadFailed are distinct — a pruned path was never attempted"
    );
}

#[test]
fn skip_reason_read_projects_to_read_failed() {
    let disp = FileDisposition::from_skipped(&skipped_with(SkipReason::Read("EACCES".into())));
    assert!(
        matches!(disp, FileDisposition::ReadFailed { .. }),
        "SkipReason::Read must project to ReadFailed, got {disp:?}"
    );
    // ReadFailed is NEVER an ExcludedByPolicy — the path was
    // considered relevant and the read was attempted.
    assert!(!matches!(disp, FileDisposition::ExcludedByPolicy { .. }));
}

#[test]
fn skip_reason_parse_projects_to_parse_failed() {
    let disp =
        FileDisposition::from_skipped(&skipped_with(SkipReason::Parse("unexpected EOF".into())));
    assert!(
        matches!(disp, FileDisposition::ParseFailed { .. }),
        "SkipReason::Parse must project to ParseFailed, got {disp:?}"
    );
    // ParseFailed and ReadFailed are distinct: a parse error means
    // the file was readable; a read error means it was not.
    assert!(!matches!(disp, FileDisposition::ReadFailed { .. }));
}

#[test]
fn skip_reason_unsupported_extension_projects_to_unsupported_relevant() {
    // Same projection as the WalkClassification path, but exercised
    // through the SkippedFile arm. A `.kt` file ends up in `skipped`
    // (because merge_with_report records the failure); the envelope
    // must still surface it as UnsupportedRelevant, not ReadFailed
    // (the read succeeded) and not ExcludedByPolicy (the path is
    // relevant source).
    let disp =
        FileDisposition::from_skipped(&skipped_with(SkipReason::UnsupportedExtension("kt".into())));
    assert!(
        matches!(disp, FileDisposition::UnsupportedRelevant { .. }),
        "SkipReason::UnsupportedExtension must project to UnsupportedRelevant, got {disp:?}"
    );
}

#[test]
fn a_file_in_the_graph_projects_to_parsed() {
    // A file that was read, parsed, and contributed symbols to the
    // merged graph has no SkippedFile and no WalkDecision against
    // it. Its disposition is Parsed. This is the only variant that
    // can be summarised as "this contributed to the answer".
    let disp = FileDisposition::parsed();
    assert!(matches!(disp, FileDisposition::Parsed));
    // The Parsed variant carries no payload — a per-file record
    // doesn't need it; the graph is the witness.
}

#[test]
fn skip_reason_other_falls_back_to_excluded_by_policy_not_unsupported() {
    // SkipReason::Other is a fallback for I/O errors that don't fit
    // Read/Parse. The H1.1 invariant says there are only 5
    // dispositions; mapping Other into UnsupportedRelevant would
    // falsely demote BuildStatus to Partial for paths we don't
    // know about. The honest mapping is ExcludedByPolicy (we
    // couldn't classify it → we don't claim a gap).
    let disp = FileDisposition::from_skipped(&skipped_with(SkipReason::Other("weird".into())));
    assert!(
        matches!(disp, FileDisposition::ExcludedByPolicy { .. }),
        "SkipReason::Other must project to ExcludedByPolicy (honest \
         default), not UnsupportedRelevant, got {disp:?}"
    );
    assert!(!matches!(disp, FileDisposition::UnsupportedRelevant { .. }));
}

#[test]
fn walked_supported_extension_with_no_skip_projects_to_parsed() {
    // The full happy path: a `.rs` file (or any of the 29 supported
    // languages) is walked, classified as Analyze, read, parsed,
    // and merged. The envelope projects it to Parsed, not
    // ExcludedByPolicy and not UnsupportedRelevant.
    let class = classify_walk_extension(Some("rs"));
    assert_eq!(class, WalkClassification::Analyze);
    let disp = FileDisposition::from_walk_classification(class, Some("rs"));
    // A `.rs` file with no skip reason is Parsed. We do this by
    // checking that from_walk_classification + a "no skip" outcome
    // collapses to Parsed, which is the helper's "happy path"
    // constructor.
    assert!(
        matches!(disp, FileDisposition::Parsed),
        "an Analyze-classified file with no skip must project to Parsed, got {disp:?}"
    );
    // And explicitly NOT to the other two single-keyword variants
    // that look like happy outcomes.
    assert!(!matches!(disp, FileDisposition::ExcludedByPolicy { .. }));
    assert!(!matches!(disp, FileDisposition::UnsupportedRelevant { .. }));
    // suppress the unused-variable warning for the second filter
    // builder (kept for symmetry and to anchor a future test that
    // exercises WalkDecision::Skip vs Prune).
    let _ = filter_keeps_target_descended();
}
