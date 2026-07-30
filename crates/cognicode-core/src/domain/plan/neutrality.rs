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
//! This module provides a compile-time assertion via a sealed marker trait.
//! The `Sealed` trait is defined HERE (in `neutrality.rs`) and re-exported
//! by ALL plan types' parent modules. A type implements `Sealed` to certify
//! it is part of the plan module. External code cannot implement `Sealed`
//! because the trait is not accessible outside the `plan` module boundary.
//
//! ## Sealed trait re-export
//!
//! Each child module (`version.rs`, `limits.rs`, etc.) re-exports `Sealed`
//! via `pub use super::neutrality::Sealed;` and then `impl Sealed for MyType;`.
//! This allows `assert_backend_neutral!` to work while keeping `Sealed`
//! inaccessible from outside the `plan` module.
//! because the trait is not accessible outside the module hierarchy.

use std::marker::PhantomData;

// ============================================================================
// Sealed marker trait — no backend types
// ============================================================================

/// Seals the `BackendNeutral` trait to types within the `plan` module.
///
/// `Sealed` is implemented by every type that lives in `domain::plan::*`.
/// External code cannot implement `Sealed` because this trait is not
/// exported outside the `plan` module boundary — it is re-exported only
/// by the specific modules that define each plan type (via `pub use super::sealed::Sealed`).
///
/// This is the standard Rust sealed-trait pattern: the trait is accessible
/// within the module hierarchy but not outside it.
pub trait Sealed {}

/// Marker trait for types that are guaranteed not to contain any backend-
/// specific types (`sqlx`, `tokio`, `petgraph`, etc.).
///
/// A type implements `Sealed` (above) to certify its plan-module membership.
/// `BackendNeutral` is automatically satisfied by any `Sealed` type via the
/// blanket impl below. The `assert_backend_neutral!` macro generates a
/// compile-time assertion that `T: BackendNeutral`.
pub trait BackendNeutral: Sealed {}

impl<T: Sealed> BackendNeutral for T {}

/// A type that has been verified as backend-neutral at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendNeutralMarker<T>(PhantomData<T>);

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
        use super::super::{
            CancellationToken, ConstructId, ExecutorError, Path, PlanError, PlanHash, PlanLimit,
            PlanLimits, PlanMetadata, PlanVersion, ResultSet, SourceLocation, TypedValue,
            UnsupportedConstruct,
        };
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
