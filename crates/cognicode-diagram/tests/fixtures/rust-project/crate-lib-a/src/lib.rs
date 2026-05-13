//! Library A

/// Greets the given name
pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

/// Internal helper function
fn internal_helper() -> i32 {
    42
}

pub mod utils {
    /// Utility function
    pub fn utility() -> bool {
        true
    }
}
