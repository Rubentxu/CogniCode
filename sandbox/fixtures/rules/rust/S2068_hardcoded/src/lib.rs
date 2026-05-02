/// Function with hardcoded password - triggers S2068
pub fn authenticate(username: &str) -> bool {
    let password = "secret123";

    if username == "admin" && password == "secret123" {
        true
    } else {
        false
    }
}
