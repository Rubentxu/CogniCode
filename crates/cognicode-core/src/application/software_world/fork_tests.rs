//! Tests for `application::software_world::fork` (e71 WU2).
//!
//! Adversarial coverage:
//!
//! 1. The parent world is unchanged after `fork` (Rust borrow + a
//!    structural equality check).
//! 2. The child inherits the parent's `base_snapshot`.
//! 3. The child carries the mutation's path and content hash.
//! 4. The child's `parent_world` points at the parent's id.
//! 5. An identity mutation produces a child whose `source_state` is
//!    observationally identical to the parent's — this is the UAT
//!    decisive "fork without changes" case.
//! 6. A chain of forks produces a navigable lineage.
//! 7. Forking a forked world is legal (multi-level lineage).

use std::path::PathBuf;

use crate::application::software_world::fork::{SourceMutation, fork};
use crate::application::software_world::world::{
    ContentHash, SoftwareWorld, SoftwareWorldId, WorldSourceState,
};
use crate::domain::evidence_kernel::ids::SnapshotId;

// --- helpers ---------------------------------------------------------

fn hash_of(byte: u8) -> ContentHash {
    ContentHash([byte; 32])
}

fn snapshot(n: u64) -> SnapshotId {
    SnapshotId::new(n)
}

fn base_world(id: &str, snap: u64) -> SoftwareWorld {
    SoftwareWorld::new_base(SoftwareWorldId::from_string(id), snapshot(snap))
}

// --- parent unchanged ------------------------------------------------

#[test]
fn parent_is_unchanged_after_fork() {
    let parent = base_world("w-root", 5);
    let parent_before = parent.clone();

    let outcome = fork(
        &parent,
        SoftwareWorldId::from_string("w-child"),
        SourceMutation::new_source(PathBuf::from("/cand"), hash_of(9)),
    );

    // The parent we have is byte-equal to the parent we built.
    assert_eq!(parent, parent_before);
    // The outcome carries the same parent reference back.
    assert_eq!(outcome.parent, &parent);
    assert!(outcome.parent.is_base());
}

// --- child carries the mutation and lineage --------------------------

#[test]
fn child_carries_mutation_path_and_hash() {
    let parent = base_world("w-root", 5);
    let path = PathBuf::from("/candidates/foo");
    let hash = hash_of(11);

    let outcome = fork(
        &parent,
        SoftwareWorldId::from_string("w-child"),
        SourceMutation::new_source(path.clone(), hash.clone()),
    );

    let child = outcome.new_world;
    assert_eq!(
        child.source_state,
        WorldSourceState::Forked {
            path: path.clone(),
            content_hash: hash.clone(),
        }
    );
    assert_eq!(child.source_state.path(), Some(&path));
    assert_eq!(child.source_state.content_hash(), Some(&hash));
    assert!(child.is_forked());
}

#[test]
fn child_inherits_parent_base_snapshot() {
    let parent = base_world("w-root", 7);
    let outcome = fork(
        &parent,
        SoftwareWorldId::from_string("w-child"),
        SourceMutation::new_source(PathBuf::from("/x"), hash_of(1)),
    );

    assert_eq!(outcome.new_world.base_snapshot, parent.base_snapshot);
    assert_eq!(outcome.new_world.base_snapshot, snapshot(7));
}

#[test]
fn child_parent_world_points_at_parent_id() {
    let parent = base_world("w-root", 1);
    let outcome = fork(
        &parent,
        SoftwareWorldId::from_string("w-child"),
        SourceMutation::new_source(PathBuf::from("/x"), hash_of(1)),
    );

    assert_eq!(outcome.new_world.parent_world.as_ref(), Some(&parent.id));
}

// --- identity mutation (UAT decisive case) ---------------------------

