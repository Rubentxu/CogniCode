//! Software world types (e71 WU1 — M9).
//!
//! Pure domain types for representing an isolated, lineage-tracked
//! derivation context. See [`crate::application::software_world`] for the
//! module-level rationale (why this is not a second truth store, why
//! `source_state` is descriptive and not materialized, and why this is
//! an application-layer type).

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::evidence_kernel::ids::SnapshotId;

/// Stable identifier of a [`SoftwareWorld`].
///
/// Wrapped string so it can be carried across serialization boundaries
/// (audit trails, trial evidence, promotion records) without depending
/// on a specific UUID crate at the API surface. Constructed by the
/// caller; this module never mints IDs at construction time, so the
/// type is deterministic and test-friendly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SoftwareWorldId(String);

impl SoftwareWorldId {
    /// Construct a `SoftwareWorldId` from any string-shaped identifier.
    ///
    /// The caller owns the choice of identifier scheme. The kernel never
    /// mints a world id; this type is the bookkeeping identity.
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SoftwareWorldId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable 256-bit content digest carried by a forked [`SoftwareWorld`].
///
/// Wrapped to make digest handling explicit at every call site (rather
/// than letting `[u8; 32]` float around). The digest is *descriptive* —
/// it records what was measured, but the world does not compute it.
/// Computation lives in the filesystem / content-addressed layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash(pub [u8; 32]);

impl ContentHash {
    /// Hex-encode the digest in lower-case. Stable across runs.
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for byte in &self.0 {
            use std::fmt::Write as _;
            let _ = write!(&mut out, "{:02x}", byte);
        }
        out
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// The `source_state` half of a [`SoftwareWorld`].
///
/// Two variants, with a hard invariant on construction (see
/// [`SoftwareWorld::new_base`] and [`SoftwareWorld::new_forked`]):
///
/// - `Base`: this world is the canonical derivation root. There is no
///   parent, and `source_state` is empty.
/// - `Forked`: this world derives from a parent. The `path` is a
///   logical reference to the source being measured; the `content_hash`
///   is the stable digest of the source content as analysed.
///
/// The path is descriptive — it does not imply that any directory or
/// file exists at that location. Worlds do not materialize their source.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorldSourceState {
    /// Base world: no parent, no source reference.
    Base,
    /// Forked world: derives from a parent; carries a logical source
    /// reference and a stable content digest.
    Forked {
        /// Logical identifier of the source under analysis (e.g.
        /// workspace-relative path or commit URL).
        path: PathBuf,
        /// Stable digest of the source content.
        content_hash: ContentHash,
    },
}

impl WorldSourceState {
    /// True iff this is a [`WorldSourceState::Base`].
    pub fn is_base(&self) -> bool {
        matches!(self, WorldSourceState::Base)
    }

    /// True iff this is a [`WorldSourceState::Forked`].
    pub fn is_forked(&self) -> bool {
        matches!(self, WorldSourceState::Forked { .. })
    }

    /// Borrow the forked `path` (if any).
    pub fn path(&self) -> Option<&PathBuf> {
        match self {
            WorldSourceState::Base => None,
            WorldSourceState::Forked { path, .. } => Some(path),
        }
    }

    /// Borrow the forked `content_hash` (if any).
    pub fn content_hash(&self) -> Option<&ContentHash> {
        match self {
            WorldSourceState::Base => None,
            WorldSourceState::Forked { content_hash, .. } => Some(content_hash),
        }
    }
}

/// Isolated, lineage-tracked derivation context for an analysis target.
///
/// A world has exactly three responsibilities:
///
/// 1. It names itself (`id`).
/// 2. It remembers what canonical snapshot it was measured against
///    (`base_snapshot`). This is the *only* reference to canonical
///    truth; it is a [`SnapshotId`], bijective with a `RevisionId`.
/// 3. It remembers how it was produced (`parent_world`, `source_state`).
///
/// It does NOT remember facts, evidence, or work results. Those live
/// elsewhere (kernel, e69 bundles, e70 reports). A world is a receipt
/// for "where this derivation came from", not a record of what it found.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SoftwareWorld {
    /// Stable identifier of this world.
    pub id: SoftwareWorldId,
    /// Snapshot against which the candidate was (or will be) measured.
    pub base_snapshot: SnapshotId,
    /// Parent world (None for a base world, Some for a forked world).
    pub parent_world: Option<SoftwareWorldId>,
    /// How this world was produced.
    pub source_state: WorldSourceState,
}

impl SoftwareWorld {
    /// Construct a base world.
    ///
    /// `parent_world` MUST be `None`; `source_state` MUST be `Base`. The
    /// type system cannot enforce the latter on its own, so this
    /// constructor rejects mismatches with a panic (fail-closed at
    /// construction; the only ways to produce a malformed world are to
    /// bypass the constructor or to construct manually, both of which
    /// are local and easy to audit).
    pub fn new_base(id: SoftwareWorldId, base_snapshot: SnapshotId) -> Self {
        Self {
            id,
            base_snapshot,
            parent_world: None,
            source_state: WorldSourceState::Base,
        }
    }

    /// Construct a forked world.
    ///
    /// `parent_world` MUST be `Some(_)`; `source_state` MUST be
    /// `Forked { .. }`. Same fail-closed construction rule as
    /// [`new_base`](Self::new_base).
    pub fn new_forked(
        id: SoftwareWorldId,
        base_snapshot: SnapshotId,
        parent_world: SoftwareWorldId,
        path: PathBuf,
        content_hash: ContentHash,
    ) -> Self {
        Self {
            id,
            base_snapshot,
            parent_world: Some(parent_world),
            source_state: WorldSourceState::Forked { path, content_hash },
        }
    }

    /// True iff this is a base world (`parent_world.is_none()` and
    /// `source_state.is_base()`).
    pub fn is_base(&self) -> bool {
        self.parent_world.is_none() && self.source_state.is_base()
    }

    /// True iff this is a forked world (`parent_world.is_some()` and
    /// `source_state.is_forked()`).
    pub fn is_forked(&self) -> bool {
        self.parent_world.is_some() && self.source_state.is_forked()
    }
}
