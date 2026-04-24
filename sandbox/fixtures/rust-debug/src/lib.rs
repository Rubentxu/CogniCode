//! Debug Test Fixture Library
//!
//! Exposes crash functions for library testing.

/// Panics with index out of bounds
pub fn crash_index_oob() {
    let v = vec![1, 2, 3];
    let _ = v[10];
}

/// Panics with unwrap on None
pub fn crash_unwrap_none() {
    let opt: Option<i32> = None;
    opt.unwrap();
}

/// Panics with expect on None
pub fn crash_expect_none() {
    let opt: Option<&str> = None;
    opt.expect("Expected a value");
}

/// Division by zero panic
pub fn crash_divzero() {
    let v = vec![1, 2, 3];
    let divisor = v.get(5).copied().unwrap_or(0);
    let _ = 10 / divisor;
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
    let _slice = &arr[2..10];
}

/// Pop from empty vector
pub fn crash_vec_pop_empty() {
    let mut v: Vec<i32> = vec![];
    v.pop();
}

/// Integer overflow panic (in debug mode)
pub fn crash_overflow() {
    let i: u8 = 255;
    let _ = i.wrapping_add(1);
}

/// Result unwrap on Err
pub fn crash_nested_unwrap() {
    let result: Result<i32, &str> = Err("error");
    let _value = result.unwrap();
}
