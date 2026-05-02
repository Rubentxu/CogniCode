/// Clean function - should NOT trigger S1135
pub fn calculate(x: i32) -> i32 {
    if x < 0 {
        return 0;
    }
    x * 2
}
