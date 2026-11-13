//! Matching a pattern against the text of one file.
//!
//! The content search reads the files and hands each one's text here. Nothing
//! in this module opens a file or walks a tree, so what it answers depends on
//! the text and the pattern alone.

use regex::Regex;

/// Finds matches in content using literal or regex search
pub(crate) fn find_matches_in_content(
    content: &str,
    pattern: &str,
    is_regex: bool,
    case_insensitive: bool,
    context_lines: usize,
) -> Vec<(u32, u32, String, Vec<String>)> {
    // Vec of (line_number, col, matched_text, context)
    let mut results = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    if is_regex {
        // Use the real regex crate
        let pattern_with_flags = if case_insensitive {
            format!("(?i){}", pattern)
        } else {
            pattern.to_string()
        };

        let re = match Regex::new(&pattern_with_flags) {
            Ok(re) => re,
            Err(_) => return results, // Invalid regex, return empty
        };

        for (idx, line_content) in lines.iter().enumerate() {
            for m in re.find_iter(line_content) {
                let context_before: Vec<String> = lines[..idx]
                    .iter()
                    .rev()
                    .take(context_lines)
                    .map(|s| s.to_string())
                    .collect();
                let context_after: Vec<String> = lines[idx + 1..]
                    .iter()
                    .take(context_lines)
                    .map(|s| s.to_string())
                    .collect();

                results.push((
                    (idx + 1) as u32,
                    (m.start() + 1) as u32, // 1-indexed column
                    m.as_str().to_string(),
                    context_before
                        .into_iter()
                        .rev()
                        .chain(context_after)
                        .collect(),
                ));
            }
        }
    } else {
        // Literal search
        for (idx, line_content) in lines.iter().enumerate() {
            let search_text = if case_insensitive {
                line_content.to_lowercase()
            } else {
                line_content.to_string()
            };
            let search_pattern = if case_insensitive {
                pattern.to_lowercase()
            } else {
                pattern.to_string()
            };

            let mut start = 0;
            while let Some(pos) = search_text[start..].find(&search_pattern) {
                let abs_pos = start + pos;
                let matched_text = &line_content[abs_pos..abs_pos + search_pattern.len()];

                let context_before: Vec<String> = lines[..idx]
                    .iter()
                    .rev()
                    .take(context_lines)
                    .map(|s| s.to_string())
                    .collect();
                let context_after: Vec<String> = lines[idx + 1..]
                    .iter()
                    .take(context_lines)
                    .map(|s| s.to_string())
                    .collect();

                results.push((
                    (idx + 1) as u32,
                    (abs_pos + 1) as u32, // 1-indexed column
                    matched_text.to_string(),
                    context_before
                        .into_iter()
                        .rev()
                        .chain(context_after)
                        .collect(),
                ));
                start += pos + 1;
            }
        }
    }

    results
}
