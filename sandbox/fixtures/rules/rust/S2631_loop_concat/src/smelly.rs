/// Vulnerable function with loop concat - triggers S2631
pub fn build_dynamic_query(ids: Vec<i32>) -> String {
    let mut query = String::from("SELECT * FROM users WHERE ");
    // SQL concatenation in loop
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            query.push_str(" OR ");
        }
        query.push_str(&format!("id = {}", id));
    }
    query
}

/// Vulnerable with push_str in loop
pub fn collect_filter(ids: Vec<i32>) -> String {
    let mut sql = String::new();
    sql.push_str("SELECT * FROM items WHERE ");
    for id in ids {
        sql.push_str(&format!("id = {} OR ", id));
    }
    sql
}
