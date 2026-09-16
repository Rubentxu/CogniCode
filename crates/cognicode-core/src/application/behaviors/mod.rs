//! Application layer adapters for behaviors (M7.4, cycle e65).

pub mod clock;

pub use clock::{Clock, MockClock, SystemClock};
