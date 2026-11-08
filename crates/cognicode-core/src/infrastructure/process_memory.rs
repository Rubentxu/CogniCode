//! Process-local memory observations used by analysis instrumentation.
//!
//! The reader is deliberately best-effort: platforms that do not expose
//! `/proc/self/status`, or sandboxes that deny it, return `None` rather than
//! publishing zero as if memory had been measured.

/// Read the process high-water resident set size in bytes.
///
/// Linux exposes this as `VmHWM` in `/proc/self/status`. The operation is
/// observational only and returns `None` when the platform or sandbox does
/// not provide the field.
pub fn read_peak_memory_bytes() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/status").ok()?;
    parse_memory_field(&text, "VmHWM:")
}

fn parse_memory_field(text: &str, field: &str) -> Option<u64> {
    let rest = text.lines().find_map(|line| line.strip_prefix(field))?;
    let mut iter = rest.split_whitespace();
    let value: u64 = iter.next()?.parse().ok()?;
    let unit = iter.next().unwrap_or("kB");
    match unit {
        "kB" | "KiB" => value.checked_mul(1024),
        "MB" | "MiB" => value.checked_mul(1024 * 1024),
        "B" => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_memory_field;

    #[test]
    fn parses_linux_kibibytes() {
        assert_eq!(
            parse_memory_field("VmHWM:\t 12 kB\n", "VmHWM:"),
            Some(12 * 1024)
        );
    }

    #[test]
    fn parses_explicit_units() {
        assert_eq!(
            parse_memory_field("Peak: 2 MiB\n", "Peak:"),
            Some(2 * 1024 * 1024)
        );
        assert_eq!(parse_memory_field("Peak: 7 B\n", "Peak:"), Some(7));
    }

    #[test]
    fn rejects_missing_or_invalid_fields() {
        assert_eq!(parse_memory_field("VmRSS: 3 kB\n", "VmHWM:"), None);
        assert_eq!(parse_memory_field("VmHWM: nope kB\n", "VmHWM:"), None);
        assert_eq!(parse_memory_field("VmHWM: 3 bananas\n", "VmHWM:"), None);
    }
}
