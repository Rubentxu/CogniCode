//! EmbedResolver — parses `!view(view_id, key=value, ...)` markers in narrative markdown
//! and resolves them to live child ViewBlock entries.
//!
//! Grammar: `!view(view_id, key=value, ...)`
//! Example: `!view(call-graph, symbol=foo)`
//!
//! Each marker produces a child ViewBlock with:
//! - `id`: derived from view_id (e.g., `"call-graph"` → `"embed:call-graph:0"`)
//! - `title`: the view_id
//! - `body`: `{"kind": view_id, "params": {key: value, ...}}`
//!
//! Malformed markers produce error blocks with `id = "embed-error:N"`.

use crate::dto::ViewBlock;
use serde_json::json;

/// Resolves `!view(...)` markers in narrative markdown to child ViewBlock entries.
///
/// Returns `(cleaned_markdown, Vec<ViewBlock>)` — markers are stripped from
/// the markdown text and returned as separate child blocks for the caller to embed.
pub struct EmbedResolver;

impl EmbedResolver {
    /// Parse all `!view(...)` markers from `markdown` and return them as
    /// child ViewBlocks. Markers are removed from the returned markdown text
    /// (caller embeds the resolved blocks alongside the cleaned markdown block).
    pub fn resolve(markdown: &str) -> (String, Vec<ViewBlock>) {
        let mut blocks = Vec::new();
        let mut error_count = 0;
        let mut last_end = 0;
        let mut result = String::with_capacity(markdown.len());

        for marker in Self::find_markers(markdown) {
            // Append text before this marker
            result.push_str(&markdown[last_end..marker.start]);
            last_end = marker.end;

            match Self::parse_marker(&markdown[marker.start..marker.end], blocks.len()) {
                Ok(block) => blocks.push(block),
                Err(()) => {
                    // Malformed — push error block
                    blocks.push(ViewBlock {
                        id: format!("embed-error:{}", error_count),
                        title: "Malformed embed".into(),
                        body: json!({
                            "resolution_error": format!(
                                "Failed to parse embed marker at offset {}: '{}'",
                                marker.start,
                                &markdown[marker.start..marker.end.min(marker.start + 50)]
                            ),
                            "embed_index": error_count,
                        }),
                    });
                    error_count += 1;
                }
            }
        }

        // Append remaining text after last marker
        result.push_str(&markdown[last_end..]);

        (result, blocks)
    }

    /// Find all `!view(...)` markers in the markdown.
    fn find_markers(markdown: &str) -> Vec<MarkerSpan> {
        let mut spans = Vec::new();
        let bytes = markdown.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            // Look for `!view(` case-insensitively
            if bytes[i] == b'!'
                && i + 5 < bytes.len()
                && (bytes[i + 1] == b'v' || bytes[i + 1] == b'V')
                && (bytes[i + 2] == b'i' || bytes[i + 2] == b'I')
                && (bytes[i + 3] == b'e' || bytes[i + 3] == b'E')
                && (bytes[i + 4] == b'w' || bytes[i + 4] == b'W')
                && bytes[i + 5] == b'('
            {
                let start = i;
                // Find matching closing paren
                let mut depth = 1;
                let mut j = i + 6;
                while j < bytes.len() && depth > 0 {
                    match bytes[j] {
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        _ => {}
                    }
                    j += 1;
                }
                if depth == 0 {
                    spans.push(MarkerSpan { start, end: j });
                    i = j;
                    continue;
                }
            }
            i += 1;
        }

