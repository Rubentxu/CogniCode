//! RevisionId — canonical graph revision identifier.
//!
//! A monotonic u64 assigned per workspace on every ingest commit.
//! RevisionId = 0 is the `NONE` sentinel and is never valid.
//!
//! Part of e28-0-canonical-graph-revisions: PR1 Foundation.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// ============================================================================
// RevisionId
// ============================================================================

/// Monotonic identifier for a graph revision.
///
/// Each ingest commit opens a new revision. Revisions are workspace-scoped —
/// the same numeric id can exist in different workspaces without collision.
/// `RevisionId(0)` is the `NONE` sentinel and is never valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RevisionId(pub u64);

impl RevisionId {
    /// Reserved sentinel for "no revision". Equivalent to `None` in the
    /// type system, but it lets callers carry an id without an extra `Option`.
    pub const NONE: RevisionId = RevisionId(0);

    /// Constructs a `RevisionId` from a raw u64.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Returns the raw u64 value.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// A revision id is valid iff it is not `NONE`.
    pub const fn is_valid(self) -> bool {
        self.0 > 0
    }
}

impl fmt::Display for RevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "rev:{}", self.0)
    }
}

/// Error type for [`RevisionId::from_str`] failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseRevisionIdError {
    #[error("invalid revision id format: expected 'rev:N' where N is a non-negative integer")]
    MalformedFormat,
    #[error("revision id must not be zero (use RevisionId::NONE instead)")]
    ZeroSentinel,
}

impl FromStr for RevisionId {
    type Err = ParseRevisionIdError;

    /// Parse a `RevisionId` from its `Display` form: `"rev:N"`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let prefix = "rev:";
        if !s.starts_with(prefix) {
            return Err(ParseRevisionIdError::MalformedFormat);
        }
        let num_str = &s[prefix.len()..];
        if num_str.is_empty() {
            return Err(ParseRevisionIdError::MalformedFormat);
        }
        let n: u64 = num_str
            .parse()
            .map_err(|_| ParseRevisionIdError::MalformedFormat)?;
        if n == 0 {
            return Err(ParseRevisionIdError::ZeroSentinel);
        }
        Ok(RevisionId(n))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // -------------------------------------------------------------------------
    // Task 1.1a RED — Round-trip and reserved sentinel
    // Scenario: `graph-revisions::Round-trip and reserved sentinel`
    // Assert: `RevisionId(7).to_string()=="rev:7"` parses back;
    //         `RevisionId::NONE.is_valid()==false`
    // -------------------------------------------------------------------------

    /// `RevisionId::new(7).to_string()` must produce `"rev:7"` which parses back.
    #[test]
    fn revision_id_round_trip() {
        let id = RevisionId::new(7);
        let display = id.to_string();
        assert_eq!(display, "rev:7", "Display must produce 'rev:N' format");

        let parsed = RevisionId::from_str(&display).expect("'rev:7' must parse");
        assert_eq!(parsed, id, "Parsed value must equal original");
    }

    /// `RevisionId::NONE.is_valid()` must be `false` — the sentinel is never valid.
    #[test]
    fn revision_id_none_is_invalid() {
        assert!(
            !RevisionId::NONE.is_valid(),
            "RevisionId::NONE must be invalid"
        );
    }

    /// A normal revision id (non-zero) must be valid.
    #[test]
    fn revision_id_normal_is_valid() {
        assert!(RevisionId::new(1).is_valid());
        assert!(RevisionId::new(u64::MAX).is_valid());
    }

    /// `RevisionId` must derive `Copy` so it can be passed by value.
    #[test]
    fn revision_id_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<RevisionId>();
    }

    /// `RevisionId` must implement `PartialEq` and `Eq`.
    #[test]
    fn revision_id_equality() {
        assert_eq!(RevisionId::new(1), RevisionId::new(1));
        assert_ne!(RevisionId::new(1), RevisionId::new(2));
        assert_eq!(RevisionId::NONE, RevisionId::NONE);
    }

    /// `RevisionId` must implement `Ord` for sorting.
    #[test]
    fn revision_id_ordered() {
        use std::cmp::Ordering;
        assert_eq!(RevisionId::new(1).cmp(&RevisionId::new(2)), Ordering::Less);
        assert_eq!(RevisionId::NONE.cmp(&RevisionId::new(1)), Ordering::Less);
    }

    /// `FromStr` must reject malformed strings.
    #[test]
    fn revision_id_from_str_rejects_malformed() {
        assert!(RevisionId::from_str("rev:").is_err());
        assert!(RevisionId::from_str("rev:abc").is_err());
        assert!(RevisionId::from_str("rev:-1").is_err());
        assert!(RevisionId::from_str("not-a-revision").is_err());
        assert!(RevisionId::from_str("").is_err());
        assert!(RevisionId::from_str("rev:0").is_err());
    }

    /// `RevisionId` must serialize and deserialize correctly via serde.
    #[test]
    fn revision_id_serde_roundtrip() {
        let id = RevisionId::new(42);
        let json = serde_json::to_string(&id).expect("serialize");
        let parsed: RevisionId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, id);
    }
}
