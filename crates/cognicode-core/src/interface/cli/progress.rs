//! The two channels of the CLI, and the rule that decides between them.
//!
//! The standard output carries the PRODUCT: the JSON document, or the
//! text of the answer. The standard error carries the OBSERVABILITY:
//! progress lines and warnings.
//!
//! The rule this module holds:
//!
//!   - In text mode the progress line is part of the published text
//!     format, and the published format puts it in the standard output.
//!   - In JSON mode the standard output is the document and nothing
//!     else —a consumer that reads the whole channel has to be able to
//!     parse it— so the progress line goes to the standard error.
//!
//! The user reads the same line in both modes: the standard error is on
//! the terminal too. What changes is the channel, and the channel is
//! what a consumer parses.

use super::commands::Format;

/// Writes a progress line to the channel that belongs to it.
///
/// The text format publishes the progress line in the standard output,
/// and the snapshot tests pin it there. The JSON document is the whole
/// standard output, so there the progress line goes to the standard
/// error: the document stays a document, and the user still sees the
/// line.
pub fn progress_line(format: Format, message: &str) {
    if format == Format::Json {
        eprintln!("{message}");
    } else {
        println!("{message}");
    }
}
