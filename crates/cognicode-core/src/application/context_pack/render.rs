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

/// The text of one item: where it is and why, then what of it is shown.
///
/// A signature is its one-line header. A slice or a full body is quoted
/// line by line under its original line numbers, with the lines left out
/// named; the header is then the first quoted line.
pub fn item_text(item: &PackItem) -> String {
    let mut text = format!(
        "{}\n  {}:{}-{}  {}  reason={}",
        item.id, item.file, item.range.start_line, item.range.end_line, item.kind, item.reason
    );
    if let Some(detail) = &item.reason_detail {
        text.push_str(&format!(" ({detail})"));
    }
    if let Some(selection) = &item.selection {
        text.push_str(&format!(" role={}", selection.role.code()));
    }
    text.push('\n');
    match &item.representation {
        Some(shown) if !shown.segments.is_empty() => {
            text.push_str(&format!("  [{}", shown.level.code()));
            if let Some(by) = &shown.selected_by {
                text.push_str(&format!(": {by}"));
            }
            text.push_str("]\n");
            for segment in &shown.segments {
                for (offset, line) in segment.text.lines().enumerate() {
                    text.push_str(&format!(
                        "  {:>4}| {line}\n",
                        segment.start_line + offset as u32
                    ));
                }
                if let Some(gap) = shown
                    .elided
                    .iter()
                    .find(|range| range.start_line == segment.end_line + 1)
                {
                    text.push_str(&format!(
                        "       … lines {}-{} elided\n",
                        gap.start_line, gap.end_line
                    ));
                }
            }
        }
        Some(shown) if shown.degraded_from.is_some() => {
            let from = shown.degraded_from.map_or("", |level| level.code());
            text.push_str(&format!(
                "  {}\n  [signature: degraded from {from}]\n",
                item.signature
            ));
        }
        _ => text.push_str(&format!("  {}\n", item.signature)),
    }
    text
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
        text.push_str(&item_text(item));
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
