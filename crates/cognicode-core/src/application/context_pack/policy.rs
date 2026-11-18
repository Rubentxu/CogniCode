//! The selection policy: what an adaptive pack weighs, and how.
//!
//! A policy is data. The same policy over the same snapshot and request
//! gives the same pack, so a pack can be reproduced and a change of weights
//! can be measured. The policy a pack was built with is published in the
//! answer: the version, the mode and every weight.
//!
//! No weight here is a probability or a confidence. A score is a ranking
//! value: it orders candidates for one request and means nothing outside it.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The version of the weights and rules below. A change of any default
/// weight or rule bumps it, so an answer says which policy produced it.
pub const POLICY_VERSION: &str = "selection-policy/v1";

/// What the reader is going to do with the pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    /// Find the cause of a defect: what is next to the suspect in the call
    /// chain, and the code that exercises it.
    Diagnose,
    /// Change a symbol safely: who depends on it and the contract it keeps.
    Refactor,
    /// Understand how the parts relate: the central symbols over the detail.
    Architecture,
}

impl Purpose {
    /// The word the request and the answer use.
    pub fn code(self) -> &'static str {
        match self {
            Purpose::Diagnose => "diagnose",
            Purpose::Refactor => "refactor",
            Purpose::Architecture => "architecture",
        }
    }
}

impl FromStr for Purpose {
    type Err = String;

    fn from_str(word: &str) -> Result<Self, String> {
        match word {
            "diagnose" => Ok(Purpose::Diagnose),
            "refactor" => Ok(Purpose::Refactor),
            "architecture" => Ok(Purpose::Architecture),
            other => Err(format!(
                "`{other}` is not a mode: use `diagnose`, `refactor` or `architecture`."
            )),
        }
    }
}

/// How much of a symbol a pack shows.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Detail {
    /// The declaration header, as written.
    Signature,
    /// The header plus the statements of the body that matter to the request,
    /// each with its original lines and the elided lines named.
    Slice,
    /// Every line of the declaration.
    Full,
}

impl Detail {
    /// The word the request and the answer use.
    pub fn code(self) -> &'static str {
        match self {
            Detail::Signature => "signature",
            Detail::Slice => "slice",
            Detail::Full => "full",
        }
    }

    /// The next smaller representation, when there is one.
    pub fn lower(self) -> Option<Detail> {
        match self {
            Detail::Full => Some(Detail::Slice),
            Detail::Slice => Some(Detail::Signature),
            Detail::Signature => None,
        }
    }
}

impl FromStr for Detail {
    type Err = String;

    fn from_str(word: &str) -> Result<Self, String> {
        match word {
            "signature" => Ok(Detail::Signature),
            "slice" => Ok(Detail::Slice),
            "full" => Ok(Detail::Full),
            other => Err(format!(
                "`{other}` is not a detail: use `auto`, `signature`, `slice` or `full`."
            )),
        }
    }
}

/// The part a symbol plays around the symbols the request is about.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// The caller named it, or its name is the intent.
    Seed,
    /// A trait or an interface: the contract of a seed.
    Interface,
    /// It inherits a contract, or a seed inherits it.
    Implementation,
    /// It uses a seed and is not a test.
    Consumer,
    /// The reader marked it as a test.
    Test,
    /// A seed uses it.
    Dependency,
    /// It is near the seeds and nothing more is known.
    Related,
}

impl Role {
    /// The word the answer uses.
    pub fn code(self) -> &'static str {
        match self {
            Role::Seed => "seed",
            Role::Interface => "interface",
            Role::Implementation => "implementation",
            Role::Consumer => "consumer",
            Role::Test => "test",
            Role::Dependency => "dependency",
            Role::Related => "related",
        }
    }
}

/// The weights of the signals that make up the score of a candidate.
///
/// They are relative: the score is their weighted mean, so only the ratios
/// matter.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Weights {
    /// The name matches the intent (the IDF ranker).
    pub lexical: f64,
    /// Personalized PageRank over the local neighbourhood of the seeds.
    pub ppr: f64,
    /// Closeness to a seed in the graph: one over one plus the hops.
    pub proximity: f64,
    /// How many symbols depend on it (callers), on a log scale.
    pub impact: f64,
}

impl Weights {
    /// The weights that only look at the name: the baseline of every
    /// comparison.
    pub const LEXICAL_ONLY: Weights = Weights {
        lexical: 1.0,
        ppr: 0.0,
        proximity: 0.0,
        impact: 0.0,
    };

    /// The weights that only look at the walk of the graph.
    pub const PPR_ONLY: Weights = Weights {
        lexical: 0.0,
        ppr: 1.0,
        proximity: 0.0,
        impact: 0.0,
    };

    /// The sum of the weights; zero when none is set.
    pub fn total(&self) -> f64 {
        self.lexical + self.ppr + self.proximity + self.impact
    }
}

/// Whether the pack walks the graph beyond the symbols the request names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Expansion {
    /// Walk the graph unless the request is a simple lookup the name alone
    /// answers.
    Auto,
    /// Always walk the graph.
    Always,
    /// Never walk the graph: only the symbols the request names.
    Never,
}

impl FromStr for Expansion {
    type Err = String;

    fn from_str(word: &str) -> Result<Self, String> {
        match word {
            "auto" => Ok(Expansion::Auto),
            "always" => Ok(Expansion::Always),
            "never" => Ok(Expansion::Never),
            other => Err(format!(
                "`{other}` is not an expansion: use `auto`, `always` or `never`."
            )),
        }
    }
}

