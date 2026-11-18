//! A reader of `Cargo.lock`: the RESOLVED layer of a Cargo workspace.
//!
//! Reading a lockfile is a pure function of its bytes. It runs no build
//! script, resolves nothing, downloads nothing and shells out to nothing;
//! what it cannot say (features, dev/build kinds of transitive edges, target
//! conditions of edges between registry crates) is absent from the model
//! rather than guessed.

use std::collections::BTreeMap;

use super::source::PackageSource;

/// A dependency edge as a lockfile writes it: `name`, `name version` or
/// `name version (source)`; the longer forms appear only when the shorter
/// would be ambiguous.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockDependencyRef {
    /// The package name.
    pub name: String,
    /// The version, when the lockfile disambiguates by it.
    pub version: Option<String>,
    /// The source, when the lockfile disambiguates by it.
    pub source: Option<String>,
}

impl LockDependencyRef {
    fn parse(text: &str) -> Self {
        let mut words = text.splitn(3, ' ');
        let name = words.next().unwrap_or_default().to_string();
        let version = words.next().map(str::to_string);
        let source = words.next().map(|rest| {
            rest.trim_start_matches('(')
                .trim_end_matches(')')
                .to_string()
        });
        Self {
            name,
            version,
            source,
        }
    }
}

/// One `[[package]]` of the lockfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedPackage {
    /// The package name.
    pub name: String,
    /// The locked version.
    pub version: String,
    /// Where the package comes from.
    pub source: PackageSource,
    /// The registry checksum (SHA-256 hex) the lockfile records, if any.
    pub checksum: Option<String>,
    /// The edges the lockfile records for this package.
    pub dependencies: Vec<LockDependencyRef>,
}

/// A parsed `Cargo.lock`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoLock {
    /// The lockfile format version; `None` for the legacy format that has no
    /// `version` key.
    pub format_version: Option<i64>,
    /// The packages, in lockfile order.
    pub packages: Vec<LockedPackage>,
}

/// Why a lockfile could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LockParseError {
    /// The bytes are not UTF-8 TOML.
    #[error("Cargo.lock is not valid TOML: {0}")]
    Toml(String),
    /// A `[[package]]` lacks `name` or `version`.
    #[error("Cargo.lock package #{index} has no {field}")]
    MissingField {
        /// Position of the package in the file.
        index: usize,
        /// The missing key.
        field: &'static str,
    },
}

impl CargoLock {
    /// Reads a lockfile from its bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self, LockParseError> {
        let text =
            std::str::from_utf8(bytes).map_err(|error| LockParseError::Toml(error.to_string()))?;
        let document: toml::Table =
            toml::from_str(text).map_err(|error| LockParseError::Toml(error.to_string()))?;
        let legacy_checksums = legacy_checksums(&document);
        let entries = document
            .get("package")
            .and_then(toml::Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut packages = Vec::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            let field = |key: &'static str| {
                entry
                    .get(key)
                    .and_then(toml::Value::as_str)
                    .ok_or(LockParseError::MissingField { index, field: key })
            };
            let name = field("name")?.to_string();
            let version = field("version")?.to_string();
            let raw_source = entry.get("source").and_then(toml::Value::as_str);
            let source = PackageSource::from_lock(raw_source);
            let checksum = entry
                .get("checksum")
                .and_then(toml::Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    legacy_checksums
                        .get(&(
                            name.clone(),
                            version.clone(),
                            raw_source.map(str::to_string),
                        ))
                        .cloned()
                });
            let dependencies = entry
                .get("dependencies")
                .and_then(toml::Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .map(LockDependencyRef::parse)
                        .collect()
                })
                .unwrap_or_default();
            packages.push(LockedPackage {
                name,
                version,
                source,
                checksum,
                dependencies,
            });
        }
        Ok(Self {
            format_version: document.get("version").and_then(toml::Value::as_integer),
            packages,
        })
    }
}

type LegacyKey = (String, String, Option<String>);

