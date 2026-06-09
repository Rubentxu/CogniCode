//! Cursor — single-pass byte cursor for the MoldQL parser.
//!
//! Extracted from `moldql/parser.rs` so the new ExplorerQL parsers can
//! share the same `peek/consume/skip_ws` primitives. Pure zero-behavior
//! refactor — every test that previously lived on the inline `Cursor`
//! in `parser.rs` is mirrored here.
//!
//! ## Invariants
//!
//! - `index` is a byte offset into `input`.
//! - `(line, column)` is 1-based, recomputed on demand.
//! - `peek_*` never advances. `advance*` and `consume_keyword` advance.
//! - `consume_keyword` panics if the keyword does NOT match the
//!   current token — callers must `peek_keyword` first. (Same
//!   semantics as the inline version in `parser.rs`.)

/// Single-pass byte cursor. Tracks `index` into `input` and recomputes
/// 1-based `line` / `column` on demand.
pub(crate) struct Cursor<'a> {
    pub(crate) input: &'a str,
    pub(crate) index: usize,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(input: &'a str) -> Self {
        Self { input, index: 0 }
    }

    pub(crate) fn is_eof(&self) -> bool {
        self.index >= self.input.len()
    }

    pub(crate) fn peek_char(&self) -> Option<char> {
        self.input[self.index..].chars().next()
    }

    pub(crate) fn peek_char_at(&self, offset: usize) -> Option<char> {
        self.input[self.index..].chars().nth(offset)
    }

    pub(crate) fn advance(&mut self) {
        if let Some(c) = self.peek_char() {
            self.index += c.len_utf8();
        }
    }

    pub(crate) fn advance_by(&mut self, chars: usize) {
        for _ in 0..chars {
            self.advance();
        }
    }

    pub(crate) fn skip_ws(&mut self) {
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    /// Read a keyword (whitespace-delimited, stops at operator chars,
    /// dots, and parens) and rewind — only the peek is non-destructive.
    /// The caller uses [`Self::consume_keyword`] to actually advance.
    pub(crate) fn peek_keyword(&self) -> Option<String> {
        let mut temp = Cursor {
            input: self.input,
            index: self.index,
        };
        temp.skip_ws();
        let start = temp.index;
        while let Some(c) = temp.peek_char() {
            if c.is_whitespace() || matches!(c, '>' | '<' | '=' | '!' | '~' | '"' | '.' | '(' | ')')
            {
                break;
            }
            temp.advance();
        }
        if temp.index == start {
            return None;
        }
        Some(temp.input[start..temp.index].to_string())
    }

    /// Advance past `keyword` (case-insensitive) and the trailing
    /// whitespace. Panics if the keyword does not match the current
    /// token — callers must check with `peek_keyword` first.
    pub(crate) fn consume_keyword(&mut self, keyword: &str) {
        self.skip_ws();
        let start = self.index;
        while let Some(c) = self.peek_char() {
            if c.is_whitespace() || matches!(c, '>' | '<' | '=' | '!' | '~' | '"' | '.' | '(' | ')')
            {
                break;
            }
            self.advance();
        }
        let raw = &self.input[start..self.index];
        debug_assert!(
            raw.eq_ignore_ascii_case(keyword),
            "consume_keyword({keyword}) on `{raw}`"
        );
        self.skip_ws();
    }

    /// (line, column) for the current index. 1-based.
    pub(crate) fn position(&self) -> (u32, u32) {
        let mut line: u32 = 1;
        let mut col: u32 = 1;
        for (i, c) in self.input.char_indices() {
            if i >= self.index {
                break;
            }
            if c == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    /// Remaining input for diagnostic messages. Clipped to 32 chars.
    pub(crate) fn remaining(&self) -> String {
        let s: String = self.input[self.index..].chars().take(32).collect();
        s
    }
}

// ============================================================================
// Tests — the public surface of Cursor.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> Cursor<'_> {
        Cursor::new(s)
    }

    #[test]
    fn new_starts_at_zero() {
        let cur = c("hello");
        assert_eq!(cur.index, 0);
        assert!(!cur.is_eof());
    }

    #[test]
    fn empty_input_is_eof() {
        let cur = c("");
        assert!(cur.is_eof());
        assert_eq!(cur.peek_char(), None);
    }

    #[test]
    fn peek_char_does_not_advance() {
        let mut cur = c("ab");
        assert_eq!(cur.peek_char(), Some('a'));
        assert_eq!(cur.peek_char(), Some('a'));
        assert_eq!(cur.index, 0);
        cur.advance();
        assert_eq!(cur.index, 1);
        assert_eq!(cur.peek_char(), Some('b'));
    }

    #[test]
    fn peek_char_at_offset() {
        let cur = c("abcd");
        assert_eq!(cur.peek_char_at(0), Some('a'));
        assert_eq!(cur.peek_char_at(2), Some('c'));
        assert_eq!(cur.peek_char_at(10), None);
    }

    #[test]
    fn advance_walks_one_char_at_a_time() {
        let mut cur = c("abc");
        cur.advance();
        assert_eq!(cur.index, 1);
        cur.advance();
        assert_eq!(cur.index, 2);
    }

    #[test]
    fn advance_at_eof_is_noop() {
        let mut cur = c("");
        cur.advance();
        assert_eq!(cur.index, 0);
    }

    #[test]
    fn advance_by_walks_n_chars() {
        let mut cur = c("abcdef");
        cur.advance_by(3);
        assert_eq!(cur.index, 3);
        assert_eq!(cur.peek_char(), Some('d'));
    }

    #[test]
    fn skip_ws_passes_whitespace() {
        let mut cur = c("   foo");
        cur.skip_ws();
        assert_eq!(cur.peek_char(), Some('f'));
        assert_eq!(cur.index, 3);
    }

    #[test]
    fn skip_ws_handles_tabs_and_newlines() {
        let mut cur = c("\t\n  \r x");
        cur.skip_ws();
        assert_eq!(cur.peek_char(), Some('x'));
    }

    #[test]
    fn skip_ws_at_eof_is_safe() {
        let mut cur = c("   ");
        cur.skip_ws();
        assert!(cur.is_eof());
    }

    #[test]
    fn peek_keyword_returns_word() {
        let cur = c("FIND symbols");
        assert_eq!(cur.peek_keyword().as_deref(), Some("FIND"));
    }

    #[test]
    fn peek_keyword_returns_none_for_empty() {
        let cur = c("   ");
        assert_eq!(cur.peek_keyword(), None);
    }

    #[test]
    fn peek_keyword_stops_at_operators() {
        let cur = c("foo > 1");
        assert_eq!(cur.peek_keyword().as_deref(), Some("foo"));
    }

    #[test]
    fn peek_keyword_stops_at_dot() {
        let cur = c("provenance.lsp");
        assert_eq!(cur.peek_keyword().as_deref(), Some("provenance"));
    }

    #[test]
    fn peek_keyword_case_insensitive_via_caller() {
        // The cursor returns the raw bytes; case-handling is up to the
        // caller. This test pins the behavior.
        let cur = c("find");
        assert_eq!(cur.peek_keyword().as_deref(), Some("find"));
    }

    #[test]
    fn consume_keyword_advances_past_word() {
        let mut cur = c("FIND  symbols");
        cur.consume_keyword("FIND");
        assert_eq!(cur.peek_char(), Some('s'));
    }

    #[test]
    fn consume_keyword_panics_on_mismatch() {
        let mut cur = c("EXPLORE");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cur.consume_keyword("FIND");
        }));
        assert!(result.is_err());
    }

    #[test]
    fn position_starts_at_one_one() {
        let cur = c("abc");
        assert_eq!(cur.position(), (1, 1));
    }

    #[test]
    fn position_after_newline() {
        let mut cur = c("ab\ncd");
        cur.advance_by(3); // past the newline
        assert_eq!(cur.position(), (2, 1));
    }

    #[test]
    fn position_mid_line() {
        let mut cur = c("abcdef");
        cur.advance_by(3);
        assert_eq!(cur.position(), (1, 4));
    }

    #[test]
    fn remaining_returns_tail() {
        let mut cur = c("FIND symbols WHERE x = 1");
        cur.advance_by(5); // past FIND
        let r = cur.remaining();
        assert!(r.starts_with("symbols"));
    }

    #[test]
    fn remaining_clips_to_32_chars() {
        let s = "a".repeat(100);
        let cur = c(&s);
        let r = cur.remaining();
        assert_eq!(r.len(), 32);
    }
}
