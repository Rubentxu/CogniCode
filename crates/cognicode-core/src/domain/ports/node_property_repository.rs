//! Async port for reading node properties (ownership attribution).
//!
//! Separated from [`crate::domain::traits::graph_query_port::GraphQueryPort`]
//! to avoid forcing async propagation through the sync graph query surface.
//!
//! # Why a separate port?
//!
//! [`GraphQueryPort::node_properties`] is a sync trait method. Implementing it
//! asynchronously from inside a Tokio runtime requires `Handle::current().block_on()`,
//! which has deadlock risk and is not idiomatic. Instead, we expose node properties
//! via this dedicated async port and let executors that have it available use it
//! directly; executors that only have the sync port see no ownership data.
//!
//! # Implementations
//!
//! - `CallGraphRepository` implements this trait when built with `--features ownership`
//!   by delegating to the canonical adapter's async `node_properties`.
//! - Mocks can provide a trivial async stub.

use crate::domain::aggregates::SymbolId;
use async_trait::async_trait;
use std::collections::HashMap;

/// Async port for reading node properties (ownership attribution).
///
/// Methods return `Option<HashMap>` so that callers can degrade gracefully when
/// the underlying repository has no data for the given id (or when no
/// repository is wired).
#[async_trait]
pub trait NodePropertyRepository: Send + Sync {
    /// Return the JSONB properties map for `id`, or `None` if not present.
    async fn node_properties(&self, id: &SymbolId) -> Option<HashMap<String, String>>;
}
