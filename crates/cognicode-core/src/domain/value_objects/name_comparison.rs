//! How a name was compared with the declarations it named.
//!
//! The comparison is the language's property: a language that tells
//! `Describe` from `describe` answers only with the declaration spelled
//! the way the name was spelled, and a language that folds the case
//! answers with every declaration of the folded name. The rule lives in
//! [`Language`](crate::domain::value_objects::Language), and this type is
//! the word for the comparison that rule produced.
//!
//! The word is about the answer, not about the process: `exact` says that
//! every declaration the answer publishes is spelled the way the name
//! that was searched is spelled, and `case-insensitive` says that at least
//! one of them answers to the name only because the language that wrote
//! it folds the case. A search that folded the case and found nothing but
//! exact spellings is `exact`, because nothing in the answer mixes the
//! spellings.
//!
//! The word exists because a consumer that reads a count of declarations
//! has to know whether that count is the count of the name it asked for:
//! "seven declarations of `Ok`" and "seven declarations that answer to
//! `Ok`, six of them spelled `ok`" are different answers, and only one of
//! them can be read as the first.

use serde::{Deserialize, Serialize};

/// How the names were compared with the declarations an answer publishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NameComparison {
    /// Every declaration the answer publishes is spelled exactly the way
    /// the name that was searched is spelled.
    ///
    /// An answer with no declarations is exact too: nothing was folded,
    /// because nothing answered.
    Exact,
    /// At least one declaration answers to the name only because the
    /// language it is written in folds the case: `ok` answers to `Ok` in
    /// PHP, in PowerShell and in Fortran.
    CaseInsensitive,
}

impl NameComparison {
    /// The comparison that the declarations of an answer were found with.
    ///
    /// The answer compares the name it was given with the name each
    /// declaration carries, and it says which of the two it was.
    pub fn of<'a>(searched: &str, declared: impl Iterator<Item = &'a str>) -> Self {
        if declared.into_iter().any(|name| name != searched) {
            NameComparison::CaseInsensitive
        } else {
            NameComparison::Exact
        }
    }

    /// The word this comparison is published with.
    pub fn as_str(self) -> &'static str {
        match self {
            NameComparison::Exact => "exact",
            NameComparison::CaseInsensitive => "case-insensitive",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_declaration_spelled_the_same_way_is_an_exact_comparison() {
        assert_eq!(
            NameComparison::of("Ok", ["Ok", "Ok"].into_iter()),
            NameComparison::Exact
        );
    }

    #[test]
    fn a_declaration_spelled_differently_is_a_folded_comparison() {
        assert_eq!(
            NameComparison::of("Ok", ["Ok", "ok"].into_iter()),
            NameComparison::CaseInsensitive
        );
    }

    #[test]
    fn an_answer_without_declarations_is_exact() {
        assert_eq!(
            NameComparison::of("Ok", std::iter::empty()),
            NameComparison::Exact
        );
    }

    #[test]
    fn the_published_words_are_the_ones_a_consumer_reads() {
        assert_eq!(
            serde_json::to_string(&NameComparison::Exact).expect("serialisable"),
            "\"exact\""
        );
        assert_eq!(
            serde_json::to_string(&NameComparison::CaseInsensitive).expect("serialisable"),
            "\"case-insensitive\""
        );
    }
}
