/// Vulnerable function using format! with SQL - triggers S2076
pub fn find_user(user_id: i32) -> String {
    // SQL injection vulnerability via format! string interpolation
    format!("SELECT * FROM users WHERE id = {}", user_id)
}

/// Vulnerable insert using format!
pub fn create_user(name: &str, email: &str) -> String {
    format!("INSERT INTO users (name, email) VALUES ('{}', '{}')", name, email)
}

/// Vulnerable update using format!
pub fn update_user_email(user_id: i32, new_email: &str) -> String {
    format!("UPDATE users SET email = '{}' WHERE id = {}", new_email, user_id)
}

/// Vulnerable delete using format!
pub fn delete_user(user_id: i32) -> String {
    format!("DELETE FROM users WHERE id = {}", user_id)
}
