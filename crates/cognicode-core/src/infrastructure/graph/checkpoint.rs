//! Checkpoint infrastructure for graph snapshot isolation (ADR-035).
//!
//! Provides a monotonic identifier ([`CheckpointId`]) and a versioned ring
//! buffer ([`VersionedGraphCache`]) that keeps the last N graphs so that
//! in-flight readers can pin to a specific version while writers replace
//! the head atomically.

use crate::domain::aggregates::call_graph::CallGraph;
use std::collections::VecDeque;
use std::sync::Arc;

/// Monotonic identifier for a graph checkpoint.
///
/// Assigned sequentially at creation. Never reused. IDs start at 1
/// (id 0 is reserved as "no checkpoint").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CheckpointId(pub u64);

impl CheckpointId {
    /// Reserved sentinel for "no checkpoint". Equivalent to `None` in the
    /// type system, but it lets callers carry an id in structs without an
    /// extra `Option`.
    pub const NONE: CheckpointId = CheckpointId(0);

    /// A checkpoint id is valid iff it was produced by
    /// [`VersionedGraphCache::insert`]. `CheckpointId::NONE` is invalid.
    pub fn is_valid(self) -> bool {
        self.0 > 0
    }

    /// Returns the next checkpoint id. Pure arithmetic — does NOT
    /// allocate or coordinate. Use [`VersionedGraphCache::insert`] to
    /// obtain an id that is guaranteed unique within a single cache.
    pub fn next(self) -> Self {
        CheckpointId(self.0 + 1)
    }
}

impl std::fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "checkpoint:{}", self.0)
    }
}

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
