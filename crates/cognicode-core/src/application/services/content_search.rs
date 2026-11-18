//! Matching a pattern against the text of one file.
//!
//! The content search reads the files and hands each one's text here. Nothing
//! in this module opens a file or walks a tree, so what it answers depends on
//! the text and the pattern alone.
//!
//! A pattern is compiled once ([`ContentPattern::new`]) and matched against
//! every file, and a match is reported where it is in the original line: its
//! column and its text are those of the file, including when the comparison
//! ignored case on text that does not lowercase to the same number of bytes.

use regex::Regex;

/// A line, a column (1-based, in bytes), the matched text and the lines
/// around it.
pub(crate) type LineMatch = (u32, u32, String, Vec<String>);

/// A pattern ready to be matched against many files.
pub(crate) struct ContentPattern {
    matcher: Matcher,
}

enum Matcher {
    Regex(Regex),
    Literal {
        /// The text to find, as written.
        needle: String,
        /// The text to find, with every character lowercased on its own; set
        /// only when the comparison ignores case.
        lowered: Option<String>,
    },
}

impl ContentPattern {
    /// Prepares `pattern`, or says that it cannot match anything: a regex
    /// that does not compile, and a literal that is empty (it is not a
    /// request to match every position).
    pub(crate) fn new(pattern: &str, is_regex: bool, case_insensitive: bool) -> Option<Self> {
        let matcher = if is_regex {
            let with_flags = if case_insensitive {
                format!("(?i){pattern}")
            } else {
                pattern.to_string()
            };
            Matcher::Regex(Regex::new(&with_flags).ok()?)
        } else {
            if pattern.is_empty() {
                return None;
            }
            Matcher::Literal {
                needle: pattern.to_string(),
                lowered: case_insensitive.then(|| lowercase_each_char(pattern)),
            }
        };
        Some(Self { matcher })
    }

    /// Every match in `content`, in line order, with `context_lines` lines
    /// before and after each.
    ///
    /// A literal can match again inside an earlier match (`aa` in `aaa`), as
    /// it always could.
    pub(crate) fn find_in(&self, content: &str, context_lines: usize) -> Vec<LineMatch> {
        let lines: Vec<&str> = content.lines().collect();
        let mut results = Vec::new();
        for (idx, line) in lines.iter().enumerate() {
            for (start, end) in self.spans_in_line(line) {
                results.push((
                    (idx + 1) as u32,
                    (start + 1) as u32,
                    line[start..end].to_string(),
                    context_around(&lines, idx, context_lines),
                ));
            }
        }
        results
    }

    /// The byte ranges of the line that match, in the line as it is.
    fn spans_in_line(&self, line: &str) -> Vec<(usize, usize)> {
        match &self.matcher {
            Matcher::Regex(re) => re.find_iter(line).map(|m| (m.start(), m.end())).collect(),
            Matcher::Literal {
                needle,
                lowered: None,
            } => literal_spans(line, needle),
            Matcher::Literal {
                lowered: Some(lowered),
                ..
            } => case_insensitive_spans(line, lowered),
        }
    }
}

/// The lines before and after line `idx`, in file order.
fn context_around(lines: &[&str], idx: usize, context_lines: usize) -> Vec<String> {
    let before_from = idx.saturating_sub(context_lines);
    let after_to = (idx + 1 + context_lines).min(lines.len());
    lines[before_from..idx]
        .iter()
        .chain(&lines[idx + 1..after_to])
        .map(|s| (*s).to_string())
        .collect()
}

