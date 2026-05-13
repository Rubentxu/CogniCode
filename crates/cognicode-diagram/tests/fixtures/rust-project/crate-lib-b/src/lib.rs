//! Library B

use crate_lib_a::greet;

/// Process data using library A
pub fn process_with_greeting(name: &str) -> String {
    greet(name)
}

/// Another utility function
pub fn utility_b() -> i32 {
    42
}
