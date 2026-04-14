//! Call Graph Test Fixture - Rust
//!
//! This fixture has known call relationships for testing call graph tools.
//!
//! Call graph structure:
//!   main → helper → compute
//!   main → process
//!
//! Ground truth:
//!   - Entry points: [main]
//!   - Leaf functions: [compute, process, helper_internal]
//!   - Edges: main→helper, helper→compute, main→process, helper→helper_internal

/// Main entry point - calls helper and process.
pub fn main() {
    let result = helper(42);
    process(result);
}

/// Helper function - calls compute and helper_internal.
pub fn helper(x: i32) -> i32 {
    let a = compute(x);
    helper_internal(a)
}

/// Compute function - leaf node (no outgoing edges).
pub fn compute(x: i32) -> i32 {
    x * 2
}

/// Process function - leaf node (no outgoing edges).
pub fn process(value: i32) {
    println!("Result: {}", value);
}

/// Internal helper - leaf node (private, no outgoing edges).
fn helper_internal(x: i32) -> i32 {
    x + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main() {
        main();
    }

    #[test]
    fn test_helper() {
        assert_eq!(helper(5), 11); // 5*2 + 1
    }

    #[test]
    fn test_compute() {
        assert_eq!(compute(5), 10);
    }
}
