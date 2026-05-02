/// Function using unwrap in non-test code - triggers S5631
pub fn get_value(option: Option<i32>) -> i32 {
    option.unwrap()
}
