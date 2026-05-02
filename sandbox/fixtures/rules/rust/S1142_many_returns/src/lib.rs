/// Function with 5 return statements - triggers S1142
pub fn classify(value: i32) -> &'static str {
    if value < 0 {
        return "negative";
    }
    if value == 0 {
        return "zero";
    }
    if value < 10 {
        return "small";
    }
    if value < 100 {
        return "medium";
    }
    "large"
}
