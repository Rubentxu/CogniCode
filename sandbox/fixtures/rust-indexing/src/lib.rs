//! Indexing Test Fixture - Rust
//!
//! This fixture is designed for testing build_lightweight_index and query_symbol_index tools.
//! It contains multiple source files with known symbols.
//!
//! Ground truth symbols (indexed across all files):
//!   - greet (function) - src/lib.rs:7
//!   - add (function) - src/lib.rs:12
//!   - multiply (function) - src/lib.rs:17
//!   - helper_internal (function, private) - src/lib.rs:24
//!   - Calculator (struct) - src/lib.rs:29
//!   - Calculator::new (method) - src/lib.rs:35
//!   - Calculator::add (method) - src/lib.rs:41
//!   - Calculator::value (method) - src/lib.rs:46
//!   - process (function) - src/utils.rs:5
//!   - format_result (function) - src/utils.rs:12

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

    #[test]
    fn test_multiply() {
        assert_eq!(multiply(3, 4), 12);
    }

    #[test]
    fn test_calculator() {
        let mut calc = Calculator::new(10);
        calc.add(5);
        assert_eq!(calc.value(), 15);
    }
}