/// Lowercases every character on its own, so that two texts that differ only
/// in case lower to the same text wherever they sit in a line.
fn lowercase_each_char(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// The length of the first character of `text`, which is never empty here.
fn first_char_len(text: &str) -> usize {
    text.chars().next().map_or(1, char::len_utf8)
}

/// The places `needle` occurs in `haystack`, including inside each other.
fn find_all(haystack: &str, needle: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(pos) = haystack[from..].find(needle) {
        let at = from + pos;
        found.push(at);
        // The next search starts after the first character of this match,
        // never inside it.
        from = at + first_char_len(&haystack[at..]);
    }
    found
}

fn literal_spans(line: &str, needle: &str) -> Vec<(usize, usize)> {
    find_all(line, needle)
        .into_iter()
        .map(|at| (at, at + needle.len()))
        .collect()
}

fn case_insensitive_spans(line: &str, lowered_needle: &str) -> Vec<(usize, usize)> {
    if line.is_ascii() {
        // Lowering ASCII keeps every offset.
        let lowered_line = line.to_ascii_lowercase();
        return find_all(&lowered_line, lowered_needle)
            .into_iter()
            .map(|at| (at, at + lowered_needle.len()))
            .collect();
    }
    let (lowered_line, starts) = lower_with_origins(line);
    find_all(&lowered_line, lowered_needle)
        .into_iter()
        .filter_map(|at| original_span(line, &lowered_line, &starts, at, lowered_needle.len()))
        .collect()
}

/// The line with every character lowercased on its own, and for each
/// character the offset it starts at in the lowered text and in the line.
fn lower_with_origins(line: &str) -> (String, Vec<(usize, usize)>) {
    let mut lowered = String::with_capacity(line.len());
    let mut starts = Vec::with_capacity(line.len());
    for (offset, c) in line.char_indices() {
        starts.push((lowered.len(), offset));
        lowered.extend(c.to_lowercase());
    }
    (lowered, starts)
}

/// Where the match at `at..at + len` of the lowered line is in the line.
///
/// A match that starts or ends inside what one character lowered to (the
/// dotted capital I lowers to two characters) is not a match of whole
/// characters, and there is no part of the line it corresponds to.
fn original_span(
    line: &str,
    lowered_line: &str,
    starts: &[(usize, usize)],
    at: usize,
    len: usize,
) -> Option<(usize, usize)> {
    let origin_of = |lowered_offset: usize| {
        starts
            .binary_search_by_key(&lowered_offset, |(lowered, _)| *lowered)
            .ok()
            .map(|i| starts[i].1)
    };
    let start = origin_of(at)?;
    let end = if at + len == lowered_line.len() {
        line.len()
    } else {
        origin_of(at + len)?
    };
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(
        pattern: &str,
        is_regex: bool,
        insensitive: bool,
        content: &str,
    ) -> Vec<(u32, u32, String)> {
        ContentPattern::new(pattern, is_regex, insensitive)
            .map(|p| p.find_in(content, 0))
            .unwrap_or_default()
            .into_iter()
            .map(|(line, col, text, _)| (line, col, text))
            .collect()
    }

    #[test]
    fn a_literal_is_found_with_its_column_and_text() {
        assert_eq!(
            found("beta", false, false, "alpha\nxx beta beta\n"),
            vec![(2, 4, "beta".into()), (2, 9, "beta".into())]
        );
    }

    #[test]
    fn a_literal_can_match_inside_an_earlier_match() {
        assert_eq!(found("aa", false, false, "aaa").len(), 2);
    }

    /// A match that starts with a multi-byte character and appears twice in
    /// a line used to resume the search in the middle of that character.
    #[test]
    fn a_literal_that_starts_with_a_multibyte_character_matches_twice_in_a_line() {
        assert_eq!(
            found("é", false, false, "café é"),
            vec![(1, 4, "é".into()), (1, 7, "é".into())]
        );
        assert_eq!(found("éé", false, false, "ééé").len(), 2);
    }

    #[test]
    fn an_empty_literal_matches_nothing() {
        assert!(ContentPattern::new("", false, false).is_none());
        assert!(ContentPattern::new("", false, true).is_none());
    }

    #[test]
    fn an_invalid_regex_matches_nothing() {
        assert!(ContentPattern::new("(", true, false).is_none());
    }

    #[test]
    fn ignoring_case_reports_the_text_of_the_line_not_the_pattern() {
        assert_eq!(
            found("hello", false, true, "say HeLLo!"),
            vec![(1, 5, "HeLLo".into())]
        );
        assert_eq!(
            found("hello", true, true, "say HeLLo!"),
            vec![(1, 5, "HeLLo".into())]
        );
    }

    /// The text of a match must be the text of the line. The dotted capital
    /// I is two bytes in the line and three once lowered, so offsets taken
    /// from the lowered text point into the wrong place of the line.
    #[test]
    fn ignoring_case_keeps_the_offsets_of_the_line_when_lowering_changes_its_length() {
        let line = "İİ needle İ";
        assert_eq!(
            found("needle", false, true, line),
            vec![(1, 6, "needle".into())],
            "the column is where the line has the word"
        );
        assert_eq!(&line[5..11], "needle");
    }

    #[test]
    fn ignoring_case_does_not_match_inside_what_one_character_lowers_to() {
        // `İ` lowers to `i` plus a combining dot: a search for `i` has no
        // whole character of the line to point at.
        assert!(found("i", false, true, "İ").is_empty());
        assert_eq!(
            found("i\u{307}", false, true, "İ"),
            vec![(1, 1, "İ".into())]
        );
    }

    #[test]
    fn ignoring_case_matches_a_capital_sigma_with_a_literal_and_every_sigma_with_a_regex() {
        // A literal lowercases each character on its own, so the final sigma
        // stays what it is; a regex folds all three sigmas together.
        assert_eq!(found("σ", false, true, "ΑΣ ας").len(), 1);
        assert_eq!(found("σ", true, true, "ΑΣ ας").len(), 2);
    }

    #[test]
    fn context_is_the_lines_around_the_match_in_file_order() {
        let pattern = ContentPattern::new("c", false, false).unwrap();
        let hits = pattern.find_in("a\nb\nc\nd\ne\n", 1);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].3, vec!["b".to_string(), "d".to_string()]);
        let hits = pattern.find_in("c\nd\ne\n", 5);
        assert_eq!(hits[0].3, vec!["d".to_string(), "e".to_string()]);
    }
}
