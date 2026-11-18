//! Per-snapshot statistics for the IDF-weighted symbol ranking, and the
//! bounded cache that lets every query of a snapshot share them.
//!
//! # Contract
//!
//! [`RankingStats::build`] reads one [`CallGraph`] once and keeps everything
//! the ranking needs that does not depend on the query: the inverse document
//! frequency of every name token, the degree of every symbol, the 95th
//! percentile of those degrees (the hub threshold) and the lowercase and
//! tokenised form of every name. [`RankingStats::rank`] then answers any
//! number of queries from that value without touching the graph again.
//!
//! The value is a pure function of the graph: ranking through it is equal,
//! bit for bit in the scores and position for position in the order, to
//! [`SearchRanker::search`](super::search_ranker::SearchRanker::search) over
//! the same graph. Ties are broken by symbol id, the order the graph
//! publishes its symbols in.
//!
//! # Identity and invalidation
//!
//! [`RankingStatsCache`] is keyed by the identity of the published immutable
//! `Arc<CallGraph>` (the entry of the checkpoint ring), not by a revision
//! number: the in-memory single-version store reports the same checkpoint id
//! after every overwrite, so a number cannot tell two snapshots apart, while
//! the allocation can. The cache holds a `Weak` pointer to the graph, which
//! has two consequences the cache relies on:
//!
//! - the allocation of a keyed graph cannot be reused for another graph
//!   while the entry exists, so an address never aliases two snapshots;
//! - `Arc::get_mut` refuses to hand out `&mut` to a graph that has a `Weak`,
//!   so a keyed graph cannot be edited in place under the cache.
//!
//! When the last strong reference to a snapshot goes away its entry is
//! dropped on the next lookup: scores of a retired snapshot are never served.
//!
//! # Memory
//!
//! The cache holds at most [`RankingCacheLimits::max_entries`] snapshots and
//! at most [`RankingCacheLimits::max_symbols`] symbols summed over them,
//! evicting the least recently used first. A single snapshot larger than the
//! symbol budget is ranked from a value that is not retained.
//!
//! # Concurrency
//!
//! Lookups take a short lock that never covers a build. Concurrent first
//! queries of the same snapshot build once: the others wait for that build
//! and share its result.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, Weak};

use crate::domain::aggregates::{CallGraph, SymbolId};

use super::search_ranker::{ScoredSymbol, SearchRanker};

/// One distinct name token of the snapshot.
struct Term {
    text: Box<str>,
    idf: f64,
}

/// One symbol with a non-empty name.
struct Entry {
    id: SymbolId,
    name: Box<str>,
    name_lower: Box<str>,
    file: Arc<str>,
    /// Range of this symbol's tokens inside `RankingStats::token_terms`.
    tokens: (u32, u32),
    degree: usize,
}

/// The part of the IDF ranking that depends on the snapshot and not on the
/// query. See the [module documentation](self) for the contract.
pub struct RankingStats {
    /// IDF of every name token and of every full lowercase name.
    idf: HashMap<Box<str>, f64>,
    /// Distinct name tokens, referenced by index from `token_terms`.
    vocabulary: Vec<Term>,
    /// Symbols with a non-empty name, in symbol id order.
    entries: Vec<Entry>,
    /// The tokens of every entry, in name order, as indexes into
    /// `vocabulary`.
    token_terms: Vec<u32>,
    /// Degrees above this are demoted by the hub bypass.
    hub_threshold: usize,
    /// Symbols and edges read while building.
    build_visits: usize,
    /// Symbols scanned by every `rank` call so far.
    scan_visits: AtomicU64,
}

