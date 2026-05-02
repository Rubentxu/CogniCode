use md5::{Md5, Digest};

/// Function using MD5 for hashing - triggers S4792
pub fn hash_password(password: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}
