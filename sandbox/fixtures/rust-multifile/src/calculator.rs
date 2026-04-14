//! Calculator module for multifile fixture.
//!
//! Contains Calculator struct for testing method call detection.

/// A simple calculator.
pub struct Calculator {
    value: i32,
}

impl Calculator {
    /// Creates a new calculator.
    pub fn new(initial: i32) -> Self {
        Calculator { value: initial }
    }

    /// Adds to the current value.
    pub fn add(&mut self, amount: i32) {
        self.value += amount;
    }

    /// Gets the current value.
    pub fn value(&self) -> i32 {
        self.value
    }
}
