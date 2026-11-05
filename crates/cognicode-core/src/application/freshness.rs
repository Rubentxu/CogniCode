//! Who else can say that a manifest still describes the tree.
//!
//! ## Why this is a port
//!
//! The application decides whether a graph is served or calculated again,
//! and it decides that by reading the content of every file in the tree.
//! A daemon that keeps the index warm can say the same thing without the
//! reading —but a daemon is an adapter, and the application must not know
//! how it is reached—. So the application declares what it needs: a thing
//! that can vouch for a manifest. The interface supplies it, and the
//! application never learns whether the vouch came from a daemon, from a
//! file, or from nowhere at all.
//!
//! ## What a vouch is worth
//!
//! A vouch is about a tree state, and it says so: the manifest it is
//! about. A vouch about some other tree says nothing about this one, so
//! the adapter that brings it is the one that has to check that it is
//! about this manifest.
//!
//! ## When it is asked
//!
//! The vouch is the last question, not the first: the cheap checks —the
//! walk that says what holes the tree has, and the modification times
//! that say whether a file changed— are answered first, and only a tree
//! that passes them is worth the expensive answer.

use crate::domain::value_objects::file_manifest::FileManifest;

/// Somebody who can say that a manifest still describes the tree.
///
/// The answer is asked only when the cheap checks already agree, so the
/// expensive answer is the last one and not the first.
pub trait FreshnessOracle: Send + Sync {
    /// Whether the manifest still describes the tree, according to
    /// somebody who is watching it.
    fn vouches_for(&self, manifest: &FileManifest) -> bool;
}
