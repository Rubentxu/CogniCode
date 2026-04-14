//! Refactoring Test Fixture - Rust
//!
//! This fixture is designed for testing safe_refactor actions: extract, inline, move, change_signature.
//!
//! Contains functions and structures that are refactoring targets.
//!
//! Ground truth for behavioral preservation: tests must pass after each refactoring.

/// Calculates the sum of two numbers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Calculates the difference of two numbers.
pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}

/// Calculates the product of two numbers.
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

/// Calculates the quotient of two numbers.
pub fn divide(a: i32, b: i32) -> i32 {
    a / b
}

/// A simple calculator struct.
pub struct SimpleCalc {
    value: i32,
}

impl SimpleCalc {
    /// Creates a new SimpleCalc.
    pub fn new() -> Self {
        SimpleCalc { value: 0 }
    }

    /// Sets the value.
    pub fn set_value(&mut self, val: i32) {
        self.value = val;
    }

    /// Gets the current value.
    pub fn get_value(&self) -> i32 {
        self.value
    }

    /// Adds to the value.
    pub fn add(&mut self, amount: i32) {
        self.value += amount;
    }
}

impl Default for SimpleCalc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(-1, 1), 0);
        assert_eq!(add(0, 0), 0);
    }

    #[test]
    fn test_subtract() {
        assert_eq!(subtract(5, 3), 2);
        assert_eq!(subtract(3, 5), -2);
    }

    #[test]
    fn test_multiply() {
        assert_eq!(multiply(3, 4), 12);
        assert_eq!(multiply(-2, 3), -6);
    }

    #[test]
    fn test_divide() {
        assert_eq!(divide(10, 2), 5);
        assert_eq!(divide(9, 3), 3);
    }

    #[test]
    fn test_simple_calc() {
        let mut calc = SimpleCalc::new();
        assert_eq!(calc.get_value(), 0);
        calc.set_value(10);
        assert_eq!(calc.get_value(), 10);
        calc.add(5);
        assert_eq!(calc.get_value(), 15);
    }
}
