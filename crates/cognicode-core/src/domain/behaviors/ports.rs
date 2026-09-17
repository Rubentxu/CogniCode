//! Domain ports for the behavior runtime.
//!
//! `Clock` is the only capability the domain needs from the application
//! layer: a monotonic time source for budget measurement. Keeping the
//! trait here makes the boundary explicit — `BehaviorRuntime` depends on
//! `&dyn Clock`, and concrete implementations (`SystemClock`,
//! `MockClock`) live in the application layer / test-support.
//!
//! Why this lives in `domain`:
//!   * The domain defines what it needs (a `u64` monotonic ms counter).
//!   * The domain never imports `tokio` or `std::time`; that is the
//!     application's job to satisfy through an adapter.
//!   * The trait is `Send + Sync` so async runtimes can hold it.

/// A monotonic wall-clock for budget measurement.
///
/// Implementors must guarantee that [`Clock::now_millis`] never decreases
/// between calls within a single execution (monotonicity).
/// `SystemClock` (in the application layer) uses `std::time::Instant`,
/// which satisfies this. `MockClock` (test-support) satisfies it by
/// construction.
pub trait Clock: Send + Sync {
    /// The current time in monotonic milliseconds.
    fn now_millis(&self) -> u64;
}
