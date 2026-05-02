/// Clean function without redundant else - should NOT trigger S1163
pub fn classify(value: i32) -> &'static str {
    if value < 0 {
        "negative"
    } else {
        "non-negative"
    }
}
