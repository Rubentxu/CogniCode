//! Concrete `Clock` adapters for the behavior runtime (M7.4, cycle e65).
//!
//! The `Clock` *trait* is owned by the domain (`crate::domain::behaviors::ports`).
//! This module provides the production adapter (`SystemClock`) and the
//! deterministic test double (`MockClock`) that satisfy the trait.
//!
//! Application layer responsibility: provide monotonic, `Send + Sync`
//! implementations without forcing the domain to depend on `std::time`
//! or `tokio`. `SystemClock` uses `std::time::Instant` (no `async`,
//! no runtime dependency); `MockClock` is advanceable and deterministic.

use crate::domain::behaviors::ports::Clock;

/// System wall-clock using `std::time::Instant`.
///
/// No `tokio` dependency. `Instant` is monotonic on all platforms that matter.
pub struct SystemClock {
    started: std::time::Instant,
}

impl SystemClock {
    /// Construct a clock anchored to the current `Instant`.
    pub fn new() -> Self {
        Self {
            started: std::time::Instant::now(),
        }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_millis(&self) -> u64 {
        self.started.elapsed().as_millis() as u64
    }
}

/// A deterministic, advanceable clock for tests.
///
/// Advances by a fixed step each call to `advance`, simulating time passing
/// without actual wall-clock waits.
#[cfg(any(test, feature = "test-support"))]
pub struct MockClock {
    now: u64,
    step_millis: u64,
}

#[cfg(any(test, feature = "test-support"))]
impl MockClock {
    /// Construct a mock clock starting at `now` milliseconds.
    pub fn new(now: u64) -> Self {
        Self {
            now,
            step_millis: 10, // default step: 10ms
        }
    }

    /// Construct starting at 0.
    pub fn start() -> Self {
        Self::new(0)
    }

    /// Advance the clock by `step_millis` milliseconds.
    pub fn advance(&mut self) {
        self.now += self.step_millis;
    }

    /// Advance by a specific amount.
    pub fn advance_by(&mut self, millis: u64) {
        self.now += millis;
    }

    /// Set the step used by `advance()`.
    pub fn set_step(&mut self, step_millis: u64) {
        self.step_millis = step_millis;
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Clock for MockClock {
    fn now_millis(&self) -> u64 {
        self.now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_does_not_panic() {
        let clock = SystemClock::new();
        let t1 = clock.now_millis();
        assert!(t1 >= 0);
        let t2 = clock.now_millis();
        assert!(t2 >= t1);
    }

    #[test]
    fn mock_clock_is_deterministic() {
        let mut clock = MockClock::new(0);
        assert_eq!(clock.now_millis(), 0);
        clock.advance();
        assert_eq!(clock.now_millis(), 10);
        clock.advance_by(50);
        assert_eq!(clock.now_millis(), 60);
    }

    #[test]
    fn mock_clock_advance_is_monotonic() {
        let mut clock = MockClock::new(0);
        let mut prev = 0u64;
        for _ in 0..100 {
            clock.advance();
            let curr = clock.now_millis();
            assert!(curr > prev);
            prev = curr;
        }
    }

    #[test]
    fn mock_clock_start_at_zero() {
        let clock = MockClock::start();
        assert_eq!(clock.now_millis(), 0);
    }
}
