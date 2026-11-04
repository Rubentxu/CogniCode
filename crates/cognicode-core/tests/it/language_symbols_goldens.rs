//! The types and callables the reader finds in a versioned corpus, against
//! a hand-written golden.
//!
//! Each directory under `sandbox/fixtures/<corpus>` carries a
//! `symbols.golden` written from the sources, not from the reader's output.
//! A line `!missing <entry>` declares something the sources hold and the
//! reader does not produce; `!spurious <entry>` declares something the
//! reader produces and the sources do not hold. Both are asserted exactly,
//! so a declared limit cannot go stale: when the reader is fixed, the test
//! fails until the declaration is removed.

use std::path::{Path, PathBuf};

use cognicode_core::domain::value_objects::SymbolKind;
use cognicode_core::infrastructure::parser::{Language, TreeSitterParser};

fn corpus_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sandbox/fixtures")
        .join(name)
        .canonicalize()
        .expect("the fixture directory is versioned")
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// Every file the reader recognises, sorted.
fn sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk(root, &mut files);
    files.retain(|f| Language::from_path(f).is_some());
    files.sort();
    assert!(!files.is_empty(), "no readable files in {root:?}");
    files
}

/// `class|function <name>@<relative path>`, one per declaration read.
fn read(root: &Path) -> Vec<String> {
    let mut entries = Vec::new();
    for file in sources(root) {
        let language = Language::from_path(&file).unwrap();
        let relative = file
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(&file).unwrap();
        let symbols = TreeSitterParser::new(language)
            .unwrap()
            .find_all_symbols_with_path(&text, &relative)
            .unwrap();
        for symbol in symbols {
            let kind = match symbol.kind() {
                SymbolKind::Class => "class",
                SymbolKind::Function => "function",
                _ => continue,
            };
            if symbol.name().starts_with('<') {
                continue;
            }
            entries.push(format!("{kind} {}@{relative}", symbol.name()));
        }
    }
    entries.sort();
    entries
}

struct Golden {
    declared: Vec<String>,
    missing: Vec<String>,
    spurious: Vec<String>,
}

fn golden(root: &Path) -> Golden {
    let text = std::fs::read_to_string(root.join("symbols.golden")).unwrap();
    let mut g = Golden {
        declared: Vec::new(),
        missing: Vec::new(),
        spurious: Vec::new(),
    };
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(entry) = line.strip_prefix("!missing ") {
            g.missing.push(entry.to_string());
        } else if let Some(entry) = line.strip_prefix("!spurious ") {
            g.spurious.push(entry.to_string());
        } else {
            g.declared.push(line.to_string());
        }
    }
    assert!(!g.declared.is_empty(), "empty golden in {root:?}");
    g
}

fn remove_one(entries: &mut Vec<String>, entry: &str) {
    let at = entries
        .iter()
        .position(|e| e == entry)
        .unwrap_or_else(|| panic!("declared limit `{entry}` is not part of the golden"));
    entries.remove(at);
}

fn check(name: &str) {
    let root = corpus_dir(name);
    let g = golden(&root);

    let mut expected = g.declared.clone();
    for entry in &g.missing {
        remove_one(&mut expected, entry);
    }
    expected.extend(g.spurious.iter().cloned());
    expected.sort();

    assert_eq!(
        read(&root),
        expected,
        "{name}: the reader and the hand-written golden disagree"
    );
}

#[test]
fn groovy_jenkins_reads_its_declarations() {
    check("groovy-jenkins");
}

#[test]
fn kotlin_reads_its_declarations() {
    check("kotlin");
}

#[test]
fn csharp_reads_its_declarations() {
    check("csharp");
}
