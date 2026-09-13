pub fn helper(value: u32) -> u32 {
    value + 1
}

pub fn main() -> u32 {
    let doubled = helper(2);
    doubled * 2
}
