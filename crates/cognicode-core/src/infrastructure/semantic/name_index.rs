//! What a name search means, and the table that narrows it to candidates.
//!
//! Two pieces, because they change for different reasons:
//!
//! * the **rules** — [`classify`] decides whether a lowercase name matches a
//!   lowercase query and how well. There is one copy of them: the table
//!   below only narrows *which* names are worth asking, the rules still
//!   judge each one, so a candidate that was narrowed in wrongly is dropped
//!   and never reported;
//! * the **table** — [`NameTerms`] groups the entries of the index by their
//!   lowercase name, keeps those names ordered (an exact name is a lookup, a
//!   prefix is a range) and keeps one 64-bit signature per name (the
//!   characters it contains) so that a substring or subsequence query only
//!   looks at the names that have all the characters it needs.
//!
//! # Text semantics
//!
//! Names and queries are compared after `str::to_lowercase` (the Unicode
//! default case mapping) and **nothing else**: no normalization form is
//! applied, so a precomposed `é` and `e` followed by a combining accent are
//! different names. `Contains` and `Prefix` are byte-wise on the UTF-8
//! encoding, which is the same as character-wise because UTF-8 is
//! self-synchronizing; the `Contains` score uses the byte offset of the first
//! occurrence over the byte length of the name; `Fuzzy` is a subsequence of
//! Unicode scalar values.

use std::collections::BTreeMap;
use std::ops::Bound;
use std::sync::Arc;

use super::semantic_search::MatchType;

/// The score every approximate (subsequence) match gets.
///
/// Approximate matches are not ranked among themselves: a distance between
/// two spellings says how alike they look, not how relevant either symbol
/// is, and the old formula already collapsed to this value for every match.
pub(super) const FUZZY_SCORE: f32 = 0.6;

/// How well `name_lower` answers `query_lower`, if it does.
///
/// Both arguments are already lowercase. The order of the checks is the
/// order of the ranking: exact, prefix, substring, subsequence.
pub(super) fn classify(name_lower: &str, query_lower: &str) -> Option<(MatchType, f32)> {
    if name_lower == query_lower {
        return Some((MatchType::Exact, 1.0));
    }
    if name_lower.starts_with(query_lower) {
        return Some((MatchType::Prefix, 0.9));
    }
    if let Some(score) = contains_score(name_lower, query_lower) {
        return Some((MatchType::Contains, score));
    }
    let score = fuzzy_score(query_lower, name_lower);
    (score > 0.0).then_some((MatchType::Fuzzy, score))
}

/// The substring score of `name_lower` for `query_lower`: higher the earlier
/// the first occurrence is. `None` when the query is not a substring.
pub(super) fn contains_score(name_lower: &str, query_lower: &str) -> Option<f32> {
    let pos = name_lower.find(query_lower)?;
    Some(0.7 + (0.2 * (1.0 - pos as f32 / name_lower.len() as f32)))
}

/// `FUZZY_SCORE` when every character of `query` appears in `target` in
/// order (not necessarily adjacent), `0.0` otherwise.
pub(super) fn fuzzy_score(query: &str, target: &str) -> f32 {
    if query.is_empty() || target.is_empty() {
        return 0.0;
    }
    let mut wanted = query.chars().peekable();
    for c in target.chars() {
        if wanted.peek() == Some(&c) {
            wanted.next();
        }
    }
    if wanted.peek().is_none() {
        FUZZY_SCORE
    } else {
        0.0
    }
}

/// The set of characters of `text`, as a 64-bit mask.
///
/// `a-z`, `0-9` and `_` have a bit each; every other character shares one of
/// the remaining 27 bits by its code point. Sharing a bit can only make a
/// name look like it has a character it lacks, never the other way round, so
/// `signature(name) & signature(query) == signature(query)` is a *necessary*
/// condition for the query to be a substring or a subsequence of the name:
/// it can let a name through that does not match, it never drops one that
/// does.
pub(super) fn signature(text: &str) -> u64 {
    text.chars().fold(0, |mask, c| mask | character_bit(c))
}

fn character_bit(c: char) -> u64 {
    let bit = match c {
        'a'..='z' => c as u32 - 'a' as u32,
        '0'..='9' => 26 + (c as u32 - '0' as u32),
        '_' => 36,
        other => 37 + other as u32 % 27,
    };
    1u64 << bit
}

/// The entries of one distinct lowercase name.
struct Term {
    name: Arc<str>,
    /// Slots of the index that hold a symbol with this name, in no order.
    slots: Vec<u32>,
}

