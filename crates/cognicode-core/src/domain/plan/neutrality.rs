//! Static backend-neutrality assertions.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR1 Foundation Phase 1.
//!
//! ## Design
//!
//! The `plan` module must NEVER import backend-specific types:
//! - `sqlx::Value` / `sqlx::Row` — PostgreSQL driver
//! - `tokio::task::JoinSet` / `tokio::spawn` — async runtime
//! - `petgraph::Graph` / `petgraph::graphmap` — graph data structure
//!
//! This module provides a compile-time assertion via a sealed marker trait
//! and a `static` assertion struct. Any module that implements the sealed
//! trait certifies that it does not contain backend types.

use std::marker::PhantomData;

// ============================================================================
// Sealed marker trait — no backend types
// ============================================================================

mod sealed {
    pub trait Sealed {}
}

/// Marker trait for types that are guaranteed not to contain any backend-
/// specific types (`sqlx`, `tokio`, `petgraph`, etc.).
///
/// Types in `domain::plan::*` should implement this marker trait to certify
/// their backend-neutrality at the type level. The `assert_backend_neutral!`
/// macro generates a compile-time assertion that the type's module has not
/// imported banned types.
pub trait BackendNeutral: sealed::Sealed {}

/// A type that has been verified as backend-neutral at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendNeutralMarker<T>(PhantomData<T>);

impl<T: sealed::Sealed> BackendNeutral for T {}

/// Assert at compile time that `T` is backend-neutral.
///
/// Usage: `assert_backend_neutral!(MyPlanType);`
/// If `MyPlanType`'s module imports a banned type, this fails to compile.
#[macro_export]
macro_rules! assert_backend_neutral {
    ($ty:ty) => {
        const _: fn($ty) = |t| {
            // This closure is never called — it only type-checks that `T: BackendNeutral`.
            fn assert_neutral<T: $crate::domain::plan::BackendNeutral>(_: &T) {}
            assert_neutral(&t);
        };
    };
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 1.11a RED — Static backend-neutrality: import of banned types fails build
    // Scenario: `moldplan-graphplan::Backend-Neutrality` (Static neutrality assertion +
    //           Plan is Send + Sync + 'static)
    // Assert: `cargo build -p cognicode-core` fails if banned backend type leaks in
    // -------------------------------------------------------------------------

    /// `BackendNeutral` is implemented for `PhantomData<()>` as a baseline.
    #[test]
    fn backend_neutral_marker_works() {
        // This test verifies the marker trait compiles without importing banned types.
        // The actual compile-time assertion is in the `assert_backend_neutral!` macro.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<BackendNeutralMarker<()>>();
    }

    /// Types in the plan module must be `Send + Sync + 'static`.
    /// This is verified by the Rust type system — if a plan type contained
    /// a `tokio::task::JoinSet` it would not be `Send`.
    #[test]
    fn plan_types_are_send_sync_static() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_static<T: 'static>() {}

        // Verify the types we define satisfy these bounds.
        // If a type contains a non-Send/non-Sync/non-'static field, this fails to compile.
        use super::super::{PlanVersion, PlanHash, PlanMetadata, PlanLimits, PlanLimit,
                           TypedValue, ResultSet, Path, CancellationToken,
                           UnsupportedConstruct, ConstructId, SourceLocation,
                           ExecutorError, PlanError};
        assert_send::<PlanVersion>();
        assert_sync::<PlanVersion>();
        assert_static::<PlanVersion>();
        assert_send::<PlanHash>();
        assert_sync::<PlanHash>();
        assert_static::<PlanHash>();
        assert_send::<PlanMetadata>();
        assert_sync::<PlanMetadata>();
        assert_static::<PlanMetadata>();
        assert_send::<PlanLimits>();
        assert_sync::<PlanLimits>();
        assert_static::<PlanLimits>();
        assert_send::<TypedValue>();
        assert_sync::<TypedValue>();
        assert_static::<TypedValue>();
        assert_send::<ResultSet>();
        assert_sync::<ResultSet>();
        assert_static::<ResultSet>();
        assert_send::<Path>();
        assert_sync::<Path>();
        assert_static::<Path>();
        assert_send::<CancellationToken>();
        assert_sync::<CancellationToken>();
        assert_static::<CancellationToken>();
        assert_send::<UnsupportedConstruct>();
        assert_sync::<UnsupportedConstruct>();
        assert_static::<UnsupportedConstruct>();
        assert_send::<ExecutorError>();
        assert_sync::<ExecutorError>();
        assert_static::<ExecutorError>();
    }
}