impl RankingStats {
    /// Reads `graph` once and keeps the query-independent ranking state.
    pub fn build(graph: &CallGraph) -> Self {
        let documents = graph.symbol_count();
        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        let mut index_of: HashMap<&SymbolId, usize> = HashMap::with_capacity(documents);
        let mut tokenised: Vec<Vec<String>> = Vec::with_capacity(documents);
        let mut lowered: Vec<String> = Vec::with_capacity(documents);

        for (position, (id, symbol)) in graph.symbol_ids().enumerate() {
            index_of.insert(id, position);
            let tokens = SearchRanker::tokenize(symbol.name());
            let full = symbol.name().to_lowercase();
            // A document counts once for each distinct term, and its full
            // lowercase name is a term of its own.
            let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for token in &tokens {
                if seen.insert(token) {
                    *doc_freq.entry(token.clone()).or_insert(0) += 1;
                }
            }
            if seen.insert(&full) {
                *doc_freq.entry(full.clone()).or_insert(0) += 1;
            }
            tokenised.push(tokens);
            lowered.push(full);
        }

        let mut degrees = vec![0usize; documents];
        let mut edges = 0usize;
        for (source, target, _) in graph.all_dependencies() {
            if let (Some(&from), Some(&to)) = (index_of.get(source), index_of.get(target)) {
                degrees[from] += 1;
                degrees[to] += 1;
                edges += 1;
            }
        }
        let hub_threshold = hub_threshold(&degrees);

        let n = documents as f64;
        let idf: HashMap<Box<str>, f64> = doc_freq
            .into_iter()
            .map(|(term, df)| (term.into_boxed_str(), (1.0 + n / (1.0 + df as f64)).ln()))
            .collect();

        let mut vocabulary: Vec<Term> = Vec::new();
        let mut term_index: HashMap<String, u32> = HashMap::new();
        let mut token_terms: Vec<u32> = Vec::new();
        let mut files: HashMap<&str, Arc<str>> = HashMap::new();
        let mut entries: Vec<Entry> = Vec::with_capacity(documents);
        for (position, (id, symbol)) in graph.symbol_ids().enumerate() {
            if symbol.name().is_empty() {
                continue;
            }
            let start = token_terms.len() as u32;
            for token in std::mem::take(&mut tokenised[position]) {
                let term = *term_index.entry(token).or_insert_with_key(|text| {
                    vocabulary.push(Term {
                        text: text.as_str().into(),
                        idf: idf[text.as_str()],
                    });
                    (vocabulary.len() - 1) as u32
                });
                token_terms.push(term);
            }
            let file = symbol.location().file();
            let file = files.entry(file).or_insert_with(|| Arc::from(file)).clone();
            entries.push(Entry {
                id: id.clone(),
                name: symbol.name().into(),
                name_lower: std::mem::take(&mut lowered[position]).into_boxed_str(),
                file,
                tokens: (start, token_terms.len() as u32),
                degree: degrees[position],
            });
        }

        Self {
            idf,
            vocabulary,
            entries,
            token_terms,
            hub_threshold,
            build_visits: documents + edges,
            scan_visits: AtomicU64::new(0),
        }
    }

    /// Ranks the snapshot for `query` and returns at most `max_results`
    /// symbols, best first, equal scores in symbol id order.
    pub fn rank(&self, query: &str, max_results: usize) -> Vec<ScoredSymbol> {
        self.scan_visits
            .fetch_add(self.entries.len() as u64, Ordering::Relaxed);
        if max_results == 0 || self.entries.is_empty() {
            return Vec::new();
        }

        let query_lower = query.to_lowercase();
        let query_tokens = SearchRanker::tokenize(query);
        let name_idf = self.idf.get(query_lower.as_str()).copied().unwrap_or(1.0);
        // What each name token is worth against each query token, worked
        // out once per distinct token instead of once per symbol.
        let weights: Vec<Vec<f64>> = query_tokens
            .iter()
            .map(|query_token| {
                self.vocabulary
                    .iter()
                    .map(|term| token_weight(term, query_token))
                    .collect()
            })
            .collect();

        let mut hits: Vec<(f64, u32)> = Vec::new();
        for (position, entry) in self.entries.iter().enumerate() {
            let mut score = 0.0f64;
            if *entry.name_lower == *query_lower {
                score += name_idf * 1000.0;
            } else if !query_lower.is_empty() && entry.name_lower.contains(&query_lower) {
                score += name_idf * 100.0;
            }
            let (from, to) = entry.tokens;
            let tokens = &self.token_terms[from as usize..to as usize];
            for weight in &weights {
                for &term in tokens {
                    score += weight[term as usize];
                }
            }
            if score <= 0.0 {
                continue;
            }
            if entry.degree > self.hub_threshold {
                score *= 0.1;
            }
            hits.push((score, position as u32));
        }

        // Scores are finite, so this order is total and equals a stable sort
        // by score over the symbols in id order.
        let best_first = |a: &(f64, u32), b: &(f64, u32)| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.1.cmp(&b.1))
        };
        if hits.len() > max_results {
            hits.select_nth_unstable_by(max_results - 1, best_first);
            hits.truncate(max_results);
        }
        hits.sort_unstable_by(best_first);

        hits.into_iter()
            .map(|(score, position)| {
                let entry = &self.entries[position as usize];
                ScoredSymbol {
                    symbol_id: entry.id.clone(),
                    name: entry.name.to_string(),
                    file: entry.file.to_string(),
                    idf_score: score,
                    degree: entry.degree,
                }
            })
            .collect()
    }

    /// Symbols and edges read to build this value.
    pub fn build_visits(&self) -> usize {
        self.build_visits
    }

    /// Symbols scanned so far by every [`rank`](Self::rank) call.
    pub fn scan_visits(&self) -> u64 {
        self.scan_visits.load(Ordering::Relaxed)
    }
}

/// What a name token adds to the score of a symbol for one query token:
/// ×100 for an equal token, ×10 for a prefix, ×1 for a substring.
fn token_weight(term: &Term, query_token: &str) -> f64 {
    let text: &str = &term.text;
    if text == query_token {
        term.idf * 100.0
    } else if text.starts_with(query_token) {
        term.idf * 10.0
    } else if text.contains(query_token) {
        term.idf * 1.0
    } else {
        0.0
    }
}