/// Entries grouped by lowercase name, ordered, with a character signature
/// per name.
///
/// Derived data: everything here can be rebuilt from the entries of the
/// index, which is what the index does when it finds the table in doubt.
pub(super) struct NameTerms {
    by_name: BTreeMap<Arc<str>, u32>,
    terms: Vec<Option<Term>>,
    /// One per term id; `0` for a free id, which no query signature matches.
    signatures: Vec<u64>,
    free: Vec<u32>,
}

impl NameTerms {
    pub(super) fn new() -> Self {
        Self {
            by_name: BTreeMap::new(),
            terms: Vec::new(),
            signatures: Vec::new(),
            free: Vec::new(),
        }
    }

    /// Records that `slot` holds a symbol named `name_lower`; returns the id
    /// of the name and the one copy of the name that every holder shares.
    pub(super) fn attach(&mut self, name_lower: &str, slot: u32) -> (u32, Arc<str>) {
        if let Some(&id) = self.by_name.get(name_lower) {
            if let Some(term) = self.terms[id as usize].as_mut() {
                term.slots.push(slot);
                return (id, Arc::clone(&term.name));
            }
        }
        let name: Arc<str> = Arc::from(name_lower);
        let term = Term {
            name: Arc::clone(&name),
            slots: vec![slot],
        };
        let mask = signature(name_lower);
        let id = match self.free.pop() {
            Some(id) => {
                self.terms[id as usize] = Some(term);
                self.signatures[id as usize] = mask;
                id
            }
            None => {
                self.terms.push(Some(term));
                self.signatures.push(mask);
                (self.terms.len() - 1) as u32
            }
        };
        self.by_name.insert(Arc::clone(&name), id);
        (id, name)
    }

    /// Forgets that `slot` holds a symbol of the name `term`; a name nobody
    /// holds any more disappears.
    pub(super) fn detach(&mut self, term: u32, slot: u32) {
        let Some(entry) = self.terms[term as usize].as_mut() else {
            return;
        };
        if let Some(at) = entry.slots.iter().position(|&s| s == slot) {
            entry.slots.swap_remove(at);
        }
        if entry.slots.is_empty() {
            let name = Arc::clone(&entry.name);
            self.by_name.remove(&*name);
            self.terms[term as usize] = None;
            self.signatures[term as usize] = 0;
            self.free.push(term);
        }
    }

    /// The id of the name equal to `name_lower`, if any entry has it.
    pub(super) fn exact(&self, name_lower: &str) -> Option<u32> {
        self.by_name.get(name_lower).copied()
    }

