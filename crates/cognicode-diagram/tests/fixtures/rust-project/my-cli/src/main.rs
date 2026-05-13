//! My CLI application

use crate_lib_a::greet;
use crate_lib_b::process_with_greeting;

fn main() {
    println!("{}", greet("World"));
    println!("{}", process_with_greeting("CogniCode"));
}
