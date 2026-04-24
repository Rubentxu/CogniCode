//! Debug Test Fixture - Rust
//!
//! This crate contains various crash scenarios for testing debug_analyze().
//! Each function panics in a different way to test different error detection scenarios.

/// Panics with index out of bounds
pub fn crash_index_oob() {
    let v = vec![1, 2, 3];
    let _ = v[10]; // PANIC: index out of bounds: the len is `3` but the index is `10`
}

/// Panics with unwrap on None
pub fn crash_unwrap_none() {
    let opt: Option<i32> = None;
    opt.unwrap(); // PANIC: called `Option::unwrap()` on a `None` value
}

/// Panics with expect on None
pub fn crash_expect_none() {
    let opt: Option<&str> = None;
    opt.expect("Expected a value"); // PANIC: Expected a value
}

/// Division by zero panic
pub fn crash_divzero() {
    let v = vec![1, 2, 3];
    let divisor = v.get(5).copied().unwrap_or(0);
    let _ = 10 / divisor; // PANIC: division by zero
}

/// Assertion failure
pub fn crash_assert() {
    assert_eq!(1, 2, "Numbers should be equal");
}

/// Custom panic message
pub fn crash_custom_panic() {
    panic!("This is a custom panic message for testing");
}

/// Out of bounds slice access
pub fn crash_slice_oob() {
    let arr = [1, 2, 3, 4, 5];
    let _slice = &arr[2..10]; // PANIC: range end index 10 out of range for slice of 5 elements
}

/// HashMap key not found (if we use a similar pattern)
pub fn crash_vec_pop_empty() {
    let mut v: Vec<i32> = vec![];
    v.pop(); // PANIC: called `Vec::pop()` on an empty `Vec`
}

/// Integer overflow panic (in debug mode)
pub fn crash_overflow() {
    let i: u8 = 255;
    let _ = i.wrapping_add(1); // PANIC: attempt to add with overflow
}

/// Option unwrap in a chain
pub fn crash_nested_unwrap() {
    let result: Result<i32, &str> = Err("error");
    let _value = result.unwrap(); // PANIC: called `Result::unwrap()` on an `Err` value: "error"
}

// =============================================================================
// Main entry points for binary testing
// =============================================================================

fn main() {
    use std::env;

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: crash_demo <crash_type>");
        eprintln!("Available crash types:");
        eprintln!("  index_oob       - Index out of bounds");
        eprintln!("  unwrap_none     - Unwrap on None");
        eprintln!("  expect_none     - Expect on None");
        eprintln!("  divzero         - Division by zero");
        eprintln!("  assert          - Assertion failure");
        eprintln!("  custom_panic    - Custom panic message");
        eprintln!("  slice_oob       - Slice range out of bounds");
        eprintln!("  pop_empty       - Pop from empty vector");
        eprintln!("  overflow        - Integer overflow");
        eprintln!("  nested_unwrap   - Nested unwrap Result::Err");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "index_oob" => crash_index_oob(),
        "unwrap_none" => crash_unwrap_none(),
        "expect_none" => crash_expect_none(),
        "divzero" => crash_divzero(),
        "assert" => crash_assert(),
        "custom_panic" => crash_custom_panic(),
        "slice_oob" => crash_slice_oob(),
        "pop_empty" => crash_vec_pop_empty(),
        "overflow" => crash_overflow(),
        "nested_unwrap" => crash_nested_unwrap(),
        _ => {
            eprintln!("Unknown crash type: {}", args[1]);
            std::process::exit(1);
        }
    }
}
