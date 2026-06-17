//! Monotonic identifier for a graph checkpoint (ADR-035).
//!
//! Assigned sequentially at creation. Never reused. IDs start at 1
//! (id 0 is reserved as "no checkpoint").
//!
//! Lives in the domain layer because the [`GraphStore`](crate::domain::traits::GraphStore)
//! trait references it. The canonical implementation is in
//! [`infrastructure::graph::checkpoint`](crate::infrastructure::graph::checkpoint).

use std::fmt;

/// Monotonic identifier for a graph checkpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CheckpointId(pub u64);

impl CheckpointId {
    /// Reserved sentinel for "no checkpoint". Equivalent to `None` in the
    /// type system, but it lets callers carry an id in structs without an
    /// extra `Option`.
    pub const NONE: CheckpointId = CheckpointId(0);

    /// A checkpoint id is valid iff it was produced by
    /// [`crate::infrastructure::graph::checkpoint::VersionedGraphCache::insert`].
    /// `CheckpointId::NONE` is invalid.
    pub const fn is_valid(self) -> bool {
        self.0 > 0
    }

    /// Returns the next checkpoint id. Pure arithmetic — does NOT
    /// allocate or coordinate. Use
    /// [`crate::infrastructure::graph::checkpoint::VersionedGraphCache::insert`]
    /// to obtain an id that is guaranteed unique within a single cache.
    pub const fn next(self) -> Self {
        CheckpointId(self.0 + 1)
    }
}

impl fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checkpoint:{}", self.0)
    }
}
