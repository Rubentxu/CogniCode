//! The text of a pack.
//!
//! A thin adapter: it lays out what the service decided and decides
//! nothing. The service measures this exact text against the budget, so the
//! number the pack publishes is the number of the text it publishes.

use super::service::PackItem;

/// What the text shows.
pub struct RenderView<'a> {
    /// The digest of the snapshot the pack was computed over.
    pub snapshot_digest: &'a str,
    /// The items delivered, in order.
    pub items: &'a [PackItem],
    /// How many candidates are left for a next page.
    pub remaining: usize,
    /// How many candidates were refused because their source changed or
    /// cannot be read.
    pub refused: usize,
    /// The token that continues the pack, when there is a next page.
    pub continuation: Option<&'a str>,
}

/// How many digest characters the header shows.
const DIGEST_PREFIX: usize = 12;

/// The text of a pack.
pub fn render(view: &RenderView) -> String {
    render_with_notes(view, &[])
}

/// The text of a pack with `notes`, one line each, between the header and
/// the items. The notes are part of the text, so they are part of what the
/// budget measures.
pub fn render_with_notes(view: &RenderView, notes: &[String]) -> String {
    let digest: String = view.snapshot_digest.chars().take(DIGEST_PREFIX).collect();
    let mut text = format!("context pack @ {digest}\n");
    for note in notes {
        text.push_str(note);
        text.push('\n');
    }
    for item in view.items {
        text.push_str(&format!(
            "{}\n  {}:{}-{}  {}  reason={}",
            item.id, item.file, item.range.start_line, item.range.end_line, item.kind, item.reason
        ));
        if let Some(detail) = &item.reason_detail {
            text.push_str(&format!(" ({detail})"));
        }
        text.push_str(&format!("\n  {}\n", item.signature));
    }
    if view.remaining > 0 {
        text.push_str(&format!("+{} candidate(s) not shown", view.remaining));
        if let Some(token) = view.continuation {
            text.push_str(&format!("; continue with continuation_token={token}"));
        }
        text.push('\n');
    }
    if view.refused > 0 {
        text.push_str(&format!(
            "{} candidate(s) refused: source differs from the snapshot or is unreadable\n",
            view.refused
        ));
    }
    text
}
