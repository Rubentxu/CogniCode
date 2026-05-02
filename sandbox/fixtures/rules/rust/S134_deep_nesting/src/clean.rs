/// Clean function with minimal nesting - should NOT trigger S134
pub fn process_simple(value: i32) -> i32 {
    if value > 10 {
        value * 2
    } else {
        value + 1
    }
}
