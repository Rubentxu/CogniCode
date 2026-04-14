//! Code Intelligence Test Fixture - Rust
//!
//! This file is designed for testing symbol extraction, outline generation,
//! and symbol code retrieval. Ground truth is documented in the manifest.

/// A simple greeting function.
pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

/// Adds two numbers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Multiplies two numbers.
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

/// Internal helper function (private).
fn helper_internal(x: i32) -> i32 {
    x * 2
}

/// A calculator struct demonstrating impl blocks.
pub struct Calculator {
    value: i32,
}

impl Calculator {
    /// Creates a new calculator with initial value.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        assert_eq!(greet("World"), "Hello, World!");
    }

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }
}
