//! `FederatedNodeId` — the wire-level id for a node in a
//! multi-space federation.
//!
//! Format: `"{space_id}::{local_id}"`. The `::` separator is
//! reserved (rejected at [`FederatedNodeId::try_new`]) and does
//! NOT appear in any of the underlying `NodeId` shapes
//! (`file:name:line`, `doc:path#slug`, `issue:tracker#num`,
//! `ev:sha256`). Parsing is a single `split_once("::")` call —
//! zero allocation on the hot path.
//!
//! The `NodeId` type itself is unchanged. The space prefix lives
//! in this wrapper, so the Generic Graph Model stays backward
//! compatible: every pre-federation `NodeId` is still a valid
//! `local_id` (just without a space prefix).
//!
//! Gated behind the `multimodal` Cargo feature. Default builds do
//! not include this module.

use std::fmt;

use cognicode_core::domain::value_objects::SpaceId;
use serde::{Deserialize, Serialize};

/// Error type for [`FederatedNodeId::try_new`] failures.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum FederatedNodeIdError {
    /// The supplied string did not contain the `::` separator.
    #[error("federated id must contain `::` separator: {0}")]
    MissingSeparator(String),
    /// The space-id segment (left of `::`) was empty.
    #[error("federated id space segment must be non-empty: {0}")]
    EmptySpaceSegment(String),
    /// The local-id segment (right of `::`) was empty.
    #[error("federated id local segment must be non-empty: {0}")]
    EmptyLocalSegment(String),
    /// The supplied string contains more than one `::` separator.
    #[error("federated id must contain exactly one `::` separator: {0}")]
    MultipleSeparators(String),
    /// The supplied string contains a forbidden `::` inside one
    /// of the two segments.
    #[error("federated id segments must not contain `::`: {0}")]
    ForbiddenSubstring(String),
}

/// Wire-level federated node id: `"{space_id}::{local_id}"`.
///
/// The inner `String` is the canonical wire form. Parse it back
/// into a `FederatedNodeId` via [`FederatedNodeId::try_new`] to
/// guarantee the invariants hold.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FederatedNodeId(pub String);

impl FederatedNodeId {
    /// The reserved separator. Guaranteed to be rejected inside
    /// either segment.
    pub const SEPARATOR: &'static str = "::";

    /// Construct a `FederatedNodeId` from a wire string. The
    /// supplied value MUST contain exactly one `::` separator with
    /// non-empty segments on both sides.
    ///
    /// ```text
    /// FederatedNodeId::try_new("auth::file.rs:main:1")  -> Ok
    /// FederatedNodeId::try_new("missing_separator")     -> Err(MissingSeparator)
    /// FederatedNodeId::try_new("::local")              -> Err(EmptySpaceSegment)
    /// FederatedNodeId::try_new("space::")              -> Err(EmptyLocalSegment)
    /// FederatedNodeId::try_new("a::b::c")              -> Err(MultipleSeparators)
    /// ```
    pub fn try_new(s: impl Into<String>) -> Result<Self, FederatedNodeIdError> {
        let inner = s.into();
        // Count separators — must be exactly one.
        let occurrences: Vec<_> = inner.match_indices(Self::SEPARATOR).collect();
        match occurrences.len() {
            0 => return Err(FederatedNodeIdError::MissingSeparator(inner)),
            1 => {} // exactly one — proceed
            _ => return Err(FederatedNodeIdError::MultipleSeparators(inner)),
        }
        let (space, local) = inner.split_once(Self::SEPARATOR).expect("one separator present");
        if space.is_empty() {
            return Err(FederatedNodeIdError::EmptySpaceSegment(inner));
        }
        if local.is_empty() {
            return Err(FederatedNodeIdError::EmptyLocalSegment(inner));
        }
        // The two halves are guaranteed to be free of `::` because
        // split_once splits on the FIRST occurrence; if either half
        // contained `::`, the count above would be > 1. Re-check
        // defensively in case the string ends with a `::` (split
        // returns the left half without the separator, so it's safe
        // for the LEFT side, but the RIGHT side is everything after
        // the FIRST `::`, so it COULD contain another `::`).
        if local.contains(Self::SEPARATOR) {
            return Err(FederatedNodeIdError::MultipleSeparators(inner));
        }
        Ok(Self(inner))
    }

    /// Construct from a `SpaceId` and a `local_id` string. The
    /// `local_id` MUST NOT contain the `::` separator (the space
    /// id is rejected at its own constructor).
    pub fn from_parts(space_id: &SpaceId, local_id: &str) -> Result<Self, FederatedNodeIdError> {
        if local_id.is_empty() {
            return Err(FederatedNodeIdError::EmptyLocalSegment(format!(
                "{}{}{}",
                space_id.as_str(),
                Self::SEPARATOR,
                local_id
            )));
        }
        if local_id.contains(Self::SEPARATOR) {
            return Err(FederatedNodeIdError::ForbiddenSubstring(format!(
                "{}{}{}",
                space_id.as_str(),
                Self::SEPARATOR,
                local_id
            )));
        }
        Ok(Self(format!(
            "{}{}{}",
            space_id.as_str(),
            Self::SEPARATOR,
            local_id
        )))
    }

