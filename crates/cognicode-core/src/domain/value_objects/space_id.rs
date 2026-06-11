//! `SpaceId` — opaque, non-empty newtype for federation spaces.
//!
//! The wire-level `id` of a space (e.g. `"default"`, `"auth-repo"`,
//! `"docs-2024"`) wrapped in a strongly-typed newtype. Rejects empty
//! strings and whitespace-only strings at construction time so the
//! rest of the federation layer can rely on the invariant.
//!
//! Gated behind the `multimodal` Cargo feature. Default builds are
//! unaffected: this module compiles to nothing without the feature
//! and `SpaceId` is not exported from `cognicode_core`.
//!
//! ```text
//! SpaceId(String)  — non-empty after trim, opaque, Display
//! ```

use std::fmt;

use serde::{Deserialize, Serialize};

/// Error type for [`SpaceId::try_new`] failures.
///
/// Three variants cover every validation failure path:
///
/// - [`SpaceError::EmptyId`](super::space::SpaceError::EmptyId) is the
///   canonical "empty / whitespace-only" error. The
///   [`SpaceId`] thin newtype re-uses the upstream `SpaceError` enum
///   so consumers can match on a single type.
pub use super::space::SpaceError;

/// Opaque, non-empty space id.
///
/// Wrap any `String`-like input with [`SpaceId::try_new`]; the call
/// returns `Err` for empty or whitespace-only input. The reserved
/// id `"default"` is constructed via [`SpaceId::default`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpaceId(pub String);

impl SpaceId {
    /// The reserved id used for backward-compatibility with the
    /// pre-federation single-graph model. Every row in the
    /// `graph_nodes` table is backfilled to this space by the
    /// `m00xx_graph_nodes_space_id` migration.
    pub const DEFAULT: &'static str = "default";

    /// Construct a `SpaceId` from any string-like input. Rejects
    /// empty strings and whitespace-only strings.
    ///
    /// ```text
    /// SpaceId::try_new("default")  -> Ok(SpaceId("default".into()))
    /// SpaceId::try_new("")         -> Err(SpaceError::EmptyId)
    /// SpaceId::try_new("   ")      -> Err(SpaceError::EmptyId)
    /// ```
    pub fn try_new(s: impl Into<String>) -> Result<Self, SpaceError> {
        let inner = s.into();
        if inner.trim().is_empty() {
            Err(SpaceError::EmptyId)
        } else {
            Ok(Self(inner))
        }
    }

    /// Borrow the underlying id string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the wrapper and return the inner `String`.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Default for SpaceId {
    /// The reserved id `"default"`. Used for the implicit space
    /// every pre-federation node belongs to.
    fn default() -> Self {
        Self(Self::DEFAULT.to_string())
    }
}

impl fmt::Display for SpaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for SpaceId {
    /// Unchecked conversion. Callers that want validation must use
    /// [`SpaceId::try_new`]. The unchecked `From` impl exists so
    /// `serde::Deserialize` round-trips work without a custom
    /// visitor.
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for SpaceId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for SpaceId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty string is rejected.
    #[test]
    fn space_id_try_new_empty_returns_err() {
        let result = SpaceId::try_new("");
        assert!(matches!(result, Err(SpaceError::EmptyId)));
    }

    /// Whitespace-only string is rejected.
    #[test]
    fn space_id_try_new_whitespace_only_returns_err() {
        let result = SpaceId::try_new("   ");
        assert!(matches!(result, Err(SpaceError::EmptyId)));
    }

    /// Non-empty string succeeds.
    #[test]
    fn space_id_try_new_non_empty_succeeds() {
        let id = SpaceId::try_new("auth-repo").expect("non-empty must succeed");
        assert_eq!(id.as_str(), "auth-repo");
    }

    /// `SpaceId::default()` returns the reserved id `"default"`.
    #[test]
    fn space_id_default_constant() {
        let id = SpaceId::default();
        assert_eq!(id.as_str(), "default");
        // Direct comparison with the constant
        assert_eq!(id, SpaceId::from(SpaceId::DEFAULT.to_string()));
    }

    /// The `Display` impl writes the inner string verbatim.
    #[test]
    fn space_id_display_prints_inner_string() {
        let id = SpaceId::try_new("docs-2024").unwrap();
        assert_eq!(format!("{id}"), "docs-2024");
    }

    /// JSON roundtrip preserves the inner string.
    #[test]
    fn space_id_json_roundtrip() {
        let id = SpaceId::try_new("auth-repo").unwrap();
        let json = serde_json::to_string(&id).expect("serialize");
        let parsed: SpaceId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, id);
    }
}
