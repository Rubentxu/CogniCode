//! Does `LightweightIndex::build_index` over the repo root abort under ASan?
//!
//! The earlier repro in this file reimplemented the parse loop, which is
//! weaker than calling the real thing: `build_index` walks with
//! `follow_links(true)` (so symlinked fixture trees are indexed twice) and
//! applies `WalkFilter`, and neither was reproduced by the hand-rolled loop.
//! The point of this test is to remove that gap, so it calls the production
//! entry point directly and lets ASan watch the whole call.
//!
//! Run with:
//!   RUSTFLAGS="-Zsanitizer=address" CFLAGS="-fsanitize=address" \
//!   ASAN_OPTIONS="detect_odr_violation=0" \
//!   cargo +nightly test -p cognicode-core \
//!     --test map_symbol_collision_repro -- --ignored --nocapture --test-threads=1

use cognicode_core::domain::value_objects::Language;
use cognicode_core::infrastructure::graph::LightweightIndex;
use cognicode_core::infrastructure::parser::TreeSitterParser;

/// Sanity floor: the real walk must actually reach the parser, otherwise a
/// green result here would only prove that the walk found nothing.
#[test]
#[ignore = "indexes the whole repo; run explicitly under ASan"]
fn build_index_over_repo_root_survives_under_asan() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf();

    assert!(root.is_dir(), "workspace root missing: {}", root.display());

    let mut index = LightweightIndex::new();
    index
        .build_index(&root)
        .unwrap_or_else(|e| panic!("build_index failed over {}: {e}", root.display()));

    let symbols = index.symbol_count();
    eprintln!("build_index over {} -> {symbols} symbols", root.display());
    assert!(
        symbols > 0,
        "build_index reported success but indexed nothing"
    );
}

/// How many distinct grammars does the repo actually contain files for?
///
/// `Language::to_ts_language` links all 30 grammars into the binary, but the
/// walk only ever loads the ones matching files that exist. Knowing the real
/// count separates "the collision needs N grammars" from "the repo only
/// exercises M of them".
#[test]
fn repo_grammar_breadth_is_measured_not_assumed() {
    fn walk(dir: &std::path::Path, langs: &mut Vec<Language>, depth: usize) {
        if depth > 16 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_symlink() {
                continue;
            }
            if meta.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }
                walk(&path, langs, depth + 1);
            } else if meta.is_file()
                && let Some(l) = Language::from_extension(path.extension())
                && !langs.contains(&l)
            {
                langs.push(l);
            }
        }
    }

    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf();

    let mut langs = Vec::new();
    walk(&root, &mut langs, 0);
    eprintln!(
        "repo contains {} distinct languages: {langs:?}",
        langs.len()
    );

    // Every one of them must load. This is a measured precondition, not the
    // thing under test, so it stays a plain assertion.
    for l in &langs {
        TreeSitterParser::new(*l).unwrap_or_else(|e| panic!("{l:?} failed to load: {e}"));
    }
    assert!(!langs.is_empty(), "no languages found in repo");
}
