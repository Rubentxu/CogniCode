//! Software world foundation (e71 WU1 — M9).
//!
//! A [`SoftwareWorld`] describes **isolation and lineage** of a candidate
//! analysis target. It is *not* a second source of truth: it carries no
//! `Fact`s, no `ReadSet`, no `EvidenceBundle`, no graph store handle.
//! The only canonical reference is [`SoftwareWorld::base_snapshot`], which
//! points at a [`SnapshotId`] (bijective with a `RevisionId`).
//!
//! ## Why a separate concept from `Snapshot`?
//!
//! A [`SnapshotDescriptor`](crate::domain::evidence_kernel::snapshot::SnapshotDescriptor)
//! is the canonical analysed truth of *one* workspace revision. A
//! `SoftwareWorld` describes an **isolated derivation context** for a
//! candidate change: where the source is, what snapshot the candidate was
//! measured against, and the chain of forks that produced it.
//!
//! The three concepts each have a single responsibility:
//!
//! - `World` describes isolation/lineage.
//! - `Snapshot` describes canonical analysed truth.
//! - `FactDelta` compares truth (e68 WU1).
//!
//! `FactDelta` does the diffing; `SoftwareWorld` does the bookkeeping.
//! Nothing here replaces or shadows the kernel.
//!
//! ## Why `source_state` is descriptive, not materialized
//!
//! A forked world carries a [`WorldSourceState::Forked`] record: the
//! `path` is a logical identifier (e.g. a workspace-relative source
//! reference), the `content_hash` is a stable 256-bit digest. We do NOT
//! copy bytes, files or facts into the world. The world is a receipt, not
//! a cache. If e72 / e73 ever need a materialized source root for an
//! isolated trial, that is a separate adapter (filesystem layer), not a
//! change to these types.
//!
//! ## Pure / deterministic / no I/O
//!
//! This module is pure domain logic. It does not read from disk, does not
//! generate UUIDs at construction time, and does not touch any graph
//! store. Construction is total (no fallible variants). [`ContentHash`]
//! is constructed by the caller; we never compute hashes here. Fork
//! construction (e71 WU2) is pure: no clock, no env, no IO.
//!
//! ## Fail-closed by construction
//!
//! - A [`SoftwareWorld::Base`] world has `parent_world == None`.
//! - A [`SoftwareWorld::Forked`] world has `parent_world == Some(_)`.
//! - The constructor enforces this: callers cannot fabricate a base
//!   world with a parent, nor a forked world without one.
//!
//! ## Why this is an application-layer type, not domain
//!
//! `SoftwareWorld` is an *operational* concept. The kernel knows about
//! revisions, snapshots and facts; it does not know about candidate
//! changes, forks, or trials. We keep this module under
//! `crate::application::software_world` so the domain layer remains
//! untouched and the architectural rule "domain has no IO/types of
//! operational lineage" is preserved (see `AGENTS.md`).
//!
//! The feature gate `evidence-kernel` is required because [`SnapshotId`]
//! lives behind that gate.

#[cfg(feature = "evidence-kernel")]
pub mod world;

#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "world_tests.rs"]
mod world_tests;
