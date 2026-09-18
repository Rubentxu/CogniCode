//! Shared namespaced-name semantics.
//!
//! **Ungated domain vocabulary (cycle e63).** This used to live in the M6
//! findings module; the M7 Intelligence Event Log needs the same grammar for
//! `EventKind`, and two validators would eventually disagree, so it moved
//! here — the same move e56 made for the kernel ids. The findings module
//! re-exports it, so every existing `findings::namespaced::*` path resolves.
//!
//! Canonical syntax is **`namespace.name`** (dot-separated), matching the
//! ANALYSIS-ENGINE examples (`security.sql_injection`,
//! `architecture.layer_violation`). This is the single validator behind
//! `SubjectPattern`, `FindingKind`, `DetectorId` and now `EventKind`, so they
//! cannot drift apart.
//!
//! ## Known divergence (tracked)
//!
//! The evidence kernel's `RelationKind` uses the older `ns:name` (colon)
//! form behind the `evidence-kernel` feature. The storage layer may
//! translate internally; unifying the two on one canonical syntax is a
//! follow-up (see `openspec/changes/e55-lsi-m6-contract-hardening/design.md`).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Why a namespaced name was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamespacedError {
    /// The value has no `.` separator.
    MissingSeparator,
    /// The value has an empty segment (leading/trailing/double separator).
    EmptySegment,
}

impl fmt::Display for NamespacedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MissingSeparator => "expected at least one '.' separator (namespace.name)",
            Self::EmptySegment => "namespace segments must not be empty",
        })
    }
}

impl std::error::Error for NamespacedError {}

/// Sanitize an arbitrary label into a valid, non-empty `ns.name` segment.
///
/// Lowercases, maps non-alphanumerics to `_`, collapses runs, trims
/// leading/trailing `_`. Falls back to `uncategorized`.
pub fn sanitize_segment(input: &str) -> String {
    let mut out = String::new();
    let mut prev_underscore = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_underscore = false;
        } else if !prev_underscore && !out.is_empty() {
            out.push('_');
            prev_underscore = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "uncategorized".to_string()
    } else {
        out
    }
}

/// A validated `namespace.name` identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NamespacedName(String);

impl NamespacedName {
    /// Validate and construct a namespaced name.
    pub fn new(value: impl Into<String>) -> Result<Self, NamespacedError> {
        let value = value.into();
        validate(&value)?;
        Ok(Self(value))
    }

    /// Borrow the raw value.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The namespace segment (before the first `.`).
    pub fn namespace(&self) -> &str {
        self.0.split('.').next().unwrap_or("")
    }

    /// The local name (everything after the first `.`).
    pub fn name(&self) -> &str {
        self.0.split_once('.').map(|x| x.1).unwrap_or("")
    }
}

/// Validate the canonical `namespace.name` grammar.
pub fn validate(value: &str) -> Result<(), NamespacedError> {
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() < 2 {
        return Err(NamespacedError::MissingSeparator);
    }
    if parts.iter().any(|segment| segment.is_empty()) {
        return Err(NamespacedError::EmptySegment);
    }
    Ok(())
}

impl TryFrom<String> for NamespacedName {
    type Error = NamespacedError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<NamespacedName> for String {
    fn from(value: NamespacedName) -> Self {
        value.0
    }
}

impl fmt::Display for NamespacedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_forms() {
        for ok in [
            "security.sql_injection",
            "architecture.layer_violation",
            "company.foo.auth_guard",
            "a.b",
        ] {
            let name = NamespacedName::new(ok).unwrap_or_else(|e| panic!("{ok}: {e}"));
            assert_eq!(name.as_str(), ok);
        }
    }

    #[test]
    fn rejects_malformed() {
        for bad in ["", "nons", "ns.", ".name", "a..b", "a.b.", "  ", "a:b"] {
            assert!(
                NamespacedName::new(bad).is_err(),
                "`{bad}` must be rejected"
            );
        }
    }

    #[test]
    fn splits_namespace_and_name() {
        let name = NamespacedName::new("security.sql_injection").unwrap();
        assert_eq!(name.namespace(), "security");
        assert_eq!(name.name(), "sql_injection");
    }

    #[test]
    fn round_trips_serde() {
        let name = NamespacedName::new("security.sql_injection").unwrap();
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"security.sql_injection\"");
        let parsed: NamespacedName = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, name);
    }

    #[test]
    fn serde_rejects_malformed() {
        assert!(serde_json::from_str::<NamespacedName>("\"nons\"").is_err());
    }
}
