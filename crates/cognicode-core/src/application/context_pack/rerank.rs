//! The order of an adaptive pack: value per token, without repeating itself.
//!
//! The candidates arrive with the signals that describe them. The order is
//! built greedily: at every step the candidate with the greatest
//! *incremental* contribution per token goes next, where the contribution is
//! its base value, lowered by how much it repeats what is already chosen and
//! raised when it adds a role the pack does not yet cover (an interface, an
//! implementation, a consumer, a test). Ties break on the symbol id, so the
//! same input gives the same order in every run.
//!
//! The function is pure: it reads no graph and no file. The cost of an item
//! is the tokens of the representation the policy plans for it, measured by
//! the caller with the measure of the pack.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::policy::{Role, SelectionPolicy, Weights};
use crate::domain::aggregates::call_graph::SymbolId;

/// How much two symbols of the same container repeat each other.
const SAME_CONTAINER_SIMILARITY: f64 = 0.8;

/// The signals of a candidate, each normalised to `[0, 1]` over the pool.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Signals {
    /// The name matches the intent.
    pub lexical: f64,
    /// The share of the personalized PageRank of the local neighbourhood.
    pub ppr: f64,
    /// Closeness to a seed.
    pub proximity: f64,
    /// How many symbols depend on it.
    pub impact: f64,
}

impl Signals {
    /// The weighted mean of the signals: a ranking value, not a confidence.
    pub fn value(&self, weights: &Weights) -> f64 {
        let total = weights.total();
        if total <= 0.0 {
            return 0.0;
        }
        (self.lexical * weights.lexical
            + self.ppr * weights.ppr
            + self.proximity * weights.proximity
            + self.impact * weights.impact)
            / total
    }
}

/// A candidate to order.
#[derive(Debug, Clone)]
pub struct Entry {
    /// The identity of the symbol.
    pub id: SymbolId,
    /// The signals that describe it.
    pub signals: Signals,
    /// The part it plays around the seeds.
    pub role: Role,
    /// The container that declares it, when the graph says which.
    pub container: Option<SymbolId>,
    /// The words of its name, lowercase.
    pub terms: Vec<String>,
    /// The tokens of the representation planned for it.
    pub cost: u64,
    /// Whether the caller named it: a named symbol goes first, in the order
    /// it was named, and is never reordered.
    pub pinned: bool,
}

/// Where a candidate landed and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// The identity of the symbol.
    pub id: SymbolId,
    /// The base value from the signals and the weights.
    pub value: f64,
    /// How much it repeats what was chosen before it, from zero to one.
    pub redundancy: f64,
    /// The value per token it was chosen with.
    pub priority: f64,
}

/// The words of an identifier: split on `_`, `-`, `.` and case changes.
pub fn name_terms(name: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for character in name.chars() {
        if !character.is_alphanumeric() {
            flush(&mut current, &mut terms);
            previous_lower = false;
            continue;
        }
        if character.is_uppercase() && previous_lower {
            flush(&mut current, &mut terms);
        }
        previous_lower = character.is_lowercase() || character.is_numeric();
        current.extend(character.to_lowercase());
    }
    flush(&mut current, &mut terms);
    terms
}

fn flush(current: &mut String, terms: &mut Vec<String>) {
    if !current.is_empty() {
        let term = std::mem::take(current);
        if !terms.contains(&term) {
            terms.push(term);
        }
    }
}

fn jaccard(a: &[String], b: &[String]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let left: HashSet<&String> = a.iter().collect();
    let right: HashSet<&String> = b.iter().collect();
    let shared = left.intersection(&right).count();
    shared as f64 / (left.len() + right.len() - shared) as f64
}

/// How much `entry` repeats `chosen`: the same container, or the same words.
fn similarity(entry: &Entry, chosen: &Entry) -> f64 {
    let same_container = entry.container.is_some() && entry.container == chosen.container;
    let by_container = if same_container {
        SAME_CONTAINER_SIMILARITY
    } else {
        0.0
    };
    by_container.max(jaccard(&entry.terms, &chosen.terms))
}

/// Whether covering this role again is worth a bonus: the roles that tell a
/// reader what the seed is, who uses it and what exercises it.
fn adds_coverage(role: Role) -> bool {
    matches!(
        role,
        Role::Interface | Role::Implementation | Role::Consumer | Role::Test
    )
}

