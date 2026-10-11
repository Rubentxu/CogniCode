//! T1-5.5 — CogniCode CLI library.
//!
//! `cognicode-cli` was historically a binary-only crate
//! (`autobins = false`, three bins: `cognicode`, `cogh`,
//! `cognicode-release`). This lib target exists so the bin
//! can publish its presenter module (`pub mod presenter`)
//! for integration testing from `crates/cognicode-cli/tests/`,
//! matching the standard bin+lib Rust pattern.
//!
//! Before T1-5.5 the presenter lived inside `cognicode-core`
//! (a layering violation: the lib should not format). Moving
//! it here puts the presenter on the same crate as the bin
//! that actually emits output.
//!
//! Public surface (intentionally minimal):
//! - `presenter`: text + JSON dispatch for `CommandOutput`.
//! - `identity`: B1.W2 — version + source SHA + compiled features.

pub use cognicode_core;

pub mod presenter;
pub mod identity;