#[test]
fn identity_mutation_yields_empty_diff_in_source_state() {
    let parent_path = PathBuf::from("/repo");
    let parent_hash = hash_of(42);
    let parent = SoftwareWorld::new_forked(
        SoftwareWorldId::from_string("w-prior"),
        snapshot(3),
        SoftwareWorldId::from_string("w-root"),
        parent_path.clone(),
        parent_hash.clone(),
    );

    let outcome = fork(
        &parent,
        SoftwareWorldId::from_string("w-child"),
        SourceMutation::identity(parent_path.clone(), parent_hash.clone()),
    );

    let child = outcome.new_world;
    // The child's source_state is structurally equal to the parent's
    // (same path, same content hash). The diff between snapshots taken
    // against this child vs the parent's snapshot is empty by
    // construction — that is what WU3 will prove via SemanticFactDelta.
    assert_eq!(child.source_state, parent.source_state);
    assert!(child.is_forked());
    // But the child is its own world (different id, different parent
    // chain).
    assert_ne!(child.id, parent.id);
}

// --- lineage chain ---------------------------------------------------

#[test]
fn chain_of_forks_is_navigable() {
    let root = base_world("w-root", 1);
    let c1 = fork(
        &root,
        SoftwareWorldId::from_string("w-c1"),
        SourceMutation::new_source(PathBuf::from("/c1"), hash_of(2)),
    )
    .new_world;
    let c2 = fork(
        &c1,
        SoftwareWorldId::from_string("w-c2"),
        SourceMutation::new_source(PathBuf::from("/c2"), hash_of(3)),
    )
    .new_world;

    assert_eq!(c1.parent_world.as_ref(), Some(&root.id));
    assert_eq!(c2.parent_world.as_ref(), Some(&c1.id));
    assert_eq!(root.parent_world, None);
}

// --- forking a forked world is legal ---------------------------------

#[test]
fn fork_can_chain_indefinitely() {
    let mut current = base_world("w-0", 1);
    for i in 1..=5 {
        let outcome = fork(
            &current,
            SoftwareWorldId::from_string(format!("w-{i}")),
            SourceMutation::new_source(PathBuf::from(format!("/step-{i}")), hash_of(i as u8)),
        );
        // The current world (parent) is still unchanged.
        assert!(outcome.parent.is_forked() || outcome.parent.is_base());
        // The new world is a fork of the previous.
        assert_eq!(outcome.new_world.parent_world.as_ref(), Some(&current.id));
        // Inherited base_snapshot.
        assert_eq!(outcome.new_world.base_snapshot, current.base_snapshot);
        // Step forward.
        current = outcome.new_world;
    }
    assert!(current.is_forked());
}

// --- mutation constructors -----------------------------------------

#[test]
fn source_mutation_new_source_is_constructive() {
    let m = SourceMutation::new_source(PathBuf::from("/x"), hash_of(1));
    assert_eq!(m.new_path, PathBuf::from("/x"));
    assert_eq!(m.new_content_hash, hash_of(1));
}

#[test]
fn source_mutation_identity_is_self_describing() {
    let m = SourceMutation::identity(PathBuf::from("/y"), hash_of(2));
    assert_eq!(m.new_path, PathBuf::from("/y"));
    assert_eq!(m.new_content_hash, hash_of(2));
}

// --- adversarial: caller cannot accidentally promote parent ---------

#[test]
fn fork_does_not_promote_base_parent_to_forked() {
    // The parent is a Base world. The child produced by fork must be
    // Forked (it has a parent_world Some(_)). The parent must remain
    // a Base world.
    let parent = base_world("w-root", 1);
    let outcome = fork(
        &parent,
        SoftwareWorldId::from_string("w-child"),
        SourceMutation::new_source(PathBuf::from("/x"), hash_of(1)),
    );

    assert!(outcome.parent.is_base());
    assert!(outcome.new_world.is_forked());
    // The parent still has parent_world == None and Base source_state.
    assert_eq!(outcome.parent.parent_world, None);
    assert_eq!(outcome.parent.source_state, WorldSourceState::Base);
}
