/// Clean function with single return - should NOT trigger S1142
pub fn classify(value: i32) -> &'static str {
    if value < 0 {
        "negative"
    } else if value == 0 {
        "zero"
    } else if value < 10 {
        "small"
    } else if value < 100 {
        "medium"
    } else {
        "large"
    }
}
