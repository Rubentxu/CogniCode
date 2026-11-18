//! How much of a symbol a pack shows: signature, slice or full body.
//!
//! The three representations are cut from the same bytes the pack verified
//! and from the same declaration the signature comes from, so they cannot
//! disagree about where a symbol starts and ends. Nothing is rewritten: a
//! segment is a run of whole source lines, quoted verbatim and addressed by
//! its original line numbers, and what is left out is named as an elided
//! range. A slice always keeps the declaration header, so it never shows a
//! body without the signature it belongs to and never invents one.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::policy::Detail;
use super::ports::DeclarationSpan;
use super::rerank::name_terms;
use super::service::ItemRange;

/// The least lines a slice keeps of the body when no statement matches the
/// focus: the opening of the body, so the slice is never only a header.
const LEADING_LINES: u32 = 3;

/// The shortest word of the focus that counts: shorter words match noise.
const MIN_FOCUS_WORD: usize = 3;

/// A run of whole source lines, quoted verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Segment {
    /// The first line, counted from one.
    pub start_line: u32,
    /// The last line, both ends included.
    pub end_line: u32,
    /// The lines exactly as the file has them, joined by newlines.
    pub text: String,
}

/// What of a symbol the pack shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Representation {
    /// `signature`, `slice` or `full`.
    pub level: Detail,
    /// The level the policy planned when the one shown is a smaller one,
    /// because the item did not fit the budget or the language does not
    /// expose the statements of its body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_from: Option<Detail>,
    /// How the statements of a slice were chosen: `focus_terms` when they
    /// mention the request, `leading_statements` when none does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_by: Option<String>,
    /// The lines shown, in source order. Empty for a signature: the header
    /// is the item's `signature`.
    pub segments: Vec<Segment>,
    /// The lines of the declaration that are not shown.
    pub elided: Vec<ItemRange>,
}

impl Representation {
    /// The representation of a signature: the header, nothing else.
    pub fn signature() -> Self {
        Self {
            level: Detail::Signature,
            degraded_from: None,
            selected_by: None,
            segments: Vec::new(),
            elided: Vec::new(),
        }
    }

    /// The same representation, marked as the smaller form of `planned`.
    pub fn degraded_from(mut self, planned: Detail) -> Self {
        if planned != self.level {
            self.degraded_from = Some(planned);
        }
        self
    }
}

/// The lines of `span` as the file has them, clamped to the file.
fn declaration_lines<'a>(source: &'a str, span: &DeclarationSpan) -> (Vec<&'a str>, u32, u32) {
    let lines: Vec<&str> = source.lines().collect();
    let last = lines.len() as u32;
    (lines, span.start_line.max(1), span.end_line.min(last))
}

fn segment(lines: &[&str], start: u32, end: u32) -> Segment {
    Segment {
        start_line: start,
        end_line: end,
        text: lines[(start - 1) as usize..end as usize].join("\n"),
    }
}

/// The last line of the declaration header, counted from one.
fn header_end(span: &DeclarationSpan, end: u32) -> u32 {
    (span.start_line + span.header.trim_end().matches('\n').count() as u32).min(end)
}

/// Whether any word of any identifier of `text` is one of the `focus` words.
fn mentions(text: &str, focus: &HashSet<&str>) -> bool {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|token| !token.is_empty())
        .flat_map(name_terms)
        .any(|word| focus.contains(word.as_str()))
}

/// Merges ranges that overlap or touch, in source order.
fn merged(mut ranges: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    ranges.sort_unstable();
    let mut out: Vec<(u32, u32)> = Vec::new();
    for (start, end) in ranges {
        match out.last_mut() {
            Some(last) if start <= last.1 + 1 => last.1 = last.1.max(end),
            _ => out.push((start, end)),
        }
    }
    out
}

/// The representation of a declaration at `level`.
///
/// `focus` are the words the request is about. A slice keeps the header, the
/// statements of the body that mention them and the closing line; when no
/// statement does, it keeps the opening of the body instead and says so. A
/// declaration whose body statements the language does not expose cannot be
/// sliced: it is shown as a signature and marked as degraded from the slice.
pub fn represent(
    source: &str,
    span: &DeclarationSpan,
    level: Detail,
    focus: &[String],
) -> Representation {
    let (lines, start, end) = declaration_lines(source, span);
    if start > end {
        return Representation::signature().degraded_from(level);
    }
    match level {
        Detail::Signature => Representation::signature(),
        Detail::Full => Representation {
            level,
            degraded_from: None,
            selected_by: None,
            segments: vec![segment(&lines, start, end)],
            elided: Vec::new(),
        },
        Detail::Slice => slice(&lines, span, (start, end), focus),
    }
}

