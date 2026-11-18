//! The token measure port: what a budget is counted in, and how sure the
//! count is.
//!
//! A pack is bounded by a number of tokens, and a token is only a token of
//! one tokenizer. The count of a model's tokenizer cannot be known without
//! that tokenizer, so the port makes the certainty part of the answer: every
//! count says whether it is [`MeasureKind::Exact`] for a named tokenizer, an
//! [`MeasureKind::UpperBound`] that no tokenizer of the stated family can
//! exceed, or [`MeasureKind::Unknown`]. A count of the second or third kind
//! is never published as exact, and `len() / 4` is not a count at all.
//!
//! Two measures ship with the product:
//!
//! - [`Utf8UpperBound`], the default. A token of a byte-level tokenizer holds
//!   at least one byte, so the number of UTF-8 bytes is an upper bound of the
//!   number of tokens of any such tokenizer. A pack that fits the bound fits
//!   the real tokenizer; the price is that the bound is loose.
//! - [`LexicalV1`], a fully specified deterministic tokenizer. Its count is
//!   exact *for that tokenizer*, and it says so by name: it is not the
//!   tokenizer of any model.
//!
//! A model tokenizer plugs in at the edge by implementing [`TokenMeasure`].

use serde::{Deserialize, Serialize};

/// How certain a count is, and of what.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MeasureKind {
    /// The count is the exact count of the named tokenizer.
    Exact {
        /// The tokenizer that produced the count.
        tokenizer: String,
    },
    /// The count is never below the real count of any tokenizer of the
    /// family the method names. It does not say how far above it is.
    UpperBound {
        /// How the bound is computed.
        method: String,
    },
    /// Nothing can be said about the count.
    Unknown,
}

impl MeasureKind {
    /// Whether the count is the exact count of a named tokenizer.
    pub fn is_exact(&self) -> bool {
        matches!(self, MeasureKind::Exact { .. })
    }

    /// Whether a count under a limit proves that the real count is under it.
    ///
    /// True for an exact count and for an upper bound: the real count is at
    /// most the count. False for an unknown measure, whose count proves
    /// nothing.
    pub fn bounds_the_real_count(&self) -> bool {
        !matches!(self, MeasureKind::Unknown)
    }
}

/// A token count and the certainty it carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Measured {
    /// The count, in the unit of [`Measured::kind`].
    pub tokens: u64,
    /// How certain the count is. It is flattened into the count, so a
    /// published count reads `{"tokens": 12, "kind": "upper_bound",
    /// "method": "utf8-bytes"}`.
    #[serde(flatten)]
    pub kind: MeasureKind,
}

/// Counts the tokens of a text.
///
/// Implementations are pure: the same text gives the same count, whatever
/// the process, the host or the hour.
pub trait TokenMeasure: Send + Sync {
    /// Counts `text`.
    fn measure(&self, text: &str) -> Measured;
}

/// The default measure: UTF-8 bytes, an upper bound of any byte-level
/// tokenizer.
#[derive(Debug, Clone, Copy, Default)]
pub struct Utf8UpperBound;

impl Utf8UpperBound {
    /// The name of the method, as the answer publishes it.
    pub const METHOD: &'static str = "utf8-bytes";
}

impl TokenMeasure for Utf8UpperBound {
    fn measure(&self, text: &str) -> Measured {
        Measured {
            tokens: text.len() as u64,
            kind: MeasureKind::UpperBound {
                method: Self::METHOD.to_string(),
            },
        }
    }
}

/// A deterministic tokenizer defined by one rule.
///
/// A token is a maximal run of alphanumeric characters and underscores, or a
/// single other character that is not whitespace. Whitespace separates
/// tokens and is not one. The count is exact for this definition, and for
/// nothing else.
#[derive(Debug, Clone, Copy, Default)]
pub struct LexicalV1;

impl LexicalV1 {
    /// The name the tokenizer is configured by and published under.
    pub const NAME: &'static str = "lexical-v1";
}

impl TokenMeasure for LexicalV1 {
    fn measure(&self, text: &str) -> Measured {
        let mut tokens = 0u64;
        let mut inside_word = false;
        for character in text.chars() {
            let word_character = character.is_alphanumeric() || character == '_';
            if word_character {
                if !inside_word {
                    tokens += 1;
                }
                inside_word = true;
            } else {
                inside_word = false;
                if !character.is_whitespace() {
                    tokens += 1;
                }
            }
        }
        Measured {
            tokens,
            kind: MeasureKind::Exact {
                tokenizer: Self::NAME.to_string(),
            },
        }
    }
}

/// The measure a configured tokenizer name selects.
///
/// `None` for a name the product does not ship: a configuration naming a
/// tokenizer nobody can run is an error of the caller, and the product does
/// not substitute another measure behind its back.
pub fn measure_named(name: &str) -> Option<Box<dyn TokenMeasure>> {
    match name {
        LexicalV1::NAME => Some(Box::new(LexicalV1)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_measure_is_an_upper_bound_and_never_exact() {
        let measured = Utf8UpperBound.measure("fn añadir() {}");

        assert_eq!(measured.tokens, "fn añadir() {}".len() as u64);
        assert!(!measured.kind.is_exact());
        assert!(measured.kind.bounds_the_real_count());
    }

    #[test]
    fn a_configured_tokenizer_counts_exactly_by_its_own_rule() {
        let measured = LexicalV1.measure("pub fn parse_config(path: &str) -> u32 {}");

        // pub fn parse_config ( path : & str ) - > u32 { }
        assert_eq!(measured.tokens, 14);
        assert_eq!(
            measured.kind,
            MeasureKind::Exact {
                tokenizer: "lexical-v1".to_string()
            }
        );
    }

    #[test]
    fn an_unknown_measure_proves_nothing() {
        assert!(!MeasureKind::Unknown.bounds_the_real_count());
    }

    #[test]
    fn only_a_shipped_tokenizer_can_be_configured_by_name() {
        assert!(measure_named("lexical-v1").is_some());
        assert!(measure_named("gpt-something").is_none());
    }
}
