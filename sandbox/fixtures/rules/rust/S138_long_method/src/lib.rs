pub struct Config {
    pub threshold: Option<i32>,
}

/// Long function with ~60 lines that should trigger S138
pub fn process_data(data: Vec<i32>, config: &Config) -> Vec<i32> {
    let mut result = Vec::new();

    // Line 1-5: validation
    if data.is_empty() {
        return result;
    }
    let threshold = config.threshold.unwrap_or(100);

    // Line 6-10: early filtering
    if data.len() > 1000 {
        return result;
    }

    // Line 11-20: filtering and mapping
    let filtered: Vec<_> = data
        .iter()
        .filter(|x| **x > threshold)
        .filter(|x| **x < 10000)
        .filter(|x| **x % 2 == 0)
        .collect();

    // Line 21-30: sorting
    let mut sorted = filtered.clone();
    sorted.sort();
    sorted.reverse();
    sorted.dedup();

    // Line 31-40: grouping
    let mut groups = Vec::new();
    for chunk in sorted.chunks(10) {
        let mut group = chunk.to_vec();
        group.sort();
        groups.push(group);
    }

    // Line 41-50: processing
    for group in &groups {
        let sum: i32 = group.iter().sum();
        let count = group.len() as i32;
        if count > 0 {
            let avg = sum / count;
            result.push(avg);
        }
    }

    // Line 51-55: final cleanup
    result.dedup();
    result.retain(|x| *x > 0);

    result
}
