//! Re-export shim (cycle e56).
//!
//! The canonical kernel identifiers moved to the **ungated**
//! [`crate::domain::kernel_ids`] because they are fundamental domain
//! vocabulary, not gated infrastructure. This module is kept so every
//! existing `evidence_kernel::ids::*` path continues to resolve (single
//! source of truth, no duplicated definitions).

pub use crate::domain::kernel_ids::*;
