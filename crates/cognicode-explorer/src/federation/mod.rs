//! Federation primitives — re-exports from `cognicode-core`.
//!
//! These types used to live in this crate. Phase 1 of the Graph
//! Intelligence v2 roadmap moved them into `cognicode-core`
//! (behind the `multimodal` feature gate) so future graph adapters
//! in any consumer crate can compose against a canonical
//! federation layer. The local files in `src/federation/` are
//! kept as thin re-exports so the explorer's existing
//! `crate::federation::*` import paths continue to work without
//! breakage.
//!
//! Every re-export is feature-gated behind the `multimodal` Cargo
//! feature. On a default build the entire `federation` module is
//! absent from the crate, so the byte-level shape of the public
//! surface is unchanged.

#[cfg(feature = "multimodal")]
pub use cognicode_core::domain::federation::*;
