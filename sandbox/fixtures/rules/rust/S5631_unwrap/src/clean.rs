/// Clean function using proper error handling - should NOT trigger S5631
pub fn get_value_safe(option: Option<i32>) -> i32 {
    option.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_with_unwrap() {
        // unwrap is acceptable in tests
        assert_eq!(get_value(Some(42)), 42);
    }
}
