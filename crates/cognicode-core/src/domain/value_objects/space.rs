//! `Space` — the federation unit. A named, typed collection of
//! graph data (a repo, a docs corpus, an issue tracker).
//!
//! Three values define a space: an opaque [`SpaceId`](super::space_id::SpaceId),
//! a human-readable `name`, and a [`SpaceKind`] discriminator. The
//! `source_path` and `config` fields are optional — a space can
//! exist purely in memory (e.g. an ad-hoc docs collection), and the
//! `config` JSON object holds extractor hints.
//!
//! The whole module is gated behind the `multimodal` Cargo feature
//! so the default build is byte-for-byte unchanged.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::space_id::SpaceId;

/// Three stable variants. The `Display` form (kebab-case lower) is
/// the wire-level discriminator persisted in PG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpaceKind {
    /// A source repository.
    Repo,
    /// A documentation corpus (markdown, ADRs, etc.).
    Docs,
    /// An issue tracker.
    Issues,
}

impl SpaceKind {
    /// The stable wire-level string. Used by the PG layer for the
    /// `kind` column and by the JSON schema for `node_kinds` filters.
    pub fn as_str(&self) -> &'static str {
        match self {
            SpaceKind::Repo => "Repo",
            SpaceKind::Docs => "Docs",
            SpaceKind::Issues => "Issues",
        }
    }

    /// Parse from the stable wire-level string. Returns `None` for
    /// unknown values.
    pub fn from_wire(s: &str) -> Option<Self> {
        match s {
            "Repo" => Some(SpaceKind::Repo),
            "Docs" => Some(SpaceKind::Docs),
            "Issues" => Some(SpaceKind::Issues),
            _ => None,
        }
    }
}

impl fmt::Display for SpaceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Errors raised by [`Space::try_new`] and [`SpaceId::try_new`].
///
/// All variants carry enough context to render a useful error
/// message; consumers should match on the discriminant, not the
/// `Display` form.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SpaceError {
    /// The supplied `SpaceId` string was empty (or whitespace-only).
    #[error("space id must be a non-empty string")]
    EmptyId,
    /// The supplied `Space.name` was empty.
    #[error("space name must be a non-empty string")]
    EmptyName,
    /// A space with the same id is already registered.
    #[error("duplicate space id: {0}")]
    Duplicate(String),
    /// The supplied `Space.kind` string did not match any known variant.
    #[error("invalid space kind: {0}")]
    InvalidKind(String),
}

/// A named, typed federation space.
///
/// Construct via [`Space::try_new`] to validate the invariants. The
/// `config` field defaults to an empty JSON object; supply your own
/// `serde_json::Value` when calling the struct constructor directly
/// if you need extractor-specific hints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Space {
    /// The opaque, unique space id.
    pub id: SpaceId,
    /// Human-readable name. NOT unique (only `id` is).
    pub name: String,
    /// The space kind discriminator.
    pub kind: SpaceKind,
    /// Optional source path / URL the space was loaded from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<PathBuf>,
    /// Free-form extractor hints (JSON object). Defaults to `{}`.
    #[serde(default = "default_config")]
    pub config: serde_json::Value,
}

fn default_config() -> serde_json::Value {
    serde_json::json!({})
}

impl Space {
    /// Construct a `Space` after validating the invariants.
    ///
    /// - `name` MUST be non-empty.
    /// - `config` defaults to an empty JSON object (`{}`).
    pub fn try_new(id: SpaceId, name: String, kind: SpaceKind) -> Result<Self, SpaceError> {
        if name.is_empty() {
            return Err(SpaceError::EmptyName);
        }
        Ok(Self {
            id,
            name,
            kind,
            source_path: None,
            config: default_config(),
        })
    }

    /// Builder: set the optional `source_path`.
    #[must_use]
    pub fn with_source_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Builder: replace the `config` field.
    #[must_use]
    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = config;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All three `SpaceKind` variants round-trip through JSON.
    #[test]
    fn space_kind_repo_docs_issues_roundtrip() {
        for kind in [SpaceKind::Repo, SpaceKind::Docs, SpaceKind::Issues] {
            let json = serde_json::to_string(&kind).expect("serialize");
            let parsed: SpaceKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(parsed, kind);
        }
    }

    /// `SpaceKind::as_str` and `Display` agree on the wire form.
    #[test]
    fn space_kind_as_str_matches_display() {
        for kind in [SpaceKind::Repo, SpaceKind::Docs, SpaceKind::Issues] {
            assert_eq!(kind.as_str(), format!("{kind}"));
        }
    }

    /// `SpaceKind::from_wire` accepts the canonical strings and
    /// rejects anything else.
    #[test]
    fn space_kind_from_wire_known_and_unknown() {
        assert_eq!(SpaceKind::from_wire("Repo"), Some(SpaceKind::Repo));
        assert_eq!(SpaceKind::from_wire("Docs"), Some(SpaceKind::Docs));
        assert_eq!(SpaceKind::from_wire("Issues"), Some(SpaceKind::Issues));
        assert_eq!(SpaceKind::from_wire("nope"), None);
    }

    /// `Space::try_new` accepts a non-empty name and defaults
    /// `config` to an empty JSON object.
    #[test]
    fn space_try_new_with_name_and_kind() {
        let space = Space::try_new(SpaceId::default(), "auth-repo".into(), SpaceKind::Repo)
            .expect("non-empty name must succeed");
        assert_eq!(space.id, SpaceId::default());
        assert_eq!(space.name, "auth-repo");
        assert_eq!(space.kind, SpaceKind::Repo);
        assert!(space.source_path.is_none());
        assert_eq!(space.config, serde_json::json!({}));
    }

    /// `Space::try_new` rejects an empty name.
    #[test]
    fn space_try_new_empty_name_returns_err() {
        let result = Space::try_new(SpaceId::default(), "".into(), SpaceKind::Repo);
        assert_eq!(result, Err(SpaceError::EmptyName));
    }

    /// `Space` serializes with the documented field set and a
    /// non-null default for `config`.
    #[test]
    fn space_try_new_defaults_config_to_empty_object() {
        let space = Space::try_new(SpaceId::default(), "n".into(), SpaceKind::Docs).unwrap();
        // The config field defaults to an empty object, not null.
        assert_eq!(space.config, serde_json::json!({}));
        // And the field appears in the JSON.
        let json = serde_json::to_value(&space).expect("serialize");
        assert_eq!(json["config"], serde_json::json!({}));
        assert!(json["source_path"].is_null());
    }
}
