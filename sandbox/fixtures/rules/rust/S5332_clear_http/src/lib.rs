/// Function using cleartext HTTP URL - triggers S5332
pub fn fetch_data() -> String {
    let url = "http://example.com/api/data";
    // In real code, this would make an HTTP request
    url.to_string()
}