        spans
    }

    /// Parse a single marker string like `!view(call-graph, symbol=foo)`.
    /// Returns Ok(ViewBlock) or Err(()) for malformed input.
    fn parse_marker(marker: &str, index: usize) -> Result<ViewBlock, ()> {
        // Strip `!` prefix and case-insensitive `view(` prefix
        let rest = marker.strip_prefix('!').ok_or(())?;
        let rest = rest
            .strip_prefix(|c: char| c.eq_ignore_ascii_case(&'v'))
            .ok_or(())?;
        let rest = rest
            .strip_prefix(|c: char| c.eq_ignore_ascii_case(&'i'))
            .ok_or(())?;
        let rest = rest
            .strip_prefix(|c: char| c.eq_ignore_ascii_case(&'e'))
            .ok_or(())?;
        let rest = rest
            .strip_prefix(|c: char| c.eq_ignore_ascii_case(&'w'))
            .ok_or(())?;
        let inner = rest
            .strip_prefix('(')
            .ok_or(())?
            .strip_suffix(')')
            .ok_or(())?;

        let mut parts = inner.split(',');
        let view_id = parts.next().ok_or(())?.trim();
        if view_id.is_empty() {
            return Err(());
        }

        let mut params = serde_json::Map::new();
        for part in parts {
            let part = part.trim();
            if let Some(eq_pos) = part.find('=') {
                let key = part[..eq_pos].trim();
                let value = part[eq_pos + 1..].trim();
                if !key.is_empty() {
                    params.insert(key.to_string(), json!(value));
                }
            }
        }

        Ok(ViewBlock {
            id: format!("embed:{}:{}", view_id, index),
            title: view_id.to_string(),
            body: json!({
                "kind": view_id,
                "params": params,
            }),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct MarkerSpan {
    start: usize,
    end: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_markdown_returns_empty_vec() {
        let (cleaned, blocks) = EmbedResolver::resolve("");
        assert!(blocks.is_empty());
        assert_eq!(cleaned, "");
    }

    #[test]
    fn test_single_valid_marker() {
        let (cleaned, blocks) = EmbedResolver::resolve("!view(call-graph, symbol=foo)");
        assert!(blocks.len() == 1);
        let block = &blocks[0];
        assert_eq!(block.id, "embed:call-graph:0");
        assert_eq!(block.title, "call-graph");
        assert_eq!(block.body["kind"], "call-graph");
        assert_eq!(block.body["params"]["symbol"], "foo");
        assert_eq!(cleaned, "");
    }

    #[test]
    fn test_multiple_markers() {
        let (cleaned, blocks) =
            EmbedResolver::resolve("!view(call-graph, symbol=foo)\n!view(moldql, query=MATCH)");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].id, "embed:call-graph:0");
        assert_eq!(blocks[1].id, "embed:moldql:1");
        assert_eq!(blocks[0].body["params"]["symbol"], "foo");
        assert_eq!(blocks[1].body["params"]["query"], "MATCH");
        assert_eq!(cleaned.trim(), "");
    }

    #[test]
    fn test_malformed_marker_produces_error_block() {
        // Has closing paren but no view_id — parse_marker fails on empty view_id
        let (_cleaned, blocks) = EmbedResolver::resolve("!view(,symbol=foo)");
        assert_eq!(blocks.len(), 1);
        let block = &blocks[0];
        assert_eq!(block.id, "embed-error:0");
        assert!(block.body.get("resolution_error").is_some());
    }

    #[test]
    fn test_plain_text_no_markers() {
        let (cleaned, blocks) = EmbedResolver::resolve("Plain text without any view markers");
        assert!(blocks.is_empty());
        assert_eq!(cleaned, "Plain text without any view markers");
    }

    #[test]
    fn test_marker_with_text_before_and_after() {
        let (cleaned, blocks) =
            EmbedResolver::resolve("Before\n!view(call-graph, symbol=main)\nAfter");
        assert_eq!(blocks.len(), 1);
        assert!(cleaned.contains("Before"));
        assert!(cleaned.contains("After"));
        assert!(!cleaned.contains("!view("));
    }

    #[test]
    fn test_multiple_same_view_id_different_index() {
        let (_cleaned, blocks) =
            EmbedResolver::resolve("!view(call-graph, symbol=foo)\n!view(call-graph, symbol=bar)");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].id, "embed:call-graph:0");
        assert_eq!(blocks[1].id, "embed:call-graph:1");
    }

    #[test]
    fn test_case_insensitive_view_keyword() {
        let (_cleaned, blocks) = EmbedResolver::resolve("!VIEW(call-graph, symbol=foo)");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].id, "embed:call-graph:0");
    }
}
