//! H1.4 (delivery-first-radical, package integration) — acceptance:
//! the H1 system holds end-to-end.
//!
//! This is the test that closes the H1 program. H1.1 (cycle 1) gave
//! the envelope at the type level. H1.2 (cycle 2) gave the trait
//! dispatch on the strategies. H1.3 (cycle 3) routed
//! `AnalysisService` through the same classifier. H1.4 is the test
//! that says: given one corpus, the three projections tell the
//! same story.
//!
//! The corpus is intentionally adversarial:
//!
//!   - 3 Rust source files that parse and contribute symbols
//!     (analyzed, become graph entries).
//!   - 1 Kotlin file (.kt) that parses (K1: Kotlin tiene gramática,
//!     así que es fuente y no hueco).
//!   - 1 README.md (legit exclusion, must NOT surface in any
//!     report, must NOT demote BuildStatus).
//!   - 1 .env file (security blocklist, must be pruned BEFORE
//!     the read in find_symbol_usages, must NOT appear in any
//!     build report's skipped list because security prunes
//!     happen pre-walk).
//!   - 1 target/artifact.rs, 1 node_modules/dep.js, 1 .venv/pkg.py
//!     each containing a UNIQUE marker that would match the
//!     search query if the directory was walked (regression
//!     guard for the WalkFilter::default() dedup).
//!
//! The cross-cutting claim: the route that walks the whole project
//! and the route that folds it file by file are CONSISTENT on the
//! same corpus — they both stay Complete-or-Partial on the same set
//! of files, they both name the same source they could not read, they
//! both publish the same references they could not tie, the .env is
//! invisible to both, and find_symbol_usages prunes everything the
//! WalkFilter prunes.
//!
//! `Complete` means more than it used to mean. It is no longer only
//! «every file was read»: it is «every file was read AND every
//! reference was tied to a declaration». A build that could not tie a
//! call is `Partial`, and it publishes that call with its motive — so
//! the two routes have to agree on the loss too, not only on the
//! files.
//!
//! MEDIDO. El sujeto del gap era Kotlin y ya no lo
//! es. Para que la garantía no se quede sin sujeto, el corpus
//! adversarial incluye además un `.kt` que NO PARSEA: es una fuente
//! real que el producto no puede leer, y las dos rutas tienen que
//! contarla igual.

use cognicode_core::application::services::analysis_service::{AnalysisService, UsageSearchParams};
use cognicode_core::domain::value_objects::{PendingClass, PendingReference};
use cognicode_core::infrastructure::graph::per_file_graph::{
    BuildStatus, FileDisposition, SkipReason, SkippedFile,
};

const MARKER_LEGAL: &str = "unique_keep_marker_legal";
const MARKER_TARGET: &str = "unique_keep_marker_target";
const MARKER_NODE_MODULES: &str = "unique_keep_marker_node_modules";
const MARKER_VENV: &str = "unique_keep_marker_venv";
const MARKER_ENV: &str = "unique_keep_marker_env";

fn tempdir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "h14-system-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn adversarial_corpus(dir: &std::path::Path) {
    // Three Rust modules: produce symbols (analyzed).
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("main.rs"),
        format!(
            "mod alpha;\nmod beta;\nfn {MARKER_LEGAL}() {{ alpha::helper(); beta::helper(); }}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("alpha.rs"),
        "pub fn helper() -> u32 { 1 }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src").join("beta.rs"),
        "pub fn helper() -> u32 { 2 }\n",
    )
    .unwrap();
    // Kotlin: una fuente de verdad, y se lee (K1).
    std::fs::write(
        dir.join("src").join("Other.kt"),
        "class Other { fun x() = 1 }\n",
    )
    .unwrap();
    // Legit exclusion: README does not affect completeness.
    std::fs::write(dir.join("README.md"), "# Fixture\n").unwrap();
    // Security blocklist: must be pruned BEFORE the read.
    std::fs::write(
        dir.join(".env"),
        format!("SECRET={MARKER_ENV}=do_not_search\n"),
    )
    .unwrap();
    // Performance skip directories: each holds a unique marker
    // that would match find_symbol_usages if the directory was
    // walked. The dedup must prune all of them.
    std::fs::create_dir_all(dir.join("target")).unwrap();
    std::fs::write(
        dir.join("target").join("artifact.rs"),
        format!("// decoy\npub fn {MARKER_TARGET}() {{}}\n"),
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("node_modules")).unwrap();
    std::fs::write(
        dir.join("node_modules").join("dep.js"),
        format!("// decoy\nfunction {MARKER_NODE_MODULES}() {{}}\n"),
    )
    .unwrap();
    std::fs::create_dir_all(dir.join(".venv")).unwrap();
    std::fs::write(
        dir.join(".venv").join("pkg.py"),
        format!("# decoy\ndef {MARKER_VENV}(): pass\n"),
    )
    .unwrap();
}

