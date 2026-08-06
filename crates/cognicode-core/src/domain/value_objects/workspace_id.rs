//! WorkspaceId — identifier for a workspace in the canonical graph.
//!
//! Part of e28-0-canonical-graph-revisions: PR1 Foundation.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::should_implement_trait, unused_imports)]

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

// ============================================================================
// WorkspaceId
// ============================================================================

/// Error type for [`WorkspaceId::try_new`] failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorkspaceIdError {
    /// The workspace id string was empty.
    #[error("workspace id cannot be empty")]
    Empty,
}

/// A workspace identifier. Workspaces provide hard isolation between
/// independent analysis contexts (e.g. different repos, different branches,
/// different tenants).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkspaceId(String);

impl WorkspaceId {
    /// The default workspace id (`"default"`).
    pub fn default() -> Self {
        Self("default".to_string())
    }

    /// Constructs a `WorkspaceId` from a non-empty string.
    ///
    /// Returns `Err(WorkspaceIdError::Empty)` if the string is empty
    /// after trimming whitespace.
    pub fn try_new(s: impl Into<String>) -> Result<Self, WorkspaceIdError> {
        let s = s.into();
        if s.trim().is_empty() {
            Err(WorkspaceIdError::Empty)
        } else {
            Ok(Self(s))
        }
    }

    /// Returns the workspace id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for WorkspaceId {
    fn default() -> Self {
        Self::default()
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 1.2a RED — Default and empty rejection
    // Scenario: `generic-graph-model::Default and empty rejection`
    // Assert: `WorkspaceId::default().as_str()=="default"`;
    //         `try_new("")`→`Err(WorkspaceIdError::Empty)`
    // -------------------------------------------------------------------------

    /// `WorkspaceId::default().as_str()` must be `"default"`.
    #[test]
    fn workspace_id_default_is_default() {
        assert_eq!(WorkspaceId::default().as_str(), "default");
    }

    /// `WorkspaceId::default()` must be valid (not the empty string).
    #[test]
    fn workspace_id_default_is_valid() {
        assert_ne!(WorkspaceId::default().as_str(), "");
    }

    /// `WorkspaceId::try_new("")` must return `Err(WorkspaceIdError::Empty)`.
    #[test]
    fn workspace_id_empty_rejected() {
        let err = WorkspaceId::try_new("").unwrap_err();
        assert_eq!(err, WorkspaceIdError::Empty);
    }

    /// `WorkspaceId::try_new("   ")` (whitespace only) must return `Err(WorkspaceIdError::Empty)`.
    #[test]
    fn workspace_id_whitespace_only_rejected() {
        let err = WorkspaceId::try_new("   ").unwrap_err();
        assert_eq!(err, WorkspaceIdError::Empty);
    }

    /// `WorkspaceId::try_new("my-workspace")` must succeed.
    #[test]
    fn workspace_id_valid() {
        let ws = WorkspaceId::try_new("my-workspace").expect("valid workspace id");
        assert_eq!(ws.as_str(), "my-workspace");
    }

    /// `WorkspaceId` must implement `Default` to `WorkspaceId::default()`.
    #[test]
    fn workspace_id_default_trait() {
        let default: WorkspaceId = Default::default();
        assert_eq!(default.as_str(), "default");
    }

    /// `WorkspaceId` must serialize and deserialize correctly via serde.
    #[test]
    fn workspace_id_serde_roundtrip() {
        let ws = WorkspaceId::try_new("test-ws").unwrap();
        let json = serde_json::to_string(&ws).expect("serialize");
        let parsed: WorkspaceId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, ws);
    }

    /// Two `WorkspaceId`s with the same string must be equal.
    #[test]
    fn workspace_id_equality() {
        let a = WorkspaceId::try_new("ws1").unwrap();
        let b = WorkspaceId::try_new("ws1").unwrap();
        assert_eq!(a, b);
    }
}
