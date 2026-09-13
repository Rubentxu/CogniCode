//! In-memory adapters for the evidence-kernel ports (E36 M1, design D7).
//!
//! No fact producer exists yet, so there is no LadybugDB adapter in this
//! slice (deferred per design D7): these adapters back tests and the future
//! feature-gated runtime wiring. Everything here is feature-gated behind
//! `evidence-kernel` (off by default).

#[cfg(feature = "evidence-kernel")]
pub mod in_memory;

#[cfg(feature = "evidence-kernel")]
pub use in_memory::{
    InMemoryEvidenceStore, InMemoryFactStore, InMemorySchemaRegistry, InMemorySnapshotStore,
};
