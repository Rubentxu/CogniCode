//! Reading the tree's own source, to ask it structural questions.
//!
//! Some contracts are about the SHAPE of the code rather than about what
//! it computes: «the surfaces call the owner», «the copy that had no
//! caller is gone». Those are answered by reading the sources, and the
//! reading has to be careful about two things:
//!
//! - a name mentioned in prose is a name somebody wrote about; a name
//!   DECLARED is a name that exists. Only the second one is the defect
//!   this kind of scan looks for;
//! - a scan that answers «yes» to everything proves nothing, so it has to
//!   be able to say «yes» and «no» both.
//!
//! Doing it here once keeps two contracts from growing their own copy of
//! the same scan — and keeps them from drifting apart, which is how two
//! scans end up looking for different things under the same name.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// The root of the workspace crates.
pub fn crates_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("crates")
}

/// Every Rust source file of the workspace crates, with its path relative to
/// the repository root.
pub fn rust_sources() -> Vec<(String, String)> {
    let mut pending = vec![crates_root()];
    let mut found = Vec::new();
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let relative = path
                .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));
            found.push((relative, content));
        }
    }
    found
}

/// Whether `text` carries `token` as a whole word.
///
/// The boundary is the ASCII word character: an identifier is a run of
/// word characters, and a token that touches one of them is part of a
/// longer name, not the name itself.
pub fn contains_word(text: &str, token: &str) -> bool {
    let bytes = text.as_bytes();
    let token_bytes = token.as_bytes();
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut offset = 0;
    while let Some(found) = text[offset..].find(token) {
        let at = offset + found;
        let before_ok = at == 0 || !is_word(bytes[at - 1]);
        let after = at + token_bytes.len();
        let after_ok = after >= bytes.len() || !is_word(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
        offset = at + 1;
    }
    false
}

/// Whether `text` DECLARES `token`, ignoring comments.
///
/// A name mentioned in prose is a name somebody wrote about; a name
/// declared is a name that exists. Only the second one is the defect
/// this scan looks for, and the distinction matters: a file that
/// explains why a type is gone would otherwise be reported as
/// bringing it back.
pub fn declares_word(text: &str, token: &str) -> bool {
    let mut in_block_comment = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if in_block_comment {
            match trimmed.find("*/") {
                Some(end) => {
                    in_block_comment = false;
                    if contains_word(&trimmed[end + 2..], token) {
                        return true;
                    }
                }
                None => continue,
            }
            continue;
        }
        if trimmed.starts_with("/*") {
            match trimmed.find("*/") {
                Some(end) => {
                    if contains_word(&trimmed[end + 2..], token) {
                        return true;
                    }
                }
                None => in_block_comment = true,
            }
            continue;
        }
        if trimmed.starts_with("//") {
            continue;
        }
        if contains_word(line, token) {
            return true;
        }
    }
    false
}

/// The content of one file of the tree, by its path relative to the
/// repository root.
pub fn source_of(path: &str) -> String {
    rust_sources()
        .into_iter()
        .find(|(relative, _)| relative == path)
        .map(|(_, content)| content)
        .unwrap_or_else(|| panic!("{path} is part of the tree"))
}
