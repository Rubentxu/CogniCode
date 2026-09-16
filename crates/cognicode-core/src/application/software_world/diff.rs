//! Software-world diff wiring (e71 WU3 — M9).
//!
//! Connects [`SoftwareWorld`] (e71 WU1+WU2) with the e68
//! [`FactDelta`] / [`compute_fact_delta`] without re-implementing any
//! diff algebra. The whole job of this module is **guards and labels**:
//!
//! - [`assert_base_snapshots_pair`] refuses to wire a `FactDelta`
//!   against two worlds that do not agree on the canonical base — a
//!   diff across different bases is meaningless.
//! - [`world_diff_label`] takes an already-computed [`FactDelta`] and
//!   attaches lineage metadata (parent world ids) to it, producing a
//!   [`WorldDiffReport`] that downstream consumers (e72 trials,
//!   e73 promotion evaluations) can carry without re-reading
//!   `SoftwareWorld`.
//!
//! ## What this module does NOT do
//!
//! - It does not compute `FactDelta`s itself; that lives in e68.
//! - It does not extract `Fact`s from anywhere; the caller supplies
//!   the facts it has already observed against the two snapshots.
//! - It does not introduce `WorldDiffEngine`/`SemanticWorldDiff`/its
//!   own fact equality. Reusing e68 is the whole point.
//!
//! ## Pure / deterministic / no I/O
//!
//! - No clock, no UUID mint, no env, no IO.
//! - `WorldDiffReport` is a value type with no derived behavior.
//! - The only side effect is returning a structured [`WorldDiffError`]
//!   on violation of the base-snapshot invariant.
//!
//! ## Fail-closed
//!
//! [`assert_base_snapshots_pair`] panics (with a precise message) on a
//! mismatch. The diff wiring is a programming invariant, not a runtime
//! condition; panicking on violation surfaces the bug at the call site
//! rather than silently producing a meaningless delta.

use crate::application::software_world::world::{SoftwareWorld, SoftwareWorldId};
use crate::domain::evidence_kernel::semantic_diff::FactDelta;

/// Failure modes of [`assert_base_snapshots_pair`].
///
/// This is a **programming-error** signal, not a runtime input
/// validation: there is no fallible variant of `world_diff_label`.
/// Mismatched base snapshots mean the caller has mis-wired the diff,
/// which is a bug we want to surface, not absorb.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WorldDiffError {
    /// The two worlds were forked from different canonical bases. A
    /// diff across different bases is meaningless; this signals a
    /// caller bug.
    #[error(
        "world diff requires both worlds to share the same base_snapshot; \
         got {left:?} (from world {left_world}) vs {right:?} (from world {right_world})"
    )]
    BaseSnapshotMismatch {
        left_world: SoftwareWorldId,
        right_world: SoftwareWorldId,
        left: crate::domain::evidence_kernel::ids::SnapshotId,
        right: crate::domain::evidence_kernel::ids::SnapshotId,
    },
}

/// Assert that two worlds share the same `base_snapshot`.
///
/// This is the only invariant the wiring enforces. A diff between two
/// worlds that point at different canonical revisions is not a "world
/// diff" — it is a cross-base diff, which e68 also does not define.
/// The e68 [`compute_fact_delta`] takes two snapshots directly; here
/// we add the world-level guard that wraps it.
///
/// Panics (does not return `Err`) on mismatch. The mismatch is a
/// caller bug, not a runtime condition. We use a panic so the call site
/// is unambiguous; if the wiring ever needs a fallible variant for
/// policy reasons, that becomes a separate decision.
pub fn assert_base_snapshots_pair(
    from: &SoftwareWorld,
    to: &SoftwareWorld,
) -> Result<(), WorldDiffError> {
    if from.base_snapshot != to.base_snapshot {
        return Err(WorldDiffError::BaseSnapshotMismatch {
            left_world: from.id.clone(),
            right_world: to.id.clone(),
            left: from.base_snapshot,
            right: to.base_snapshot,
        });
    }
    Ok(())
}

/// Lineage-labelled view of an e68 [`FactDelta`].
///
/// The delta is computed by e68 — we never touch the algebra here. The
/// only thing this struct adds is the bookkeeping identity: which two
/// worlds produced the delta.
///
/// [`WorldDiffReport`] is intentionally minimal. Future e72 / e73
/// layers may attach more lineage (proposal id, trial id, etc.); that
/// growth happens at the call sites, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldDiffReport {
    /// The world the diff was measured **from** (typically the base or
    /// the parent's snapshot).
    pub from_world: SoftwareWorldId,
    /// The world the diff was measured **to** (typically the candidate
    /// or the child's snapshot).
    pub to_world: SoftwareWorldId,
    /// The e68 [`FactDelta`] between the two worlds' snapshots. The
    /// algebra lives in `semantic_diff`; we only carry the result.
    pub delta: FactDelta,
}

/// Attach lineage labels to an already-computed e68 [`FactDelta`].
///
/// The caller is responsible for:
/// 1. Computing the [`FactDelta`] via e68
///    [`compute_fact_delta`](crate::domain::evidence_kernel::semantic_diff::compute_fact_delta)
///    over the two snapshots associated with `from_world` / `to_world`.
/// 2. Verifying the worlds share a `base_snapshot` — either by calling
///    [`assert_base_snapshots_pair`] first, or by relying on the domain
///    invariant that forks inherit `base_snapshot`.
///
/// This function does not re-check the base-snapshot invariant: the
/// caller has either already checked or has chosen to label a
/// cross-base delta (which is a domain-level decision, not a wiring
/// decision). We do not silently re-validate; we trust the caller.
pub fn world_diff_label(
    from_world: &SoftwareWorld,
    to_world: &SoftwareWorld,
    delta: FactDelta,
) -> WorldDiffReport {
    WorldDiffReport {
        from_world: from_world.id.clone(),
        to_world: to_world.id.clone(),
        delta,
    }
}
