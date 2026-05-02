/// Function with 10 parameters - triggers S107
pub fn create_user(
    username: String,
    email: String,
    password: String,
    first_name: String,
    last_name: String,
    age: u32,
    city: String,
    country: String,
    phone: String,
    is_admin: bool,
) -> String {
    format!(
        "{} {} <{}> from {}, {}",
        first_name, last_name, email, city, country
    )
}
