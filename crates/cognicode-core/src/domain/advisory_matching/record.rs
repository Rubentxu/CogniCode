//! The part of an OSV advisory record that matching reads.
//!
//! A record is data, never instructions: it is read into these plain values
//! and nothing in it is executed, fetched or followed (references, URLs and
//! free text the record carries are not kept).

/// The OSV ecosystem name of crates.io.
pub const CRATES_IO: &str = "crates.io";

/// The kind of a version range, as the record declares it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeKind {
    /// Semantic Versioning ordering.
    Semver,
    /// The ecosystem's own ordering. For crates.io that is Semantic
    /// Versioning, so it is read the same way.
    Ecosystem,
    /// Commit ancestry in a repository: it cannot be decided from a version.
    Git,
    /// A kind this reader does not know.
    Other(String),
}

/// One event of a version range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeEvent {
    /// The first affected version (`0` means "from the beginning").
    Introduced(String),
    /// The first version that is no longer affected.
    Fixed(String),
    /// The last affected version, inclusive.
    LastAffected(String),
    /// A limit, which only has a meaning for Git ranges.
    Limit(String),
}

/// A range of affected versions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisoryRange {
    /// How the events are ordered.
    pub kind: RangeKind,
    /// The events, or `None` when the record's events are not in the shape
    /// the OSV schema defines.
    pub events: Option<Vec<RangeEvent>>,
}

/// The package an `affected` entry names.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AffectedPackage {
    /// The OSV ecosystem (`crates.io`).
    pub ecosystem: Option<String>,
    /// The package name inside the ecosystem.
    pub name: Option<String>,
    /// The Package URL, when the record gives one.
    pub purl: Option<String>,
}

/// One `affected` entry: a package with the versions it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedEntry {
    /// The package.
    pub package: AffectedPackage,
    /// Versions the record lists one by one.
    pub versions: Vec<String>,
    /// Version ranges.
    pub ranges: Vec<AdvisoryRange>,
}

/// An advisory record that passed every limit and check of the reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisoryRecord {
    /// The record identifier (`RUSTSEC-2024-0001`, `GHSA-...`).
    pub id: String,
    /// Other identifiers of the same advisory (CVE, GHSA, RUSTSEC).
    pub aliases: Vec<String>,
    /// When the record was last modified, as the record writes it.
    pub modified: String,
    /// The same instant as seconds since the Unix epoch.
    pub modified_unix: i64,
    /// A short, sanitised, length-limited summary.
    pub summary: Option<String>,
    /// What the record says is affected.
    pub affected: Vec<AffectedEntry>,
}

/// The form of a crate name that Cargo treats as one name: lowercase, with
/// `_` and `-` equal. crates.io refuses two names that differ only that way.
pub fn canonical_crate_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c == '_' {
                '-'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

impl AffectedEntry {
    /// The canonical name of the crate this entry covers, when it names one
    /// on crates.io (by ecosystem and name, or by a `pkg:cargo` Package URL).
    pub fn cargo_crate(&self) -> Option<String> {
        let package = &self.package;
        if package.ecosystem.as_deref() == Some(CRATES_IO) {
            if let Some(name) = &package.name {
                return Some(canonical_crate_name(name));
            }
        }
        let rest = package.purl.as_deref()?.strip_prefix("pkg:cargo/")?;
        let name = rest.split(['@', '?', '#']).next()?;
        (!name.is_empty() && !name.contains('/')).then(|| canonical_crate_name(name))
    }
}

impl AdvisoryRecord {
    /// Whether any `affected` entry covers a crates.io crate.
    pub fn affects_cargo(&self) -> bool {
        self.affected
            .iter()
            .any(|entry| entry.cargo_crate().is_some())
    }
}
