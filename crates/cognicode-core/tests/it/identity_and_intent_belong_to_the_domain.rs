//! `SymbolId` and `Intent` are pure vocabulary (serde only). An inner layer
//! that has to import them from `interface` points the dependency outwards,
//! which is what keeps the interface layer inside the core crate.
//!
//! The published paths `interface::symbol_identity` and `interface::intent`
//! stay as re-exports so older consumers keep compiling.

use std::fs;
use std::path::{Path, PathBuf};

const INNER_LAYERS: [&str; 6] = [
    "domain",
    "application",
    "infrastructure",
    "impact",
    "architecture",
    "refactor",
];
const MOVED: [&str; 2] = ["interface::symbol_identity", "interface::intent"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Production imports only: stop at the first `#[cfg(test)]`.
fn imports_of_moved_modules(text: &str) -> Vec<String> {
    text.lines()
        .take_while(|line| !line.trim_start().starts_with("#[cfg(test)]"))
        .filter(|line| line.trim_start().starts_with("use crate::"))
        .filter(|line| MOVED.iter().any(|m| line.contains(m)))
        .map(|line| line.trim().to_string())
        .collect()
}

#[test]
fn the_scan_sees_an_outward_import() {
    let found = imports_of_moved_modules("use crate::interface::symbol_identity::SymbolId;\n");
    assert_eq!(found.len(), 1, "{found:?}");
    let test_only = "#[cfg(test)]\nmod t { use crate::interface::intent::Intent; }\n";
    assert!(imports_of_moved_modules(test_only).is_empty());
}

#[test]
fn inner_layers_do_not_import_identity_or_intent_from_interface() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    for layer in INNER_LAYERS {
        let mut files = Vec::new();
        rust_files(&src.join(layer), &mut files);
        for file in files {
            let text = fs::read_to_string(&file).expect("readable source");
            for line in imports_of_moved_modules(&text) {
                offenders.push(format!(
                    "{}: {line}",
                    file.strip_prefix(&src).unwrap().display()
                ));
            }
        }
    }
    assert!(offenders.is_empty(), "imports hacia fuera: {offenders:#?}");
}

#[test]
fn the_published_interface_paths_still_resolve() {
    let id = cognicode_core::interface::symbol_identity::Precision::Lsp;
    let same = cognicode_core::domain::symbol_identity::Precision::Lsp;
    assert_eq!(id.as_str(), same.as_str());
    let _ = cognicode_core::interface::intent::route_intent;
}