/// El mismo corpus, con una fuente Kotlin que NO parsea.
fn corpus_with_unreadable_kotlin(dir: &std::path::Path) {
    adversarial_corpus(dir);
    std::fs::write(
        dir.join("src").join("Broken.kt"),
        "class Broken {\n    fun oops( {\n}\n",
    )
    .unwrap();
}

fn broken_skip(skipped: &[SkippedFile]) -> &SkippedFile {
    skipped
        .iter()
        .find(|s| s.path.ends_with("Broken.kt"))
        .expect("the unreadable Kotlin must be in skipped")
}

fn assert_compatible_partial(label: &str, skipped: &[SkippedFile], other_skipped_paths: &[&str]) {
    assert!(
        !skipped.is_empty(),
        "{label}: expected Partial because of the unreadable Kotlin, got Complete"
    );
    let broken = broken_skip(skipped);
    assert!(
        matches!(broken.reason, SkipReason::Parse(_)),
        "{label}: the unreadable Kotlin must be reported as a parse failure, got {:?}",
        broken.reason
    );
    let disp = FileDisposition::from_skipped(broken);
    assert!(
        matches!(disp, FileDisposition::ParseFailed { .. }),
        "{label}: the unreadable source must project to ParseFailed, got {disp:?}"
    );
    for other in other_skipped_paths {
        assert!(
            !skipped.iter().any(|s| s.path.contains(other)),
            "{label}: {other} must NOT surface as a skip (it is pruned by the walk, not skipped after the read)"
        );
    }
}

#[test]
fn the_two_live_routes_agree_that_a_kotlin_source_is_analysed() {
    // The cross-cutting claim: the route that walks the whole project
    // and the route that folds it file by file agree on the SAME
    // corpus. With Kotlin readable, the whole corpus is covered, and
    // a route that still hung the `.kt` would be publishing a gap
    // that does not exist.
    let tmp = tempdir();
    adversarial_corpus(&tmp);

    // The whole-project route.
    let whole = AnalysisService::new();
    whole
        .build_project_graph(&tmp)
        .expect("the whole-project route builds");
    let whole_report = whole
        .get_last_build_report()
        .expect("the whole-project route publishes a report");

    // The per-file route.
    let svc = AnalysisService::new();
    let service_report = svc
        .build_graph_per_file_with_report(&tmp)
        .expect("the per-file route builds");

    // `Complete` ya no significa sólo «se leyó todo»: significa que se
    // leyó todo Y que cada referencia quedó atada a una declaración.
    // Este corpus llama a `helper`, que el proyecto declara dos veces,
    // así que esas dos llamadas no se pueden atar: el build es
    // `Partial` POR ELLAS, no por un fichero sin leer.
    for (label, report) in [("whole route", &whole_report), ("service", &service_report)] {
        assert!(
            skipped_of(&report.status).is_empty(),
            "{label}: el corpus entero se lee —Rust, Kotlin y el README \
             excluido a propósito—, así que no hay ficheros omitidos: {:?}",
            skipped_of(&report.status)
        );
        let pending = pending_of(&report.status);
        assert_eq!(
            pending.len(),
            2,
            "{label}: las dos llamadas a `helper` tienen que publicarse como pérdida: {pending:?}"
        );
        for reference in &pending {
            assert_eq!(
                reference.target, "helper",
                "{label}: la pérdida es la llamada ambigua, y su nombre dice cuál es"
            );
            assert_eq!(
                reference.class,
                PendingClass::Ambiguous { candidates: 2 },
                "{label}: la pérdida dice su motivo: el nombre lo declara el proyecto dos veces"
            );
        }
    }

    // Las dos rutas pierden lo MISMO. Si una publicara una pérdida y la
    // otra otra, el producto tendría dos grafos y dos coberturas.
    assert_eq!(
        pending_of(&whole_report.status),
        pending_of(&service_report.status),
        "las dos rutas tienen que publicar la misma pérdida"
    );
    // Neither side hangs the Kotlin source, and neither surfaces the
    // assets that are out of scope by policy.
    for (label, skipped) in [
        ("whole route", &skipped_of(&whole_report.status)),
        ("service", &skipped_of(&service_report.status)),
    ] {
        assert!(
            !skipped.iter().any(|s| s.path.ends_with("Other.kt")),
            "{label}: a Kotlin source the product can read must not be a skip"
        );
        assert!(
            !skipped.iter().any(|s| s.path.ends_with("README.md")),
            "{label}: README is LegitExclusion, must not be a skip"
        );
        for pruned in ["target", "node_modules", ".venv", ".env"] {
            assert!(
                !skipped.iter().any(|s| s.path.contains(pruned)),
                "{label}: pruned directory {pruned} must not surface as a skip"
            );
        }
    }

    // Cross-compat: the service side's typed report is reachable
    // through the public accessor (no more stale-data after a
    // per-file build, H1.3 invariant).
    let accessor_report = svc
        .get_last_build_report()
        .expect("get_last_build_report after build_graph_per_file_with_report");
    assert_eq!(
        accessor_report.status, service_report.status,
        "the accessor must return the report of THIS build, not a stale one"
    );
}

