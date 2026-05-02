/// Function that needs fixing - triggers S1135
pub fn calculate(x: i32) -> i32 {
    // TODO: fix this - the algorithm is wrong for negative numbers
    if x < 0 {
        return 0;
    }
    x * 2
}
