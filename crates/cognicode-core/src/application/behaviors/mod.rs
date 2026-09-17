//! Application layer adapters for behaviors (M7.4, cycle e65).

pub mod clock;

// Re-export the domain-owned `Clock` trait so callers that already use
// `crate::application::behaviors::*` (a stable path) keep compiling. The
// canonical home of the trait is `crate::domain::behaviors::ports::Clock`.
pub use crate::domain::behaviors::ports::Clock;
pub use clock::SystemClock;

#[cfg(any(test, feature = "test-support"))]
pub use clock::MockClock;
