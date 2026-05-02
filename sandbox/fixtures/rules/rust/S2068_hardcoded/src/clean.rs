use std::env;

/// Clean function using environment variable - should NOT trigger S2068
pub fn authenticate_from_env(username: &str) -> bool {
    let password = env::var("PASSWORD").unwrap_or_default();

    if username == "admin" && !password.is_empty() {
        true
    } else {
        false
    }
}
