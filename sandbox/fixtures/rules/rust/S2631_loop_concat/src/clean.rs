/// Clean function - no SQL concat in loop
pub fn process_ids(ids: Vec<i32>) -> Vec<String> {
    ids.iter().map(|id| format!("id: {}", id)).collect()
}

/// Clean function without SQL
pub fn build_list(items: Vec<String>) -> String {
    let mut result = String::new();
    for item in items {
        result.push_str(&item);
        result.push('\n');
    }
    result
}
