//! What the analyzer is built from, and the digest that identifies it.
//!
//! The build script and the test that checks it include this same file, so
//! the list of inputs and the way they are hashed have a single owner.
//!
//! The paths are relative to the `cognicode-core` package directory. In the
//! workspace they reach the sibling crates the analyzer is compiled with; in a
//! packaged crate (`cargo package`, a registry download) the siblings are not
//! there, and the digest records them as absent instead of failing.

use std::io;
use std::path::Path;

use sha2::{Digest, Sha256};

/// The inputs whose bytes decide what the analyzer computes.
///
/// A directory stands for every file below it. Every entry exists in the
/// workspace checkout, and a contract keeps it so: Cargo reruns a build
/// script on every build when a path it watches does not exist.
pub const INPUTS: &[&str] = &[
    "Cargo.toml",
    "build.rs",
    "analyzer_source.rs",
    "product",
    "src",
    "../../Cargo.toml",
    "../../Cargo.lock",
    "../cognicode-graph-algos/Cargo.toml",
    "../cognicode-graph-algos/src",
    "../cognicode-macros/Cargo.toml",
    "../cognicode-macros/src",
];

/// One file the analyzer is built from, or an input that is not there.
pub struct Entry {
    /// The path relative to the package directory, with `/` separators.
    pub path: String,
    /// The bytes of the file; `None` when the input does not exist.
    pub bytes: Option<Vec<u8>>,
}

/// Reads every input below `package_dir`.
///
/// A missing input is an [`Entry`] without bytes. A file that exists but
/// cannot be read is an error: hashing around it would publish an identity
/// for a tree nobody read.
pub fn collect(package_dir: &Path) -> io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for input in INPUTS {
        let path = package_dir.join(input);
        if path.exists() {
            walk(&path, input, &mut entries)?;
        } else {
            entries.push(Entry {
                path: (*input).to_string(),
                bytes: None,
            });
        }
    }
    Ok(entries)
}

fn walk(path: &Path, relative: &str, entries: &mut Vec<Entry>) -> io::Result<()> {
    if path.is_dir() {
        for child in std::fs::read_dir(path)? {
            let child = child?;
            let name = child.file_name();
            let name = name.to_str().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{}: a file name that is not UTF-8", child.path().display()),
                )
            })?;
            walk(&child.path(), &format!("{relative}/{name}"), entries)?;
        }
    } else {
        entries.push(Entry {
            path: relative.to_string(),
            bytes: Some(std::fs::read(path)?),
        });
    }
    Ok(())
}

/// The sha256 of a tree, in lowercase hex.
///
/// The digest depends on the files and not on the order they were read in.
/// Each file contributes its path, whether it exists, its length and its
/// bytes, so neither a rename nor a byte moved across two files can produce
/// the same digest.
pub fn digest(mut entries: Vec<Entry>) -> String {
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let mut hasher = Sha256::new();
    for entry in &entries {
        hasher.update(entry.path.as_bytes());
        hasher.update([0u8]);
        match &entry.bytes {
            Some(bytes) => {
                hasher.update([1u8]);
                hasher.update((bytes.len() as u64).to_le_bytes());
                hasher.update(bytes);
            }
            None => hasher.update([0u8]),
        }
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
