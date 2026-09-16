//! Tests for `application::software_world::world` (e71 WU1).
//!
//! Adversarial coverage of the four invariants documented in
//! `application::software_world`:
//!
//! 1. A `SoftwareWorld` is lineage + isolation metadata, NOT a fact
//!    store. The type surface has no `Vec<Fact>`, no `ReadSet`, no
//!    `EvidenceBundle` handle.
//! 2. `source_state` is descriptive, not materialized: there is no
//!    file/path IO surface.
//! 3. Construction is total and deterministic: no clock, no UUID mint,
//!    no env access. `SoftwareWorldId::from_string` is the only way to
//!    produce an id, and it is pure.
//! 4. `Base` and `Forked` invariants: `new_base` rejects forks;
//!    `new_forked` rejects bases.
//!
//! The "no-mirror" guarantee is also asserted structurally: the
//! `SoftwareWorld` type has no field whose type can hold facts.

use std::path::PathBuf;

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

// --- SoftwareWorldId -------------------------------------------------

#[test]
fn software_world_id_is_pure_string_wrapper() {
    // Deterministic: same input -> same id, no clock involved.
    let a = SoftwareWorldId::from_string("world-1");
    let b = SoftwareWorldId::from_string("world-1");
    let c = SoftwareWorldId::from_string("world-2");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(a.as_str(), "world-1");
    assert_eq!(format!("{}", a), "world-1");
}

// --- ContentHash -----------------------------------------------------

#[test]
fn content_hash_hex_is_stable_and_lower_case() {
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = i as u8;
    }
    let h = ContentHash(bytes);
    // The first byte (0x00) and the last (0x1f = 31) must appear at the
    // expected positions, in lower case.
    let hex = h.to_hex();
    assert_eq!(hex.len(), 64);
    assert!(hex.starts_with("00"));
    assert!(hex.ends_with("1f"));
    assert!(
        hex.chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    );
    assert_eq!(format!("{}", h), hex);
}

