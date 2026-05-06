/// Clean function using parameterized queries - should NOT trigger S2076
pub fn find_user_safe(user_id: i32) -> String {
    // Safe query - no SQL keywords in format arguments
    let base = "SELECT * FROM users";
    format!("Query built for user id: {}", user_id)
}

/// Clean insert using only values, no SQL keywords in interpolation
pub fn log_operation(operation: &str) -> String {
    format!("Operation: {}", operation)
}

/// Clean update - no SQL in format string
pub fn build_query(table: &str) -> String {
    format!("Query for table: {}", table)
}
