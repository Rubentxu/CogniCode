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
//!
//! The first line, `@language <id>`, names the language every file of the
//! corpus must be read as. `product/languages.json` takes its `support_level`
//! from that golden: `supported` when it declares no limit, `experimental`
//! when it does. The last test holds the published matrix to that rule.

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
    language: String,
    declared: Vec<String>,
    missing: Vec<String>,
    spurious: Vec<String>,
}

fn golden(root: &Path) -> Golden {
    let text = std::fs::read_to_string(root.join("symbols.golden")).unwrap();
    let mut g = Golden {
        language: String::new(),
        declared: Vec::new(),
        missing: Vec::new(),
        spurious: Vec::new(),
    };
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(id) = line.strip_prefix("@language ") {
            assert!(g.language.is_empty(), "two @language lines in {root:?}");
            g.language = id.to_string();
        } else if let Some(entry) = line.strip_prefix("!missing ") {
            g.missing.push(entry.to_string());
        } else if let Some(entry) = line.strip_prefix("!spurious ") {
            g.spurious.push(entry.to_string());
        } else {
            g.declared.push(line.to_string());
        }
    }
    assert!(!g.declared.is_empty(), "empty golden in {root:?}");
    assert!(!g.language.is_empty(), "no @language line in {root:?}");
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

    for file in sources(&root) {
        assert_eq!(
            Language::from_path(&file).unwrap().id(),
            g.language,
            "{name}: {file:?} is not a {} file",
            g.language
        );
    }

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

const CORPORA: [&str; 3] = ["groovy-jenkins", "kotlin", "csharp"];

#[test]
fn the_published_support_level_follows_the_golden() {
    let matrix: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../product/languages.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let languages = matrix["languages"].as_array().expect("a list of languages");

    let mut offenders = Vec::new();
    for name in CORPORA {
        let g = golden(&corpus_dir(name));
        let expected = if g.missing.is_empty() && g.spurious.is_empty() {
            "supported"
        } else {
            "experimental"
        };
        let published = languages
            .iter()
            .find(|entry| entry["id"] == g.language.as_str())
            .map(|entry| entry["support_level"].as_str().unwrap_or_default())
            .unwrap_or("absent");
        if published != expected {
            offenders.push(format!(
                "{}: published `{published}`, its golden says `{expected}`",
                g.language
            ));
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}
