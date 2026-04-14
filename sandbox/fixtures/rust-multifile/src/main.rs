//! Main file for multifile fixture testing.
//!
//! This file contains functions that call each other - useful for per-file graph analysis.
//! Ground truth for get_per_file_graph on this file:
//!   - main → compute (line 12)
//!   - compute → format_message (line 17)

/// Main entry point.
pub fn main() {
    let result = compute(42);
    let msg = format_message("Result", result);
    println!("{}", msg);
}

/// Compute function - calls format_message internally.
pub fn compute(x: i32) -> i32 {
    let formatted = format_message("Computing", x);
    println!("{}", formatted);
    x * 2
}

/// Format a message with the value.
pub fn format_message(label: &str, value: i32) -> String {
    format!("{}: {}", label, value)
}
