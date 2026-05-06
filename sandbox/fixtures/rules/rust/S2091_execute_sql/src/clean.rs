/// Clean function - no EXECUTE statements
pub fn safe_query(query: String) -> String {
    // No EXECUTE, safe format usage
    format!("Query: {}", query)
}

/// Clean function without SQL execution keywords
pub fn log_message(msg: String) -> String {
    format!("Message: {}", msg)
}

/// Another clean function
pub fn build_command(cmd: String) -> String {
    format!("Command: {}", cmd)
}
