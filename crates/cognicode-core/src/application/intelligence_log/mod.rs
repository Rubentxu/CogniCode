//! Intelligence Event Log application helpers (M7, cycle e63).

pub mod recorder;

pub use recorder::{CausalRecorder, counted_payload, summary_payload};