/// The parametrizable policy of an adaptive selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SelectionPolicy {
    /// The version of the defaults this policy was built from.
    pub version: String,
    /// What the pack is for; `None` for a policy built by hand.
    pub mode: Option<Purpose>,
    /// The weights of the signals.
    pub weights: Weights,
    /// How strongly redundancy lowers the value of a candidate, from zero
    /// (no effect) to one (a duplicate is worth nothing).
    pub diversity: f64,
    /// The tokens added to the cost of every item before dividing the value
    /// by it, so that a one-line item does not win only by being short.
    pub cost_floor: u64,
    /// The extra value of the first item of a role that adds coverage.
    pub role_bonus: f64,
    /// Whether the graph is walked.
    pub expansion: Expansion,
    /// How many hops from the seeds the walk goes.
    pub depth: u32,
    /// The most neighbours of one symbol the walk follows.
    pub max_fanout: usize,
    /// The least confidence an edge needs to be believed.
    pub min_edge_confidence: f64,
    /// How many of the best lexical hits seed the walk when the request
    /// names no symbol.
    pub lexical_seeds: usize,
    /// The detail every item gets, instead of the one its role gets.
    pub detail_override: Option<Detail>,
}

impl SelectionPolicy {
    /// The default policy of a mode.
    ///
    /// The diagnose weights, the role bonus and the diversity are the ones a
    /// grid search kept on a fixed corpus and confirmed on families it never
    /// saw: a search of weights, then of the bonus and the diversity over the
    /// weights it kept, which settle on a plateau. The refactor and
    /// architecture weights are the declared reading of their purpose
    /// —impact over who depends on a symbol, PageRank over how central it
    /// is— and were measured, not searched: the corpus labels one relevant
    /// set. See `tests/it/adaptive_pack_evaluation.rs` for the numbers.
    pub fn for_mode(mode: Purpose) -> Self {
        let weights = match mode {
            Purpose::Diagnose => Weights {
                lexical: 0.15,
                ppr: 0.20,
                proximity: 0.20,
                impact: 0.20,
            },
            Purpose::Refactor => Weights {
                lexical: 0.10,
                ppr: 0.15,
                proximity: 0.20,
                impact: 0.40,
            },
            Purpose::Architecture => Weights {
                lexical: 0.10,
                ppr: 0.40,
                proximity: 0.10,
                impact: 0.25,
            },
        };
        Self {
            version: POLICY_VERSION.to_string(),
            mode: Some(mode),
            weights,
            diversity: 0.8,
            cost_floor: 24,
            role_bonus: 0.75,
            expansion: Expansion::Auto,
            depth: 2,
            max_fanout: 25,
            min_edge_confidence: 0.5,
            lexical_seeds: 3,
            detail_override: None,
        }
    }

    /// The detail a role gets in this policy.
    pub fn detail_for(&self, role: Role) -> Detail {
        if let Some(detail) = self.detail_override {
            return detail;
        }
        match (self.mode, role) {
            (Some(Purpose::Diagnose), Role::Seed) => Detail::Full,
            (Some(Purpose::Diagnose), Role::Implementation | Role::Consumer | Role::Test) => {
                Detail::Slice
            }
            (Some(Purpose::Refactor), Role::Seed) => Detail::Full,
            (Some(Purpose::Refactor), Role::Consumer) => Detail::Slice,
            (Some(Purpose::Architecture), Role::Seed) => Detail::Slice,
            _ => Detail::Signature,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mode_is_read_from_its_word_and_nothing_else() {
        assert_eq!("refactor".parse::<Purpose>(), Ok(Purpose::Refactor));
        assert!("Refactor".parse::<Purpose>().is_err());
        assert_eq!("slice".parse::<Detail>(), Ok(Detail::Slice));
        assert!("auto".parse::<Detail>().is_err());
    }

    #[test]
    fn each_mode_publishes_its_own_weights_under_one_version() {
        let diagnose = SelectionPolicy::for_mode(Purpose::Diagnose);
        let refactor = SelectionPolicy::for_mode(Purpose::Refactor);
        let architecture = SelectionPolicy::for_mode(Purpose::Architecture);

        assert_eq!(diagnose.version, POLICY_VERSION);
        assert_ne!(diagnose.weights, refactor.weights);
        assert!(refactor.weights.impact > diagnose.weights.impact);
        assert!(architecture.weights.ppr > diagnose.weights.ppr);
    }

    #[test]
    fn the_detail_follows_the_role_unless_the_caller_overrides_it() {
        let mut policy = SelectionPolicy::for_mode(Purpose::Diagnose);
        assert_eq!(policy.detail_for(Role::Seed), Detail::Full);
        assert_eq!(policy.detail_for(Role::Interface), Detail::Signature);

        policy.detail_override = Some(Detail::Signature);
        assert_eq!(policy.detail_for(Role::Seed), Detail::Signature);
    }

    #[test]
    fn a_detail_lowers_one_step_at_a_time_down_to_the_signature() {
        assert_eq!(Detail::Full.lower(), Some(Detail::Slice));
        assert_eq!(Detail::Slice.lower(), Some(Detail::Signature));
        assert_eq!(Detail::Signature.lower(), None);
    }
}
