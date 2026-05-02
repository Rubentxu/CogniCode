/// Clean library function without debug prints - should NOT trigger S172
pub fn calculate(x: i32) -> i32 {
    // Using tracing instead of println for library code
    x * 2
}
