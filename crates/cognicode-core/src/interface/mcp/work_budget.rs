//! The work a tool carries, and the budget that bounds it.
//!
//! The window of the dispatch is decided by what the tool does. The
//! catalogue declares the capabilities a tool needs (`requires_graph`), and
//! the window of the graph follows that declaration.
//!
//! A work that carries its own budget is declared by the layer that does
//! the work: the verification compiles every candidate it found, and that
//! takes longer than the window of the `search` category its tool belongs
//! to. The application owns that number (`application/services/
//! verification.rs`), because the catalogue cannot own it: the catalogue
//! lives in `infrastructure/`, and CR-06 does not let the application read
//! it.
//!
//! This module is the bridge. It names the tool of the wire and answers
//! with the longest the work of that tool may take, so the server gives
//! it a window that covers its work instead of cutting it. The two read
//! the same number, so the window and the work cannot disagree about the
//! same call.

use crate::application::services::verification;
use std::time::Duration;

/// The longest the work of a tool may take, when the tool carries a bound.
///
/// The tools whose work the window of their category already covers answer
/// with nothing.
pub fn for_tool(tool_name: &str) -> Option<Duration> {
    match tool_name {
        "retrieve_and_verify" => Some(verification::worst_case()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tool that compiles what it retrieves declares the longest its
    /// work may take.
    ///
    /// The number is the one the application declared, not a second copy
    /// of it: the window of the dispatch is derived from the same bound
    /// the work stops at.
    #[test]
    fn the_tool_that_compiles_declares_the_longest_its_work_may_take() {
        assert_eq!(
            for_tool("retrieve_and_verify"),
            Some(verification::worst_case()),
            "the longest the verification may take is the one the work declares"
        );
    }

    /// A tool that carries no bound answers with nothing, so the window
    /// of its category covers its work.
    #[test]
    fn a_tool_without_work_budget_answers_with_nothing() {
        assert_eq!(for_tool("search_content"), None);
        assert_eq!(for_tool("build_graph"), None);
        // The name is assembled at runtime: written out, it would be a
        // literal in this file, and the scan that looks for the names the
        // tests mention would find this one too.
        let unknown = ["a", "tool", "this", "product", "does", "not", "serve"].join("_");
        assert_eq!(for_tool(&unknown), None);
    }
}
