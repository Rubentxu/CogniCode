//! Decisive repro: does the `map` symbol collision actually cause the
//! heap corruption, or is the ODR violation a separate latent issue?
//!
//! ASan reports an odr-violation (two size=100 globals named `map`) and
//! stops there. But an ODR violation is only a *naming* problem; the
//! `corrupted size vs. prev_size` abort is a separate *memory* problem.
//! This test links the real grammars into a test binary and drives the
//! real parser under ASan with `detect_odr_violation=0`, so the run can
//! proceed to the actual out-of-bounds access if there is one.
//!
//! Run with:
//!   RUSTFLAGS="-Zsanitizer=address" CFLAGS="-fsanitize=address" \
//!   ASAN_OPTIONS="detect_odr_violation=0" \
//!   cargo +nightly test -p cognicode-core \
//!     --test map_symbol_collision_repro -- --ignored --nocapture

use cognicode_core::infrastructure::parser::TreeSitterParser;
use cognicode_core::domain::value_objects::Language;

/// Recursively read every tracked file under `dir`, so the parse loop sees
/// the same breadth that `build_index` sees over the real repository.
fn collect_sources(dir: &std::path::Path, out: &mut Vec<(String, String)>, depth: usize) {
    if depth > 12 || out.len() > 4000 {
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
            collect_sources(&path, out, depth + 1);
        } else if meta.is_file() {
            let Some(lang) = Language::from_extension(path.extension()) else {
                continue;
            };
            // Multi-language breadth is the point: the collision needs
            // several grammars linked in, so restricting to one language
            // would hide it.
            let _ = lang;
            if let Ok(src) = std::fs::read_to_string(&path) {
                out.push((path.to_string_lossy().to_string(), src));
            }
        }
    }
}

/// Parse many files across many languages, which is what forces the
/// `ADVANCE_MAP` block statics of many grammars to be exercised.
#[test]
#[ignore = "large multi-language parse; run explicitly under ASan"]
fn map_symbol_collision_corrupts_heap_under_asan() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .to_path_buf();

    let mut files = Vec::new();
    collect_sources(&root, &mut files, 0);
    assert!(
        files.len() > 50,
        "expected a broad multi-language corpus, collected {}",
        files.len()
    );

    let mut parsers: std::collections::HashMap<Language, TreeSitterParser> =
        std::collections::HashMap::new();
    let mut total_symbols = 0usize;

    for (path, src) in &files {
        let Some(lang) = Language::from_extension(std::path::Path::new(path).extension())
        else {
            continue;
        };
        let parser = parsers.entry(lang).or_insert_with(|| {
            TreeSitterParser::new(lang).expect("grammar must load")
        });
        if let Ok(symbols) = parser.find_all_symbols_with_path(src, path) {
            total_symbols += symbols.len();
        }
    }

    eprintln!(
        "parsed {} files across {} grammars, {} symbols",
        files.len(),
        parsers.len(),
        total_symbols
    );
    assert!(total_symbols > 0, "no symbols parsed");
}