//! `EdgeMetadata` — value object representing a call-graph edge with
//! provenance and confidence metadata.
//!
//! This is the **domain projection** of a row in the PostgreSQL
//! `call_edges` table (defined in
//! `crates/cognicode-core/src/infrastructure/persistence/schema_postgres.sql`).
//! The schema models seven data columns; downstream code that needs
//! to reshape edges can use the `From` impls in
//! `cognicode-core::interface::mcp::handlers`.
//!
//! The struct is intentionally **ungated** — it is reachable from a
//! default build (no `postgres` feature required). The only place
//! `sqlx` is involved is the private `EdgeRow` mapping struct inside
//! the feature-gated `postgres_repository` module.
//!
//! The `id` surrogate key is **NOT** a field here: it is a persistence
//! detail that downstream code should not need to reason about.

use crate::domain::value_objects::{DependencyType, Provenance};

/// Call-graph edge with provenance and confidence metadata.
///
/// Mirrors the seven data columns of the SQLite/PostgreSQL
/// `call_edges` table:
/// `(caller_id, caller_name, callee_id, callee_name, dependency_type,
/// provenance, confidence)`. The `id` surrogate primary key is
/// intentionally absent — it is a persistence detail.
///
/// Derives `Debug, Clone, PartialEq` (the spec contract). Does NOT
/// derive `Serialize` — persistence is the database's job, not the
/// domain struct's.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeMetadata {
    /// Canonical symbol id of the caller (`file:name:line`).
    pub caller_id: String,
    /// Human-readable name of the caller symbol.
    pub caller_name: String,
    /// Canonical symbol id of the callee.
    pub callee_id: String,
    /// Human-readable name of the callee symbol.
    pub callee_name: String,
    /// What kind of dependency relationship this edge represents
    /// (call, import, inherit, etc.).
    pub dependency_type: DependencyType,
    /// How this edge was obtained (AST extraction, heuristic
    /// inference, or ambiguous resolution).
    pub provenance: Provenance,
    /// Confidence score in the closed interval `[0.0, 1.0]`. Defaults
    /// to `1.0` for directly extracted edges.
    pub confidence: f64,
}

impl EdgeMetadata {
    /// Construct a new [`EdgeMetadata`] with the default confidence
    /// of `1.0` (the safe, common case for AST-extracted edges).
    pub fn new(
        caller_id: impl Into<String>,
        caller_name: impl Into<String>,
        callee_id: impl Into<String>,
        callee_name: impl Into<String>,
        dependency_type: DependencyType,
        provenance: Provenance,
    ) -> Self {
        Self {
            caller_id: caller_id.into(),
            caller_name: caller_name.into(),
            callee_id: callee_id.into(),
            callee_name: callee_name.into(),
            dependency_type,
            provenance,
            confidence: 1.0,
        }
    }

    /// Construct a new [`EdgeMetadata`] with an explicit confidence
    /// value. The caller is responsible for clamping the value to
    /// `[0.0, 1.0]` if that matters for the use case.
    pub fn with_confidence(
        caller_id: impl Into<String>,
        caller_name: impl Into<String>,
        callee_id: impl Into<String>,
        callee_name: impl Into<String>,
        dependency_type: DependencyType,
        provenance: Provenance,
        confidence: f64,
    ) -> Self {
        Self {
            caller_id: caller_id.into(),
            caller_name: caller_name.into(),
            callee_id: callee_id.into(),
            callee_name: callee_name.into(),
            dependency_type,
            provenance,
            confidence,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_uses_default_confidence_one() {
        let edge = EdgeMetadata::new(
            "src/a.rs:caller:1",
            "caller",
            "src/b.rs:callee:2",
            "callee",
            DependencyType::Calls,
            Provenance::Extracted,
        );
        assert_eq!(edge.caller_id, "src/a.rs:caller:1");
        assert_eq!(edge.caller_name, "caller");
        assert_eq!(edge.callee_id, "src/b.rs:callee:2");
        assert_eq!(edge.callee_name, "callee");
        assert_eq!(edge.dependency_type, DependencyType::Calls);
        assert_eq!(edge.provenance, Provenance::Extracted);
        assert_eq!(edge.confidence, 1.0);
    }

    #[test]
    fn with_confidence_preserves_explicit_score() {
        let edge = EdgeMetadata::with_confidence(
            "a",
            "caller",
            "b",
            "callee",
            DependencyType::Imports,
            Provenance::Inferred,
            0.7,
        );
        assert_eq!(edge.dependency_type, DependencyType::Imports);
        assert_eq!(edge.provenance, Provenance::Inferred);
        assert_eq!(edge.confidence, 0.7);
    }

    #[test]
    fn equality_is_field_wise() {
        let a = EdgeMetadata::new(
            "a",
            "a",
            "b",
            "b",
            DependencyType::Calls,
            Provenance::Extracted,
        );
        let b = EdgeMetadata::new(
            "a",
            "a",
            "b",
            "b",
            DependencyType::Calls,
            Provenance::Extracted,
        );
        assert_eq!(a, b);

        let c = EdgeMetadata::with_confidence(
            "a",
            "a",
            "b",
            "b",
            DependencyType::Calls,
            Provenance::Extracted,
            0.5,
        );
        assert_ne!(a, c, "confidence must participate in equality");
    }
}
