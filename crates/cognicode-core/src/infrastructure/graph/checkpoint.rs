//! Checkpoint infrastructure for graph snapshot isolation (ADR-035).
//!
//! Provides a monotonic identifier ([`CheckpointId`]) and a versioned ring
//! buffer ([`VersionedGraphCache`]) that keeps the last N graphs so that
//! in-flight readers can pin to a specific version while writers replace
//! the head atomically.
//!
//! The [`CheckpointId`] type is canonical in
//! [`crate::domain::value_objects::CheckpointId`]; this module re-exports
//! it for ergonomics so callers can write
//! `use crate::infrastructure::graph::CheckpointId` instead of reaching
//! across the domain↔infrastructure boundary.

use crate::domain::aggregates::call_graph::CallGraph;
use std::collections::VecDeque;
use std::sync::Arc;

// Re-export the canonical domain type so existing callers using
// `crate::infrastructure::graph::checkpoint::CheckpointId` (and the
// `infrastructure::graph::CheckpointId` glob) keep working unchanged.
#[doc(no_inline)]
pub use crate::domain::value_objects::CheckpointId;

/// A versioned ring of `CallGraph` checkpoints.
///
/// Holds the most recent `retention` checkpoints (FIFO eviction).
/// `head` is always the latest insert. Readers can pin to a specific
/// [`CheckpointId`] if it is still in the ring.
pub struct VersionedGraphCache {
    checkpoints: VecDeque<(CheckpointId, Arc<CallGraph>)>,
    head: CheckpointId,
    next_id: u64,
    retention: usize,
}

impl VersionedGraphCache {
    /// Build a new empty ring with the given retention. Panics if
    /// `retention < 1` because a zero-capacity ring can never serve the
    /// current head.
    pub fn new(retention: usize) -> Self {
        assert!(retention >= 1, "retention must be at least 1");
        Self {
            checkpoints: VecDeque::with_capacity(retention),
            head: CheckpointId::NONE,
            next_id: 1,
            retention,
        }
    }

    /// Insert a new graph, return its [`CheckpointId`].
    /// Evicts the oldest checkpoint if at capacity.
    pub fn insert(&mut self, graph: Arc<CallGraph>) -> CheckpointId {
        let id = CheckpointId(self.next_id);
        self.next_id += 1;
        if self.checkpoints.len() >= self.retention {
            self.checkpoints.pop_front();
        }
        self.checkpoints.push_back((id, graph));
        self.head = id;
        id
    }

    /// Get the current head graph. `None` if no insert has happened yet.
    pub fn head(&self) -> Option<Arc<CallGraph>> {
        self.checkpoints.back().map(|(_, g)| g.clone())
    }

    /// Get the current head [`CheckpointId`], or [`CheckpointId::NONE`]
    /// if the ring is empty.
    pub fn head_id(&self) -> CheckpointId {
        self.head
    }

    /// Get a specific checkpoint by id, if still in the ring.
    pub fn get_at(&self, id: CheckpointId) -> Option<Arc<CallGraph>> {
        self.checkpoints
            .iter()
            .find(|(cid, _)| *cid == id)
            .map(|(_, g)| g.clone())
    }

    /// Check if a checkpoint id is in the ring.
    pub fn contains(&self, id: CheckpointId) -> bool {
        self.checkpoints.iter().any(|(cid, _)| *cid == id)
    }
}