#[test]
fn the_two_live_routes_agree_on_what_they_could_not_read() {
    // La misma garantía, con un sujeto REAL: un `.kt` que no parsea.
    // Las dos rutas tienen que contarlo igual, con la misma razón.
    // Sin esto, la sección de arriba se contentaría con que las dos
    // digan «Complete», que es un acuerdo sobre una nada.
    let tmp = tempdir();
    corpus_with_unreadable_kotlin(&tmp);

    let whole = AnalysisService::new();
    whole
        .build_project_graph(&tmp)
        .expect("the whole-project route builds");
    let whole_report = whole
        .get_last_build_report()
        .expect("the whole-project route publishes a report");

    let svc = AnalysisService::new();
    let service_report = svc
        .build_graph_per_file_with_report(&tmp)
        .expect("the per-file route builds");

    let whole_skipped = skipped_of(&whole_report.status);
    let service_skipped = skipped_of(&service_report.status);

    for (label, skipped) in [
        ("whole route", &whole_skipped),
        ("service", &service_skipped),
    ] {
        assert_compatible_partial(label, skipped, &["target", "node_modules", ".venv", ".env"]);
        assert!(
            !skipped.iter().any(|s| s.path.ends_with("README.md")),
            "{label}: README is LegitExclusion, must not be a skip"
        );
    }

    // The two routes name the same source, with the same KIND of
    // reason. La PALABRA del mensaje no es el contrato —las dos rutas
    // redactan el error de parseo a su manera—, lo que no puede
    // diferir es el hecho: las dos dicen «no lo pude parsear».
    let whole_broken = broken_skip(&whole_skipped);
    let service_broken = broken_skip(&service_skipped);
    assert!(
        matches!(whole_broken.reason, SkipReason::Parse(_)),
        "the whole route must report a parse failure, got {:?}",
        whole_broken.reason
    );
    assert!(
        matches!(service_broken.reason, SkipReason::Parse(_)),
        "the per-file route must report a parse failure, got {:?}",
        service_broken.reason
    );
    for (label, broken) in [("whole route", whole_broken), ("service", service_broken)] {
        assert!(
            reason_text(&broken.reason).contains("Broken.kt"),
            "{label}: the reason must name the file it is about, got {:?}",
            broken.reason
        );
    }
}

/// Los omitidos de un estado; `Complete` no tiene ninguno.
///
/// `Complete` significa que no se quedó nada sin atar: ni un fichero sin
/// leer, ni una referencia que el producto no pudo atar a una
/// declaración. Los omitidos son una parte de esa pérdida; la otra son
/// las referencias publicadas como pendientes.
fn skipped_of(status: &BuildStatus) -> Vec<SkippedFile> {
    match status {
        BuildStatus::Complete => Vec::new(),
        BuildStatus::Partial { skipped, .. } => skipped.clone(),
    }
}

/// La pérdida que el estado publica como referencias sin atar.
///
/// `Complete` no publica ninguna: un build que ató todo no tiene nada
/// que decir de sus referencias.
fn pending_of(status: &BuildStatus) -> Vec<PendingReference> {
    match status {
        BuildStatus::Complete => Vec::new(),
        BuildStatus::Partial { pending, .. } => pending.clone(),
    }
}

