/// Function with SQL injection vulnerability - triggers S5122
pub fn find_user(user_input: &str) -> String {
    let query = format!("SELECT * FROM users WHERE name = '{}'", user_input);

    // In real code, this would execute the query
    // For fixture purposes, we just return the query string
    query
}
