//! Namespaced relation vocabulary for kernel facts.
//!
//! A `RelationKind` is `"ns:name"` — the namespace groups vocabularies
//! (e.g. `core`, `callgraph`, `semantic`), the name is the predicate.
//! Membership is governed by the `SchemaRegistry` port (design D6): kernel
//! fact commits MUST reject unregistered predicates.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A namespaced relation predicate (`"ns:name"`).
///
/// Constructed only through [`RelationKind::try_new`], which enforces the
/// `ns:name` shape: exactly one `:` separator, non-empty namespace and
/// non-empty predicate name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RelationKind(String);

impl RelationKind {
    /// Constructs a namespaced relation kind, validating the `"ns:name"` shape.
    pub fn try_new(s: impl Into<String>) -> Result<Self, RelationKindError> {
        let s = s.into();
        if s.is_empty() {
            return Err(RelationKindError::Empty);
        }
        let mut parts = s.split(':');
        let ns = parts.next().unwrap_or_default();
        let name = match (parts.next(), parts.next()) {
            (Some(name), None) => name,
            (None, _) => return Err(RelationKindError::MissingSeparator),
            (Some(_), Some(_)) => return Err(RelationKindError::MultipleSeparators),
        };
        if ns.is_empty() || name.is_empty() {
            return Err(RelationKindError::EmptySegment);
        }
        Ok(Self(s))
    }

    /// The namespace (the part before `:`).
    pub fn ns(&self) -> &str {
        self.0.split_once(':').map_or("", |(ns, _)| ns)
    }

    /// The canonical string form (`"ns:name"`).
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RelationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Error type for [`RelationKind::try_new`] failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RelationKindError {
    /// The input string was empty.
    #[error("relation kind cannot be empty")]
    Empty,
    /// The input had no `:` separator.
    #[error("relation kind must be namespaced as 'ns:name' (missing ':')")]
    MissingSeparator,
    /// The input had more than one `:` separator.
    #[error("relation kind must contain exactly one ':' separator")]
    MultipleSeparators,
    /// The namespace or the predicate name was empty.
    #[error("relation kind namespace and name must be non-empty")]
    EmptySegment,
}

/// Describes a registered relation predicate in the schema vocabulary.
///
/// Minimal M1 shape (design D6): a human-readable description. The registry
/// port (`evidence_kernel::ports::SchemaRegistry`) governs membership;
/// extensions (subject/object value constraints) are future work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationSpec {
    /// Human-readable description of the predicate's meaning.
    pub description: String,
}

impl RelationSpec {
    /// Constructs a spec with the given description.
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            description: description.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 3.1 RED — namespaced RelationKind (`"ns:name"`)
    // -------------------------------------------------------------------------

    /// A valid `"ns:name"` must expose ns part, canonical form and Display
    /// form (E38.1 U5 trim: the unused per-predicate `name()` accessor was
    /// removed — the canonical string carries it).
    #[test]
    fn relation_kind_valid_ns_name() {
        let k = RelationKind::try_new("core:calls").expect("valid namespaced kind");
        assert_eq!(k.as_str(), "core:calls");
        assert_eq!(k.ns(), "core");
        assert_eq!(k.to_string(), "core:calls");
    }

    /// Malformed kinds must be rejected with the specific error.
    #[test]
    fn relation_kind_rejects_malformed() {
        assert_eq!(RelationKind::try_new(""), Err(RelationKindError::Empty));
        assert_eq!(
            RelationKind::try_new("calls"),
            Err(RelationKindError::MissingSeparator)
        );
        assert_eq!(
            RelationKind::try_new("a:b:c"),
            Err(RelationKindError::MultipleSeparators)
        );
        assert_eq!(
            RelationKind::try_new(":calls"),
            Err(RelationKindError::EmptySegment)
        );
        assert_eq!(
            RelationKind::try_new("core:"),
            Err(RelationKindError::EmptySegment)
        );
    }

    /// Kinds with equal strings are equal; different namespaces differ.
    #[test]
    fn relation_kind_equality() {
        let a = RelationKind::try_new("core:calls").expect("valid");
        let b = RelationKind::try_new("core:calls").expect("valid");
        let c = RelationKind::try_new("semantic:calls").expect("valid");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    /// `RelationKind` must serialize and deserialize losslessly via serde.
    #[test]
    fn relation_kind_serde_round_trip() {
        let k = RelationKind::try_new("callgraph:invokes").expect("valid");
        let json = serde_json::to_string(&k).expect("serialize");
        let parsed: RelationKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, k);
    }

    /// `RelationSpec` must carry its description through serde.
    #[test]
    fn relation_spec_serde_round_trip() {
        let spec = RelationSpec::new("direct call edge extracted from source");
        let json = serde_json::to_string(&spec).expect("serialize");
        let parsed: RelationSpec = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, spec);
        assert_eq!(parsed.description, "direct call edge extracted from source");
    }
}