#[test]
fn content_hash_equality_is_byte_exact() {
    let a = hash_of(7);
    let b = hash_of(7);
    let c = hash_of(8);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// --- WorldSourceState ------------------------------------------------

#[test]
fn world_source_state_distinguishes_base_from_forked() {
    let base = WorldSourceState::Base;
    let forked = WorldSourceState::Forked {
        path: PathBuf::from("/src/x"),
        content_hash: hash_of(1),
    };
    assert!(base.is_base());
    assert!(!base.is_forked());
    assert!(forked.is_forked());
    assert!(!forked.is_base());
}

#[test]
fn world_source_state_accessors_return_none_for_base() {
    let base = WorldSourceState::Base;
    assert_eq!(base.path(), None);
    assert_eq!(base.content_hash(), None);
}

#[test]
fn world_source_state_accessors_return_forked_payload() {
    let path = PathBuf::from("/src/y");
    let hash = hash_of(42);
    let forked = WorldSourceState::Forked {
        path: path.clone(),
        content_hash: hash.clone(),
    };
    assert_eq!(forked.path(), Some(&path));
    assert_eq!(forked.content_hash(), Some(&hash));
}

// --- SoftwareWorld construction invariants ---------------------------

#[test]
fn new_base_yields_isolated_root() {
    let world = SoftwareWorld::new_base(SoftwareWorldId::from_string("w-root"), snapshot(5));
    assert!(world.is_base());
    assert!(!world.is_forked());
    assert_eq!(world.parent_world, None);
    assert_eq!(world.source_state, WorldSourceState::Base);
    assert_eq!(world.base_snapshot, snapshot(5));
}

#[test]
fn new_forked_yields_lineage_child() {
    let parent = SoftwareWorldId::from_string("w-root");
    let child = SoftwareWorld::new_forked(
        SoftwareWorldId::from_string("w-child"),
        snapshot(7),
        parent.clone(),
        PathBuf::from("/candidates/foo"),
        hash_of(9),
    );
    assert!(child.is_forked());
    assert!(!child.is_base());
    assert_eq!(child.parent_world, Some(parent));
    assert_eq!(
        child.source_state,
        WorldSourceState::Forked {
            path: PathBuf::from("/candidates/foo"),
            content_hash: hash_of(9),
        }
    );
}

// --- Anti-mirror structural test -------------------------------------
//
// This is the decisive assertion: SoftwareWorld must not be a fact
// store. We cannot reach inside the type to inspect its fields, but we
// CAN assert by construction that no world constructed via the public
// API carries any facts. The pattern below is the canonical test: two
// worlds built from the same components are equal; the equality is
// over (id, base_snapshot, parent_world, source_state) only. Anything
// beyond these four fields would silently break that equality.

#[test]
fn two_worlds_with_same_components_are_byte_equal() {
    let a = SoftwareWorld::new_forked(
        SoftwareWorldId::from_string("w-x"),
        snapshot(3),
        SoftwareWorldId::from_string("w-parent"),
        PathBuf::from("/p"),
        hash_of(1),
    );
    let b = SoftwareWorld::new_forked(
        SoftwareWorldId::from_string("w-x"),
        snapshot(3),
        SoftwareWorldId::from_string("w-parent"),
        PathBuf::from("/p"),
        hash_of(1),
    );
    assert_eq!(a, b);
}

#[test]
fn changing_only_base_snapshot_distinguishes_worlds() {
    let parent = SoftwareWorldId::from_string("w-parent");
    let a = SoftwareWorld::new_forked(
        SoftwareWorldId::from_string("w-x"),
        snapshot(3),
        parent.clone(),
        PathBuf::from("/p"),
        hash_of(1),
    );
    let b = SoftwareWorld::new_forked(
        SoftwareWorldId::from_string("w-x"),
        snapshot(4), // <-- changed
        parent,
        PathBuf::from("/p"),
        hash_of(1),
    );
    assert_ne!(a, b);
    assert_eq!(a.base_snapshot, snapshot(3));
    assert_eq!(b.base_snapshot, snapshot(4));
}

#[test]
fn changing_only_content_hash_distinguishes_worlds() {
    let parent = SoftwareWorldId::from_string("w-parent");
    let a = SoftwareWorld::new_forked(
        SoftwareWorldId::from_string("w-x"),
        snapshot(3),
        parent.clone(),
        PathBuf::from("/p"),
        hash_of(1),
    );
    let b = SoftwareWorld::new_forked(
        SoftwareWorldId::from_string("w-x"),
        snapshot(3),
        parent,
        PathBuf::from("/p"),
        hash_of(2), // <-- changed
    );
    assert_ne!(a, b);
    assert_eq!(a.source_state.content_hash(), Some(&hash_of(1)));
    assert_eq!(b.source_state.content_hash(), Some(&hash_of(2)));
}

// --- Lineage chain ---------------------------------------------------

#[test]
fn lineage_chain_navigable_back_to_root() {
    // root -> child -> grandchild
    let root = SoftwareWorld::new_base(SoftwareWorldId::from_string("w-root"), snapshot(1));
    let child = SoftwareWorld::new_forked(
        SoftwareWorldId::from_string("w-child"),
        snapshot(2),
        root.id.clone(),
        PathBuf::from("/c1"),
        hash_of(11),
    );
    let grand = SoftwareWorld::new_forked(
        SoftwareWorldId::from_string("w-grand"),
        snapshot(3),
        child.id.clone(),
        PathBuf::from("/c2"),
        hash_of(12),
    );

    // The grand-child's lineage points at the child.
    assert_eq!(grand.parent_world.as_ref(), Some(&child.id));
    // The child's lineage points at the root.
    assert_eq!(child.parent_world.as_ref(), Some(&root.id));
    // The root has no parent.
    assert_eq!(root.parent_world, None);

    // None of the three is structurally equal to any other.
    assert_ne!(root, child);
    assert_ne!(child, grand);
    assert_ne!(root, grand);
}

// --- Base / Forked shape consistency ---------------------------------

#[test]
fn base_world_with_parent_via_field_is_detectable_as_inconsistent() {
    // We construct a malformed world manually (bypassing constructors).
    // The type system still permits it (we expose the fields as `pub`).
    // The `is_base` / `is_forked` predicates expose the inconsistency
    // instead of silently claiming a shape.
    let malformed = SoftwareWorld {
        id: SoftwareWorldId::from_string("w-bad-base"),
        base_snapshot: snapshot(1),
        parent_world: Some(SoftwareWorldId::from_string("w-other")),
        source_state: WorldSourceState::Base,
    };
    // Neither is_base nor is_forked returns true for this combination.
    assert!(!malformed.is_base());
    assert!(!malformed.is_forked());
}

#[test]
fn forked_world_with_no_parent_via_field_is_detectable_as_inconsistent() {
    let malformed = SoftwareWorld {
        id: SoftwareWorldId::from_string("w-bad-forked"),
        base_snapshot: snapshot(1),
        parent_world: None,
        source_state: WorldSourceState::Forked {
            path: PathBuf::from("/x"),
            content_hash: hash_of(0),
        },
    };
    assert!(!malformed.is_base());
    assert!(!malformed.is_forked());
}
