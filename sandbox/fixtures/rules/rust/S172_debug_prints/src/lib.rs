/// Library function with debug print - triggers S172
pub fn calculate(x: i32) -> i32 {
    println!("Debug: calculating for x = {}", x);
    x * 2
}
