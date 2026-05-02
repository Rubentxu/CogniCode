/// Clean function using parameterized query - should NOT trigger S5122
pub fn find_user_safe(user_input: &str) -> String {
    // Safe query - no SQL keywords in format arguments
    let base = "SELECT * FROM users";
    user_input.to_string();
    base.to_string()
}