/// Orders `entries` by incremental value per token.
pub fn rerank(entries: Vec<Entry>, policy: &SelectionPolicy) -> Vec<Placed> {
    let (pinned, mut rest): (Vec<Entry>, Vec<Entry>) =
        entries.into_iter().partition(|entry| entry.pinned);
    let mut chosen: Vec<Entry> = Vec::new();
    let mut placed: Vec<Placed> = Vec::new();
    let mut covered: HashSet<Role> = HashSet::new();

    for entry in pinned {
        placed.push(Placed {
            id: entry.id.clone(),
            value: entry.signals.value(&policy.weights),
            redundancy: 0.0,
            priority: f64::INFINITY,
        });
        covered.insert(entry.role);
        chosen.push(entry);
    }

    while !rest.is_empty() {
        let mut best: Option<(usize, Placed)> = None;
        for (at, entry) in rest.iter().enumerate() {
            let value = entry.signals.value(&policy.weights);
            let redundancy = chosen
                .iter()
                .map(|other| similarity(entry, other))
                .fold(0.0, f64::max);
            let bonus =
                if value > 0.0 && adds_coverage(entry.role) && !covered.contains(&entry.role) {
                    policy.role_bonus
                } else {
                    0.0
                };
            let contribution = value * (1.0 - policy.diversity * redundancy) + bonus;
            let priority = contribution / (policy.cost_floor + entry.cost) as f64;
            let candidate = Placed {
                id: entry.id.clone(),
                value,
                redundancy,
                priority,
            };
            let better = match &best {
                None => true,
                Some((_, current)) => candidate
                    .priority
                    .total_cmp(&current.priority)
                    .then_with(|| current.id.as_str().cmp(candidate.id.as_str()))
                    .is_gt(),
            };
            if better {
                best = Some((at, candidate));
            }
        }
        let (at, pick) = best.expect("the pool is not empty");
        let entry = rest.remove(at);
        covered.insert(entry.role);
        chosen.push(entry);
        placed.push(pick);
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::context_pack::policy::Purpose;

    fn entry(id: &str, value: f64, role: Role, container: Option<&str>, cost: u64) -> Entry {
        Entry {
            id: SymbolId::new(id),
            signals: Signals {
                lexical: value,
                ..Signals::default()
            },
            role,
            container: container.map(SymbolId::new),
            terms: name_terms(id),
            cost,
            pinned: false,
        }
    }

    fn policy() -> SelectionPolicy {
        let mut policy = SelectionPolicy::for_mode(Purpose::Diagnose);
        policy.weights = Weights::LEXICAL_ONLY;
        // Fixed here, so these cases do not move when the defaults do.
        policy.diversity = 0.6;
        policy.role_bonus = 0.25;
        policy
    }

    fn order(placed: &[Placed]) -> Vec<&str> {
        placed.iter().map(|p| p.id.as_str()).collect()
    }

    #[test]
    fn names_are_split_into_words_by_case_and_separators() {
        assert_eq!(
            name_terms("FileStorage_save2"),
            ["file", "storage", "save2"]
        );
        assert_eq!(name_terms("parse"), ["parse"]);
    }

    #[test]
    fn a_second_symbol_of_the_same_container_waits_for_one_that_adds_something() {
        let entries = vec![
            entry("alpha", 0.9, Role::Related, Some("C"), 20),
            entry("bravo", 0.85, Role::Related, Some("C"), 20),
            entry("charlie", 0.6, Role::Related, Some("D"), 20),
        ];

        let placed = rerank(entries, &policy());

        assert_eq!(order(&placed), ["alpha", "charlie", "bravo"]);
        assert!(placed[2].redundancy >= SAME_CONTAINER_SIMILARITY);
    }

    #[test]
    fn a_role_the_pack_does_not_cover_yet_is_worth_a_bonus() {
        let entries = vec![
            entry("alpha", 1.0, Role::Related, None, 20),
            entry("bravo", 0.8, Role::Related, None, 20),
            entry("charlie", 0.7, Role::Test, None, 20),
        ];

        let placed = rerank(entries, &policy());

        assert_eq!(order(&placed), ["alpha", "charlie", "bravo"]);
    }

    #[test]
    fn the_cost_counts_but_a_tiny_item_does_not_win_by_being_tiny() {
        let entries = vec![
            entry("big", 0.9, Role::Related, None, 400),
            entry("small", 0.5, Role::Related, None, 10),
            entry("dust", 0.05, Role::Related, None, 1),
        ];

        let placed = rerank(entries, &policy());

        assert_eq!(order(&placed), ["small", "big", "dust"]);
    }

    #[test]
    fn equal_priorities_break_on_the_id_so_the_order_is_stable() {
        let entries = vec![
            entry("zulu", 0.5, Role::Related, None, 20),
            entry("kilo", 0.5, Role::Related, None, 20),
        ];

        let first = rerank(entries.clone(), &policy());
        let second = rerank(entries.into_iter().rev().collect(), &policy());

        assert_eq!(order(&first), ["kilo", "zulu"]);
        assert_eq!(order(&second), ["kilo", "zulu"]);
    }

    #[test]
    fn a_pinned_symbol_goes_first_and_is_not_reordered() {
        let mut named = entry("named", 0.0, Role::Seed, None, 20);
        named.pinned = true;
        let entries = vec![entry("alpha", 0.9, Role::Related, None, 20), named];

        let placed = rerank(entries, &policy());

        assert_eq!(order(&placed), ["named", "alpha"]);
    }
}
