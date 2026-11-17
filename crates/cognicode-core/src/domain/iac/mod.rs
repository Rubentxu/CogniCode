//! Infrastructure-as-code models: pure domain, no I/O.
//!
//! The adapters that read files live under `infrastructure`; what a
//! configuration *means* lives here.

pub mod helm;
pub mod terraform;
