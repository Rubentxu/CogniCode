//! `interface/goals` and `interface/hateoas` must not depend on `interface/mcp`.
//!
//! The MCP adapter is the part of the interface that leaves the core together
//! with `rmcp`. The goal registry and the response envelope are shared by every
//! surface, so they cannot name it: anything they still need from it has to
//! live in a neutral place, otherwise moving the adapter drags them along.

use std::fs;
use std::path::{Path, PathBuf};

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

/// Production lines that name the MCP module: stops at the first
/// `#[cfg(test)]` and ignores comments.
fn mcp_mentions(text: &str) -> Vec<String> {
    text.lines()
        .take_while(|line| !line.trim_start().starts_with("#[cfg(test)]"))
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("interface::mcp") || line.contains("super::mcp"))
        .map(|line| line.trim().to_string())
        .collect()
}

#[test]
fn the_scan_sees_an_mcp_mention_and_skips_comments_and_tests() {
    assert_eq!(mcp_mentions("use crate::interface::mcp::x;\n").len(), 1);
    assert!(mcp_mentions("// crate::interface::mcp::x\n").is_empty());
    assert!(mcp_mentions("#[cfg(test)]\nuse crate::interface::mcp::x;\n").is_empty());
}

#[test]
fn goals_and_hateoas_do_not_name_the_mcp_adapter() {
    let interface = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/interface");
    let mut files = vec![interface.join("goals.rs"), interface.join("hateoas.rs")];
    rust_files(&interface.join("goals"), &mut files);
    rust_files(&interface.join("hateoas"), &mut files);
    assert!(
        files.len() > 10,
        "the scan reached too few files: {}",
        files.len()
    );
    let mut offenders = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("readable source");
        for line in mcp_mentions(&text) {
            offenders.push(format!(
                "{}: {line}",
                file.strip_prefix(&interface).unwrap().display()
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "dependen de interface::mcp: {offenders:#?}"
    );
}
