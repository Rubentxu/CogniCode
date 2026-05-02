use sha2::{Sha256, Digest};

/// Clean function using SHA-256 - should NOT trigger S4792
pub fn hash_password_safe(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}
