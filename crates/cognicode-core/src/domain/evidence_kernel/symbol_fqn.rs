//! SymbolFqn — the typed symbol identity string (E38.1 CP-1).
//!
//! The canonical identity grammar is byte-identical across every producer
//! and consumer: `"{file}:{name}:{line}"`. What DIFFERS between producers is
//! the LINE BASE of the trailing segment:
//!
//! - **Fact side** (1-based): `application/ingest/extractor.rs` emits
//!   `start.row + 1`, and the fact bridge uses the extractor's id string
//!   verbatim (e37 design D3 grammar).
//! - **Legacy side** (0-based): `domain/aggregates/symbol.rs` renders
//!   `Location::line()`, which stores the tree-sitter 0-based
//!   `start.row` (`tree_sitter_parser::node_to_symbol_with_path`).
//!
//! The constructors make the line base explicit at every PRODUCTION site;
//! the stored line is rendered VERBATIM so each side keeps emitting exactly
//! the same bytes as before the migration (grammar byte-identical rule).
//! [`SymbolFqn::parse`] preserves the line as encoded — the base of a
//! parsed string is producer-defined and round-trips byte-identically
//! through [`SymbolFqn::assemble`].
//!
//! This module is deliberately NOT gated behind the `evidence-kernel`
//! feature: the identity grammar is shared by the always-compiled legacy
//! path (`Symbol`) and the fact path, and gating it would force the grammar
//! to be duplicated instead of centralized.

/// A parsed or constructed symbol identity `"{file}:{name}:{line}"`.
///
/// The trailing [`line`](SymbolFqn::line) is stored as ENCODED — its base
/// (fact-side 1-based vs legacy-side 0-based) is declared by the constructor
/// used at the production site, never converted here.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolFqn {
    file: String,
    name: String,
    line: u32,
}

impl SymbolFqn {
    /// Fact-side constructor (e37 design D3 grammar): `one_based_line` is
    /// the 1-based line the extractor emits (`start.row + 1`). The line is
    /// stored verbatim and rendered verbatim by [`assemble`](Self::assemble),
    /// keeping the fact grammar byte-identical.
    pub fn from_fact_side(
        file: impl Into<String>,
        name: impl Into<String>,
        one_based_line: u32,
    ) -> Self {
        Self {
            file: file.into(),
            name: name.into(),
            line: one_based_line,
        }
    }

    /// Legacy-side constructor: `zero_based_line` is the 0-based line the
    /// legacy aggregate renders (`Location::line()`, the tree-sitter
    /// `start.row`). The line is stored verbatim and rendered verbatim by
    /// [`assemble`](Self::assemble), keeping the legacy FQN bytes unchanged.
    pub fn from_legacy_side(
        file: impl Into<String>,
        name: impl Into<String>,
        zero_based_line: u32,
    ) -> Self {
        Self {
            file: file.into(),
            name: name.into(),
            line: zero_based_line,
        }
    }

    /// Parses an identity string of the grammar `"{file}:{name}:{line}"`.
    ///
    /// The split runs from the RIGHT so a name (or file path) containing
    /// `:` still reconstructs byte-identically: the string is cut at exactly
    /// the two rightmost colons. Returns `None` when the trailing segment is
    /// not a line number or fewer than two separators exist. The parsed line
    /// is kept as encoded (base is producer-defined).
    pub fn parse(fqn: &str) -> Option<Self> {
        let (rest, line) = fqn.rsplit_once(':')?;
        let (file, name) = rest.rsplit_once(':')?;
        let line = line.parse::<u32>().ok()?;
        Some(Self {
            file: file.to_string(),
            name: name.to_string(),
            line,
        })
    }

    /// Renders the identity string `"{file}:{name}:{line}"` — the canonical
    /// grammar, byte-identical to the historical `format!` at every site.
    pub fn assemble(&self) -> String {
        format!("{}:{}:{}", self.file, self.name, self.line)
    }

    /// The file segment (everything before the second-to-last `:`).
    pub fn file(&self) -> &str {
        &self.file
    }

    /// The name segment (between the last two `:` separators).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The line segment AS ENCODED — 1-based on fact-side strings,
    /// 0-based on legacy-side strings (the base is producer-defined).
    pub fn line(&self) -> u32 {
        self.line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Task 1.1 — base semantics: each constructor renders its line
    /// verbatim, so the fact side keeps emitting 1-based lines and the
    /// legacy side keeps emitting 0-based lines (grammar byte-identical).
    #[test]
    fn constructors_render_line_verbatim_per_base() {
        let fact = SymbolFqn::from_fact_side("src/lib.rs", "greet", 1).assemble();
        assert_eq!(fact, "src/lib.rs:greet:1", "fact side is 1-based");

        let legacy = SymbolFqn::from_legacy_side("src/lib.rs", "greet", 0).assemble();
        assert_eq!(legacy, "src/lib.rs:greet:0", "legacy side is 0-based");
    }

    /// Task 1.1 — parse → assemble round-trips byte-identically, including
    /// identity strings whose file/name segments contain colons.
    #[test]
    fn parse_assemble_round_trip_is_byte_identical() {
        let cases = [
            "src/lib.rs:greet:1",
            "a:b:c:deep:file.rs:name:42",
            "test.rs:A:0",
            "py/mod.py:helper_1_2:999",
        ];
        for case in cases {
            let parsed = SymbolFqn::parse(case).expect("parsable identity");
            assert_eq!(parsed.assemble(), case, "round trip for {case}");
        }
    }

    /// The right-anchored split keeps colons inside the file path.
    #[test]
    fn parse_splits_from_the_right() {
        let parsed = SymbolFqn::parse("a:b:file.rs:greet:7").expect("parsable");
        assert_eq!(parsed.file(), "a:b:file.rs");
        assert_eq!(parsed.name(), "greet");
        assert_eq!(parsed.line(), 7);
    }

    /// Task 1.1 — malformed identity strings are rejected, not degraded.
    #[test]
    fn parse_rejects_malformed_identity_strings() {
        for case in ["", "greet", "src/lib.rs:greet", "src/lib.rs:greet:x"] {
            assert!(SymbolFqn::parse(case).is_none(), "{case} must not parse");
        }
    }
}