/// The checksums of a format-1 lockfile, which keeps them in a `[metadata]`
/// table keyed `checksum <name> <version> (<source>)`.
fn legacy_checksums(document: &toml::Table) -> BTreeMap<LegacyKey, String> {
    let mut checksums = BTreeMap::new();
    let Some(metadata) = document.get("metadata").and_then(toml::Value::as_table) else {
        return checksums;
    };
    for (key, value) in metadata {
        let (Some(rest), Some(checksum)) = (key.strip_prefix("checksum "), value.as_str()) else {
            continue;
        };
        let reference = LockDependencyRef::parse(rest);
        if let Some(version) = reference.version {
            checksums.insert(
                (reference.name, version, reference.source),
                checksum.to_string(),
            );
        }
    }
    checksums
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"
version = 4

[[package]]
name = "app"
version = "0.1.0"
dependencies = [
 "bitflags 1.3.2",
 "bitflags 2.13.2",
 "util",
]

[[package]]
name = "bitflags"
version = "1.3.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "bef38d45163c2f1dde094a7dfd33ccf595c92b8c8f1d4fe9d0ab1e0e4f4f4f4f"

[[package]]
name = "bitflags"
version = "2.13.2"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "0000000000000000000000000000000000000000000000000000000000000002"

[[package]]
name = "gitdep"
version = "0.3.0"
source = "git+https://example.test/gitdep?rev=abc1234#abc1234abc1234abc1234abc1234abc1234abc1"

[[package]]
name = "util"
version = "0.1.0"
dependencies = [
 "gitdep",
 "other 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)",
]
"#;

    #[test]
    fn packages_carry_version_source_and_checksum() {
        let lock = CargoLock::parse(LOCK.as_bytes()).expect("a valid lockfile");
        assert_eq!(lock.format_version, Some(4));
        assert_eq!(lock.packages.len(), 5);
        let bitflags = &lock.packages[2];
        assert_eq!(
            (bitflags.name.as_str(), bitflags.version.as_str()),
            ("bitflags", "2.13.2")
        );
        assert!(bitflags.source.is_default_registry());
        assert!(
            bitflags
                .checksum
                .as_deref()
                .is_some_and(|sum| sum.ends_with('2'))
        );
        assert_eq!(lock.packages[0].source, PackageSource::Path);
        assert_eq!(lock.packages[0].checksum, None);
        assert_eq!(lock.packages[3].source.kind(), "git");
    }

    #[test]
    fn edges_keep_the_disambiguation_the_lockfile_wrote() {
        let lock = CargoLock::parse(LOCK.as_bytes()).expect("a valid lockfile");
        let app = &lock.packages[0].dependencies;
        assert_eq!(app[0].name, "bitflags");
        assert_eq!(app[0].version.as_deref(), Some("1.3.2"));
        assert_eq!(app[2].version, None);
        let util = &lock.packages[4].dependencies;
        assert_eq!(
            util[1].source.as_deref(),
            Some("registry+https://github.com/rust-lang/crates.io-index")
        );
    }

    #[test]
    fn a_legacy_lockfile_keeps_its_checksums_in_the_metadata_table() {
        let legacy = r#"
[[package]]
name = "itoa"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[metadata]
"checksum itoa 1.0.0 (registry+https://github.com/rust-lang/crates.io-index)" = "abc123"
"#;
        let lock = CargoLock::parse(legacy.as_bytes()).expect("a legacy lockfile");
        assert_eq!(lock.format_version, None);
        assert_eq!(lock.packages[0].checksum.as_deref(), Some("abc123"));
    }

    #[test]
    fn a_broken_lockfile_is_an_error_never_an_empty_inventory() {
        assert!(matches!(
            CargoLock::parse(b"[[package]\nname ="),
            Err(LockParseError::Toml(_))
        ));
        assert!(matches!(
            CargoLock::parse(b"[[package]]\nname = \"x\"\n"),
            Err(LockParseError::MissingField {
                index: 0,
                field: "version"
            })
        ));
        assert!(matches!(
            CargoLock::parse(&[0xff, 0xfe]),
            Err(LockParseError::Toml(_))
        ));
    }
}