/// The degree above which a symbol is a hub: the 95th percentile of the
/// total degree (in plus out) of every symbol.
fn hub_threshold(degrees: &[usize]) -> usize {
    if degrees.is_empty() {
        return usize::MAX;
    }
    let mut sorted = degrees.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() as f64 * 0.95) as usize).min(sorted.len() - 1);
    sorted[index]
}

/// How much a [`RankingStatsCache`] may keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankingCacheLimits {
    /// Most snapshots kept at once. The snapshot being served is always
    /// kept while it is in use, so values below one behave as one.
    pub max_entries: usize,
    /// Most symbols kept, summed over the snapshots. A snapshot with more
    /// symbols than this is never retained.
    pub max_symbols: usize,
}

impl Default for RankingCacheLimits {
    /// Two snapshots (the head and the one before it, as the checkpoint ring
    /// retains) within two million symbols.
    fn default() -> Self {
        Self {
            max_entries: 2,
            max_symbols: 2_000_000,
        }
    }
}

/// What a [`RankingStatsCache`] has done so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RankingCacheCounters {
    /// Times a [`RankingStats`] was built.
    pub builds: u64,
    /// Lookups served by an entry that already existed.
    pub hits: u64,
    /// Entries dropped for the budget or because their snapshot is gone.
    pub evictions: u64,
    /// Lookups answered from a value that was not retained because the
    /// snapshot alone is over the symbol budget.
    pub bypasses: u64,
    /// Snapshots held now.
    pub entries: usize,
    /// Symbols held now, summed over the snapshots.
    pub symbols: usize,
}

struct Slot {
    graph: Weak<CallGraph>,
    symbols: usize,
    stats: Arc<OnceLock<Arc<RankingStats>>>,
}

/// Shares one [`RankingStats`] per published snapshot between queries. See
/// the [module documentation](self) for identity, invalidation, memory and
/// concurrency.
pub struct RankingStatsCache {
    limits: RankingCacheLimits,
    /// Least recently used first.
    slots: Mutex<Vec<Slot>>,
    builds: AtomicU64,
    hits: AtomicU64,
    evictions: AtomicU64,
    bypasses: AtomicU64,
}

impl Default for RankingStatsCache {
    fn default() -> Self {
        Self::new(RankingCacheLimits::default())
    }
}

impl RankingStatsCache {
    /// An empty cache that keeps at most what `limits` allow.
    pub fn new(limits: RankingCacheLimits) -> Self {
        Self {
            limits,
            slots: Mutex::new(Vec::new()),
            builds: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
            bypasses: AtomicU64::new(0),
        }
    }

    /// The statistics of `graph`, built by the first caller and shared by
    /// the rest for as long as the entry lives.
    pub fn stats_for(&self, graph: &Arc<CallGraph>) -> Arc<RankingStats> {
        let symbols = graph.symbol_count();
        if symbols > self.limits.max_symbols {
            self.bypasses.fetch_add(1, Ordering::Relaxed);
            self.builds.fetch_add(1, Ordering::Relaxed);
            return Arc::new(RankingStats::build(graph));
        }
        let cell = self.slot_for(graph, symbols);
        Arc::clone(cell.get_or_init(|| {
            self.builds.fetch_add(1, Ordering::Relaxed);
            Arc::new(RankingStats::build(graph))
        }))
    }

    /// Finds or creates the entry of `graph`, most recently used last, and
    /// brings the cache back within its limits. Never builds.
    fn slot_for(&self, graph: &Arc<CallGraph>, symbols: usize) -> Arc<OnceLock<Arc<RankingStats>>> {
        let key = Arc::downgrade(graph);
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);

        let before = slots.len();
        slots.retain(|slot| slot.graph.strong_count() > 0);
        self.count_evictions(before - slots.len());

        if let Some(position) = slots.iter().position(|slot| slot.graph.ptr_eq(&key)) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            let slot = slots.remove(position);
            let cell = Arc::clone(&slot.stats);
            slots.push(slot);
            return cell;
        }

        let cell = Arc::new(OnceLock::new());
        slots.push(Slot {
            graph: key,
            symbols,
            stats: Arc::clone(&cell),
        });
        // The entry just added is last and is never the one to go: it fits
        // the symbol budget on its own.
        let mut evicted = 0;
        while slots.len() > 1
            && (slots.len() > self.limits.max_entries
                || slots.iter().map(|slot| slot.symbols).sum::<usize>() > self.limits.max_symbols)
        {
            slots.remove(0);
            evicted += 1;
        }
        self.count_evictions(evicted);
        cell
    }

    fn count_evictions(&self, evicted: usize) {
        if evicted > 0 {
            self.evictions.fetch_add(evicted as u64, Ordering::Relaxed);
        }
    }

    /// What the cache has done and holds right now.
    pub fn counters(&self) -> RankingCacheCounters {
        let slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        RankingCacheCounters {
            builds: self.builds.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            bypasses: self.bypasses.load(Ordering::Relaxed),
            entries: slots.len(),
            symbols: slots.iter().map(|slot| slot.symbols).sum(),
        }
    }
}
