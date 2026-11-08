//! Measure the standalone incremental scanner without claiming product support.
//!
//! This example is an instrument, not a product route. It owns a small fixture,
//! runs the scanner through first/unchanged/edit/add-delete phases, and emits
//! machine-readable counts. `load_or_build_graph` remains a separate route.

use cognicode_core::infrastructure::graph::IncrementalScanner;
use serde::Serialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct Phase {
    changed: Vec<String>,
    unchanged: Vec<String>,
    deleted: Vec<String>,
    unreadable: Vec<String>,
    files_processed: usize,
    files_skipped: usize,
    files_removed: usize,
    total_tracked: usize,
    was_first_scan: bool,
}

fn phase(scanner: &mut IncrementalScanner, files: &[PathBuf]) -> Phase {
    let (delta, result) = scanner.scan(files);
    Phase {
        changed: delta
            .changed
            .into_iter()
            .map(|path| path.display().to_string())
            .collect(),
        unchanged: delta
            .unchanged
            .into_iter()
            .map(|path| path.display().to_string())
            .collect(),
        deleted: delta
            .deleted
            .into_iter()
            .map(|path| path.display().to_string())
            .collect(),
        unreadable: delta
            .unreadable
            .into_iter()
            .map(|(path, reason)| format!("{}:{reason:?}", path.display()))
            .collect(),
        files_processed: result.files_processed,
        files_skipped: result.files_skipped,
        files_removed: result.files_removed,
        total_tracked: result.total_tracked,
        was_first_scan: result.was_first_scan,
    }
}

fn write_fixture(root: &Path) {
    std::fs::create_dir_all(root).expect("create fixture root");
    std::fs::write(root.join("a.rs"), "pub fn a() -> u32 { 1 }\n").expect("write a.rs");
    std::fs::write(root.join("b.rs"), "pub fn b() -> u32 { 2 }\n").expect("write b.rs");
}

fn main() {
    let root = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        eprintln!("usage: measure_incremental_scanner <fixture-directory>");
        std::process::exit(2);
    });
    write_fixture(&root);

    let mut scanner = IncrementalScanner::new(root.clone());
    let initial_files = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
    let first = phase(&mut scanner, &initial_files);
    let unchanged = phase(&mut scanner, &initial_files);

    // The replacement has the same byte length. The scanner must use content,
    // not a clock tick, to classify it as changed.
    std::fs::write(root.join("a.rs"), "pub fn z() -> u32 { 9 }\n").expect("edit a.rs");
    let edited = phase(&mut scanner, &initial_files);

    std::fs::remove_file(root.join("b.rs")).expect("delete b.rs");
    std::fs::write(root.join("c.rs"), "pub fn c() -> u32 { 3 }\n").expect("write c.rs");
    let added_deleted = phase(
        &mut scanner,
        &[PathBuf::from("a.rs"), PathBuf::from("c.rs")],
    );

    println!(
        "{}",
        serde_json::json!({
            "route": "IncrementalScanner",
            "product_route": "load_or_build_graph",
            "product_delta_evidence": "NOT_RUN",
            "phases": {
                "first": first,
                "unchanged": unchanged,
                "edited_equal_size": edited,
                "added_deleted": added_deleted,
            }
        })
    );
}
