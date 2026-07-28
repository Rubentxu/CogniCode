//! AST → Plan lowering — compile MoldQL AST to GraphPlan.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR2 Plan Algebra.
//!
//! ## Architecture
//!
//! This module defines the **port** (trait) for AST→Plan lowering.
//! The **adapter** implementation lives in `cognicode-explorer` (Phase 3 Bridge),
//! where the full `MoldQLQuery` AST is available.
//!
//! This separation enforces the hexagonal architecture invariant:
//! `cognicode-core` (domain) must NOT depend on `cognicode-explorer` (infrastructure).

use super::{GraphPlan, PlanError};

/// Handles lowering of a MoldQL AST to a [`GraphPlan`].
///
/// Implementors must be provided by the infrastructure layer (e.g., `cognicode-explorer`).
pub trait AstLowerer: Send + Sync {
    /// Lower a query AST node to a [`GraphPlan`].
    ///
    /// Returns `Err(PlanError)` if the AST node cannot be lowered
    /// (e.g., unbounded quantifier, unsupported construct).
    fn lower(&self, ast: &dyn std::any::Any) -> Result<GraphPlan, PlanError>;
}

/// Default no-op lowerer used when no adapter is wired.
pub struct NoOpLowerer;

impl AstLowerer for NoOpLowerer {
    fn lower(&self, _ast: &dyn std::any::Any) -> Result<GraphPlan, PlanError> {
        Err(PlanError::UnsupportedConstruct(
            super::UnsupportedConstruct::new(
                super::ConstructId::Other("no lowerer wired".into()),
                "no AstLowerer adapter is wired in this build",
            )
            .with_alternative("wire an AstLowerer implementation from the infrastructure layer"),
        ))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// `NoOpLowerer` always returns an error indicating no lowerer is wired.
    #[test]
    fn noop_lowerer_returns_error() {
        let lowerer = NoOpLowerer;
        // Use an empty Any reference — the NoOpLowerer ignores it anyway
        struct DummyQuery;
        let dummy = &DummyQuery as &dyn std::any::Any;
        let result = lowerer.lower(dummy);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PlanError::UnsupportedConstruct { .. }));
    }

    /// `NoOpLowerer` is `Send + Sync + 'static`.
    #[test]
    fn noop_lowerer_send_sync_static() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_static<T: 'static>() {}
        assert_send::<NoOpLowerer>();
        assert_sync::<NoOpLowerer>();
        assert_static::<NoOpLowerer>();
    }
}
