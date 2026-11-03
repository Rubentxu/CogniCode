#[derive(Debug, Clone, PartialEq)]
pub struct ContextData {
    pub before: Vec<String>,
    pub current: String,
    pub after: Vec<String>,
}

/// Gets surrounding context lines for a given 0-indexed line
pub(super) fn context_data(source: &str, line: usize, context_size: usize) -> ContextData {
    let lines: Vec<&str> = source.lines().collect();

    let before_start = line.saturating_sub(context_size);
    let after_end = (line + 1 + context_size).min(lines.len());

    let before: Vec<String> = (before_start..line)
        .map(|i| lines.get(i).unwrap_or(&"").to_string())
        .collect();

    let current = lines.get(line).unwrap_or(&"").to_string();

    let after: Vec<String> = ((line + 1)..after_end)
        .map(|i| lines.get(i).unwrap_or(&"").to_string())
        .collect();

    ContextData {
        before,
        current,
        after,
    }
}