/// El texto que acompaña a la razón de un omitido.
fn reason_text(reason: &SkipReason) -> String {
    match reason {
        SkipReason::Read(message)
        | SkipReason::Parse(message)
        | SkipReason::UnsupportedExtension(message)
        | SkipReason::Walk(message)
        | SkipReason::Other(message) => message.clone(),
        SkipReason::Symlink { target } => target.clone(),
    }
}

#[test]
fn find_symbol_usages_prunes_security_and_performance_at_the_same_seam() {
    // The .env leak (H1.3 defect) and the perf-skip regression
    // (the dedup must not change which dirs are skipped) are
    // pinned together. The test searches for {MARKER_LEGAL}
    // which is unique to src/main.rs; the other four unique
    // markers (in target/, node_modules/, .venv/, .env) would
    // match if those directories were walked.
    let tmp = tempdir();
    adversarial_corpus(&tmp);

    let svc = AnalysisService::new();
    let params = UsageSearchParams {
        project_dir: tmp.clone(),
        symbol_name: MARKER_LEGAL.to_string(),
        include_declaration: true,
        context_lines: Some(0),
        first_only_definition: false,
    };
    let usages = svc.find_symbol_usages(params).expect("ok");
    let paths: Vec<String> = usages.iter().map(|u| u.file.clone()).collect();

    // The legal file is found.
    assert!(
        paths.iter().any(|p| p.contains("src/main.rs")),
        "the legal marker must be found in src/main.rs, got {paths:?}"
    );

    // None of the pruned directories leak a match.
    for (label, marker) in [
        ("target (perf skip)", MARKER_TARGET),
        ("node_modules (perf skip)", MARKER_NODE_MODULES),
        (".venv (perf skip)", MARKER_VENV),
        (".env (security blocklist)", MARKER_ENV),
    ] {
        assert!(
            !paths.iter().any(|p| p.contains(marker)),
            "{label}: marker {marker} must be invisible to find_symbol_usages, got {paths:?}"
        );
    }
}

#[test]
fn analysis_service_typed_method_does_not_lie_about_a_pure_rust_corpus() {
    // The other half of "the system holds": when the corpus has
    // NO K0 gap, NO read failure, NO parse failure, the report
    // is Complete. The trait default and the service method must
    // agree here too (a lie the other way — false Partial — is
    // just as bad as a missed Partial).
    //
    // `Complete` also requires that every reference was tied: this
    // corpus declares one function and calls nothing, so there is
    // nothing to tie and nothing to publish. A corpus that called an
    // ambiguous name would be `Partial`, and that is what the test
    // above measures.
    let tmp = tempdir();
    std::fs::create_dir_all(tmp.join("src")).unwrap();
    std::fs::write(
        tmp.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        tmp.join("src").join("lib.rs"),
        "pub fn add(a: u32, b: u32) -> u32 { a + b }\n",
    )
    .unwrap();
    std::fs::write(tmp.join("README.md"), "# Fixture\n").unwrap();
    std::fs::write(tmp.join(".env"), "KEY=val\n").unwrap();

    let svc = AnalysisService::new();
    let report = svc.build_graph_per_file_with_report(&tmp).expect("ok");
    assert_eq!(
        report.status,
        BuildStatus::Complete,
        "a pure-Rust corpus with a README and a .env must be Complete: the README is legit exclusion, the .env is pruned before the read"
    );

    let whole = AnalysisService::new();
    whole
        .build_project_graph(&tmp)
        .expect("the whole-project route builds");
    let whole_report = whole
        .get_last_build_report()
        .expect("the whole-project route publishes a report");
    assert_eq!(
        whole_report.status,
        BuildStatus::Complete,
        "the whole-project route must also report Complete for a pure-Rust corpus"
    );
}

#[test]
fn h14_helper_compat_partial_invariants_remain_anchored() {
    // Belt-and-suspenders for the helper used by the other tests:
    // assert_compatible_partial itself. If this drifts, the seam
    // invariants above are not actually being checked.
    //
    // El sujeto es un fichero Kotlin que no parsea: es la fuente que
    // el producto no puede leer, y su razón es de PARSEO, no de
    // «extensión no soportada». Antes de K1 el motivo era el segundo;
    // ahora el motivo es el primero, y confundirlos publicaría una
    // cobertura que el producto no tiene.
    let broken = SkippedFile {
        path: "fixture/Broken.kt".to_string(),
        reason: SkipReason::Parse("error node".to_string()),
    };
    let skipped = vec![broken];
    let other = ["target", "node_modules", ".venv", ".env"];
    assert_compatible_partial("helper", &skipped, &other);
}