    /// The space-id segment (left of `::`).
    pub fn space_id_str(&self) -> &str {
        // SAFETY: any FederatedNodeId is constructed with exactly
        // one `::` separator; split_once always succeeds.
        self.0.split_once(Self::SEPARATOR).unwrap().0
    }

    /// The local-id segment (right of `::`).
    pub fn local_id_str(&self) -> &str {
        // SAFETY: same as space_id_str.
        self.0.split_once(Self::SEPARATOR).unwrap().1
    }

    /// Parse the left half as a `SpaceId`. The `SpaceId` newtype
    /// is the same opaque string; this helper just unwraps it
    /// ergonomically.
    pub fn space_id(&self) -> SpaceId {
        // Unchecked `from` is safe: try_new guarantees the left
        // half is non-empty (and SpaceId::try_new would also
        // accept it; the whitespace check fails for empty strings
        // only, which we already excluded).
        SpaceId::from(self.space_id_str().to_string())
    }

    /// Borrow the inner wire string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the inner `String`.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for FederatedNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A well-formed `space::local` string is accepted.
    #[test]
    fn federated_node_id_try_new_valid_format_succeeds() {
        let id = FederatedNodeId::try_new("auth::file.rs:main:1").expect("valid");
        assert_eq!(id.as_str(), "auth::file.rs:main:1");
        assert_eq!(id.space_id_str(), "auth");
        assert_eq!(id.local_id_str(), "file.rs:main:1");
    }

    /// Missing `::` separator is rejected.
    #[test]
    fn federated_node_id_try_new_missing_separator_returns_err() {
        let result = FederatedNodeId::try_new("no_separator_here");
        assert!(matches!(
            result,
            Err(FederatedNodeIdError::MissingSeparator(_))
        ));
    }

    /// Empty left segment (`::local`) is rejected.
    #[test]
    fn federated_node_id_try_new_empty_space_segment_returns_err() {
        let result = FederatedNodeId::try_new("::local");
        assert!(matches!(
            result,
            Err(FederatedNodeIdError::EmptySpaceSegment(_))
        ));
    }

    /// Empty right segment (`space::`) is rejected.
    #[test]
    fn federated_node_id_try_new_empty_local_segment_returns_err() {
        let result = FederatedNodeId::try_new("space::");
        assert!(matches!(
            result,
            Err(FederatedNodeIdError::EmptyLocalSegment(_))
        ));
    }

    /// More than one separator is rejected.
    #[test]
    fn federated_node_id_try_new_multiple_separators_returns_err() {
        let result = FederatedNodeId::try_new("a::b::c");
        assert!(matches!(
            result,
            Err(FederatedNodeIdError::MultipleSeparators(_))
        ));
    }

    /// `space_id_str` returns the left half.
    #[test]
    fn federated_node_id_space_id_str_returns_left_of_separator() {
        let id = FederatedNodeId::try_new("auth-repo::file.rs:main:1").unwrap();
        assert_eq!(id.space_id_str(), "auth-repo");
    }

    /// `local_id_str` returns the right half.
    #[test]
    fn federated_node_id_local_id_str_returns_right_of_separator() {
        let id = FederatedNodeId::try_new("auth-repo::file.rs:main:1").unwrap();
        assert_eq!(id.local_id_str(), "file.rs:main:1");
    }

    /// `Display` writes the inner string verbatim.
    #[test]
    fn federated_node_id_display_prints_inner_string() {
        let id = FederatedNodeId::try_new("docs::doc:adr.md#intro").unwrap();
        assert_eq!(format!("{id}"), "docs::doc:adr.md#intro");
    }

    /// `from_parts` produces a `FederatedNodeId` whose segments
    /// match the inputs.
    #[test]
    fn federated_node_id_from_parts_constructs() {
        let id = FederatedNodeId::from_parts(&SpaceId::try_new("auth").unwrap(), "file.rs:main:1")
            .expect("from_parts ok");
        assert_eq!(id.as_str(), "auth::file.rs:main:1");
        assert_eq!(id.space_id_str(), "auth");
        assert_eq!(id.local_id_str(), "file.rs:main:1");
    }

    /// `from_parts` rejects a `local_id` that contains the
    /// separator.
    #[test]
    fn federated_node_id_from_parts_rejects_forbidden_substring() {
        let result =
            FederatedNodeId::from_parts(&SpaceId::try_new("auth").unwrap(), "bad::local");
        assert!(result.is_err());
    }

    /// `space_id()` roundtrips through `SpaceId`.
    #[test]
    fn federated_node_id_space_id_roundtrips() {
        let id = FederatedNodeId::try_new("default::symbol:src/a.rs:main:1").unwrap();
        let space = id.space_id();
        assert_eq!(space.as_str(), "default");
    }

    /// JSON roundtrip preserves the inner string.
    #[test]
    fn federated_node_id_json_roundtrip() {
        let id = FederatedNodeId::try_new("auth::file.rs:main:1").unwrap();
        let json = serde_json::to_string(&id).expect("serialize");
        let parsed: FederatedNodeId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, id);
    }
}