    /// Ids of the names that start with `prefix` (the name equal to it
    /// included), in name order.
    pub(super) fn with_prefix<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = u32> + 'a {
        self.by_name
            .range::<str, _>((Bound::Included(prefix), Bound::Unbounded))
            .take_while(move |(name, _)| name.starts_with(prefix))
            .map(|(_, &id)| id)
    }

    /// Ids of the names whose signature has every bit of `wanted`.
    pub(super) fn with_signature(&self, wanted: u64) -> impl Iterator<Item = u32> + '_ {
        self.signatures
            .iter()
            .enumerate()
            .filter(move |(_, have)| **have & wanted == wanted)
            .map(|(id, _)| id as u32)
    }

    /// The lowercase name of a term.
    pub(super) fn name(&self, term: u32) -> &str {
        self.terms[term as usize]
            .as_ref()
            .map_or("", |t| t.name.as_ref())
    }

    /// The slots of the entries that have this name.
    pub(super) fn slots(&self, term: u32) -> &[u32] {
        self.terms[term as usize]
            .as_ref()
            .map_or(&[], |t| t.slots.as_slice())
    }

    /// How many distinct names there are.
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.by_name.len()
    }

    /// Whether this table says exactly what `other` says: the same names with
    /// the same slots, whatever their ids.
    #[cfg(test)]
    pub(super) fn describes_the_same_as(&self, other: &NameTerms) -> bool {
        self.by_name.len() == other.by_name.len()
            && self.by_name.iter().all(|(name, &id)| {
                other.exact(name).is_some_and(|other_id| {
                    let mut mine = self.slots(id).to_vec();
                    let mut theirs = other.slots(other_id).to_vec();
                    mine.sort_unstable();
                    theirs.sort_unstable();
                    mine == theirs && self.signatures[id as usize] == signature(name)
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The score the previous implementation computed, kept as the oracle for
    /// the simplified one.
    fn previous_fuzzy_score(query: &str, target: &str) -> f32 {
        if query.is_empty() || target.is_empty() {
            return 0.0;
        }
        let query_chars: Vec<char> = query.chars().collect();
        let target_chars: Vec<char> = target.chars().collect();
        let (mut query_idx, mut matched, mut consecutive, mut max_consecutive) = (0, 0, 0, 0);
        for tc in &target_chars {
            if query_idx < query_chars.len() && *tc == query_chars[query_idx] {
                matched += 1;
                consecutive += 1;
                max_consecutive = max_consecutive.max(consecutive);
                query_idx += 1;
            } else {
                consecutive = 0;
            }
        }
        if query_idx < query_chars.len() {
            return 0.0;
        }
        let match_ratio = matched as f32 / query_chars.len() as f32;
        (match_ratio + max_consecutive as f32 * 0.1).min(0.6)
    }

    #[test]
    fn the_fuzzy_score_is_the_previous_one_on_every_pair() {
        let alphabet: Vec<char> = "abc_é日".chars().collect();
        let mut words = vec![String::new()];
        for _ in 0..4 {
            let next: Vec<String> = words
                .iter()
                .flat_map(|w| alphabet.iter().map(move |c| format!("{w}{c}")))
                .collect();
            words.extend(next);
            words.sort();
            words.dedup();
            words.truncate(900);
        }
        for query in words.iter().take(120) {
            for target in &words {
                assert_eq!(
                    fuzzy_score(query, target).to_bits(),
                    previous_fuzzy_score(query, target).to_bits(),
                    "query {query:?} target {target:?}"
                );
            }
        }
    }

    #[test]
    fn the_signature_never_drops_a_name_that_matches() {
        let names = ["handle_request", "ÑandúFoo", "日本語_x", "a1_b2", "zzz"];
        for name in names {
            let lower = name.to_lowercase();
            let chars: Vec<char> = lower.chars().collect();
            for start in 0..chars.len() {
                for end in start + 1..=chars.len() {
                    let sub: String = chars[start..end].iter().collect();
                    assert_eq!(
                        signature(&lower) & signature(&sub),
                        signature(&sub),
                        "{sub:?} is inside {lower:?}"
                    );
                }
            }
            let subsequence: String = chars.iter().step_by(2).collect();
            assert_eq!(
                signature(&lower) & signature(&subsequence),
                signature(&subsequence)
            );
        }
    }

    #[test]
    fn classify_ranks_exact_then_prefix_then_substring_then_subsequence() {
        assert_eq!(classify("foo", "foo"), Some((MatchType::Exact, 1.0)));
        assert_eq!(classify("foobar", "foo"), Some((MatchType::Prefix, 0.9)));
        let (kind, score) = classify("barfoo", "foo").expect("substring");
        assert_eq!(kind, MatchType::Contains);
        assert!(score > 0.7 && score < 0.9);
        assert_eq!(
            classify("f_o_o", "foo"),
            Some((MatchType::Fuzzy, FUZZY_SCORE))
        );
        assert_eq!(classify("bar", "foo"), None);
    }

    #[test]
    fn a_name_nobody_holds_any_more_leaves_the_table() {
        let mut terms = NameTerms::new();
        let (a, _) = terms.attach("alpha", 0);
        let (again, _) = terms.attach("alpha", 1);
        assert_eq!(a, again);
        terms.detach(a, 0);
        assert_eq!(terms.exact("alpha"), Some(a));
        terms.detach(a, 1);
        assert_eq!(terms.exact("alpha"), None);
        assert_eq!(terms.len(), 0);
        // the freed id is reused and carries the new name's signature
        let (b, _) = terms.attach("beta", 2);
        assert_eq!(b, a);
        assert_eq!(terms.name(b), "beta");
        assert!(terms.with_signature(signature("a")).any(|id| id == b));
        assert!(!terms.with_signature(signature("z")).any(|id| id == b));
    }

    #[test]
    fn prefix_lookup_is_a_range_in_name_order() {
        let mut terms = NameTerms::new();
        for (slot, name) in ["get", "get_all", "getter", "gets", "set", "ge"]
            .iter()
            .enumerate()
        {
            terms.attach(name, slot as u32);
        }
        let found: Vec<&str> = terms.with_prefix("get").map(|id| terms.name(id)).collect();
        assert_eq!(found, vec!["get", "get_all", "gets", "getter"]);
        assert_eq!(terms.with_prefix("zzz").count(), 0);
    }
}