fn slice(
    lines: &[&str],
    span: &DeclarationSpan,
    (start, end): (u32, u32),
    focus: &[String],
) -> Representation {
    if span.body_statements.is_empty() {
        return Representation::signature().degraded_from(Detail::Slice);
    }
    let header = header_end(span, end);
    let words: HashSet<&str> = focus
        .iter()
        .map(String::as_str)
        .filter(|word| word.len() >= MIN_FOCUS_WORD)
        .collect();
    let statements: Vec<(u32, u32)> = span
        .body_statements
        .iter()
        .map(|&(a, b)| (a.max(start), b.min(end)))
        .filter(|&(a, b)| a <= b && b > header)
        .collect();
    let matching: Vec<(u32, u32)> = statements
        .iter()
        .copied()
        .filter(|&(a, b)| mentions(&lines[(a - 1) as usize..b as usize].join("\n"), &words))
        .collect();
    let (kept, selected_by) = if matching.is_empty() {
        let mut taken = 0u32;
        let leading: Vec<(u32, u32)> = statements
            .iter()
            .copied()
            .take_while(|&(a, b)| {
                let keep = taken == 0 || taken < LEADING_LINES;
                taken += b - a + 1;
                keep
            })
            .collect();
        (leading, "leading_statements")
    } else {
        (matching, "focus_terms")
    };

    let mut shown = vec![(start, header)];
    shown.extend(kept);
    if let Some(&(_, last)) = span.body_statements.last()
        && end > last
        && end > header
    {
        shown.push((end, end));
    }
    let shown = merged(shown);
    let mut elided = Vec::new();
    let mut cursor = start;
    for &(a, b) in &shown {
        if a > cursor {
            elided.push(ItemRange {
                start_line: cursor,
                end_line: a - 1,
            });
        }
        cursor = b + 1;
    }
    if cursor <= end {
        elided.push(ItemRange {
            start_line: cursor,
            end_line: end,
        });
    }
    Representation {
        level: Detail::Slice,
        degraded_from: None,
        selected_by: Some(selected_by.to_string()),
        segments: shown
            .into_iter()
            .map(|(a, b)| segment(lines, a, b))
            .collect(),
        elided,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "\
fn load(path: &str) -> Config {
    let raw = read(path);
    let parsed = parse_config(&raw);
    log(\"loaded\");
    audit();
    parsed
}
";

    fn span() -> DeclarationSpan {
        DeclarationSpan {
            start_line: 1,
            end_line: 7,
            header: "fn load(path: &str) -> Config".to_string(),
            body_statements: vec![(2, 2), (3, 3), (4, 4), (5, 5), (6, 6)],
        }
    }

    fn focus(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn a_signature_shows_no_body() {
        let shown = represent(SOURCE, &span(), Detail::Signature, &[]);

        assert_eq!(shown, Representation::signature());
    }

    #[test]
    fn full_quotes_every_line_of_the_declaration_verbatim() {
        let shown = represent(SOURCE, &span(), Detail::Full, &[]);

        assert_eq!(shown.segments.len(), 1);
        assert_eq!(shown.segments[0].start_line, 1);
        assert_eq!(shown.segments[0].end_line, 7);
        assert_eq!(shown.segments[0].text, SOURCE.trim_end());
        assert!(shown.elided.is_empty());
    }

    #[test]
    fn a_slice_keeps_the_header_the_matching_statements_and_the_closer() {
        let shown = represent(SOURCE, &span(), Detail::Slice, &focus(&["parse", "config"]));

        assert_eq!(shown.selected_by.as_deref(), Some("focus_terms"));
        let kept: Vec<(u32, u32)> = shown
            .segments
            .iter()
            .map(|s| (s.start_line, s.end_line))
            .collect();
        assert_eq!(kept, [(1, 1), (3, 3), (7, 7)]);
        assert_eq!(
            shown.elided,
            [
                ItemRange {
                    start_line: 2,
                    end_line: 2
                },
                ItemRange {
                    start_line: 4,
                    end_line: 6
                }
            ]
        );
        assert_eq!(shown.segments[0].text.lines().next(), SOURCE.lines().next());
    }

    #[test]
    fn a_slice_whose_focus_matches_nothing_keeps_the_opening_and_says_so() {
        let shown = represent(SOURCE, &span(), Detail::Slice, &focus(&["unrelated"]));

        assert_eq!(shown.selected_by.as_deref(), Some("leading_statements"));
        let quoted: String = shown.segments.iter().map(|s| s.text.as_str()).collect();
        assert!(quoted.contains("let raw"));
        assert!(!quoted.contains("audit()"));
        assert_eq!(shown.elided.len(), 1);
    }

    #[test]
    fn a_declaration_without_exposed_statements_degrades_to_its_signature() {
        let mut bare = span();
        bare.body_statements.clear();

        let shown = represent(SOURCE, &bare, Detail::Slice, &focus(&["parse"]));

        assert_eq!(shown.level, Detail::Signature);
        assert_eq!(shown.degraded_from, Some(Detail::Slice));
        assert!(shown.segments.is_empty());
    }

    #[test]
    fn adjacent_ranges_merge_into_one_segment() {
        assert_eq!(
            merged(vec![(5, 6), (1, 2), (3, 3), (9, 9)]),
            [(1, 3), (5, 6), (9, 9)]
        );
    }
}
