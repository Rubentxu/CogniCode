//! `interface/goals` and `interface/mcp` must not depend on `interface/cli`.
//!
//! The CLI is the part of the interface that leaves the core together with
//! `clap`. Anything the goals or the MCP adapter still need from it has to live
//! in a neutral place, otherwise moving the CLI drags them along.
//! The published `interface::cli::*` paths stay as re-exports.

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

/// Production lines that name the CLI module: stops at the first `#[cfg(test)]`
/// and ignores comments.
fn cli_mentions(text: &str) -> Vec<String> {
    text.lines()
        .take_while(|line| !line.trim_start().starts_with("#[cfg(test)]"))
        .filter(|line| !line.trim_start().starts_with("//"))
        .filter(|line| line.contains("interface::cli"))
        .map(|line| line.trim().to_string())
        .collect()
}

#[test]
fn the_scan_sees_a_cli_mention_and_skips_comments_and_tests() {
    assert_eq!(cli_mentions("use crate::interface::cli::x;\n").len(), 1);
    assert!(cli_mentions("// crate::interface::cli::x\n").is_empty());
    assert!(cli_mentions("#[cfg(test)]\nuse crate::interface::cli::x;\n").is_empty());
}

#[test]
fn goals_and_mcp_do_not_name_the_cli() {
    let interface = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/interface");
    let mut files = vec![interface.join("goals.rs")];
    rust_files(&interface.join("goals"), &mut files);
    rust_files(&interface.join("mcp"), &mut files);
    assert!(
        files.len() > 10,
        "the scan reached too few files: {}",
        files.len()
    );
    let mut offenders = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("readable source");
        for line in cli_mentions(&text) {
            offenders.push(format!(
                "{}: {line}",
                file.strip_prefix(&interface).unwrap().display()
            ));
        }
    }
    assert!(
        offenders.is_empty(),
        "dependen de interface::cli: {offenders:#?}"
    );
}
