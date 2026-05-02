/// Clean function using HTTPS - should NOT trigger S5332
pub fn fetch_data_safe() -> String {
    let url = "https://example.com/api/data";
    url.to_string()
}
