//! Shared helper functions for performance rules.
//!
//! Provides common utilities for brace counting, loop detection, etc.

use regex::Regex;

/// Count brace balance from a position in source
pub fn count_brace_balance(source: &str, start: usize) -> isize {
    let mut count = 0;
    for (i, c) in source[start..].chars().enumerate() {
        match c {
            '{' => count += 1,
            '}' => count -= 1,
            '\n' => continue,
            _ => {}
        }
        if count < 0 {
            return -1; // Malformed
        }
        if i > 100_000 {
            break; // Safety limit
        }
    }
    count
}

/// Find the position of the closing brace matching the opening at start
pub fn find_brace_close(source: &str, start: usize, initial_balance: isize) -> Option<usize> {
    let mut balance = initial_balance;
    for (i, c) in source[start..].chars().enumerate() {
        match c {
            '{' => balance += 1,
            '}' => {
                balance -= 1;
                if balance == 0 {
                    return Some(start + i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract loop body from source given a loop match start position
pub fn extract_loop_body<'a>(source: &'a str, loop_start: usize) -> Option<(usize, &'a str)> {
    let brace_count = count_brace_balance(source, loop_start);
    if let Some(body_end) = find_brace_close(source, loop_start, brace_count) {
        let loop_body = &source[loop_start..body_end.min(source.len())];
        Some((body_end, loop_body))
    } else {
        None
    }
}

/// Check if source contains any of the patterns
pub fn contains_any(source: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| source.contains(p))
}

/// Skip test files
pub fn is_test_file(source: &str) -> bool {
    source.contains("#[test]") || source.contains("#[cfg(test)]")
}

/// Compile a regex pattern (panics if invalid - for development only)
pub fn compile_re(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap()
}
