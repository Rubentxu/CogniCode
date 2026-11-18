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
//! # Identity, memory and concurrency
//!
//! [`RankingStatsCache`] is a [`SnapshotCache`] of [`RankingStats`]: one value
//! per published immutable `Arc<CallGraph>`, keyed by the identity of the
//! allocation, bounded by [`RankingCacheLimits`], built once under concurrent
//! first queries and dropped with its snapshot. The rules are those of the
//! [`snapshot_cache`](crate::infrastructure::graph::snapshot_cache) module.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::domain::aggregates::{CallGraph, SymbolId};
use crate::infrastructure::graph::snapshot_cache::{
    SnapshotCache, SnapshotCacheCounters, SnapshotCacheLimits,
};

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
pub type RankingCacheLimits = SnapshotCacheLimits;

/// What a [`RankingStatsCache`] has done so far.
pub type RankingCacheCounters = SnapshotCacheCounters;

/// Shares one [`RankingStats`] per published snapshot between queries. See
/// the [module documentation](self) for identity, invalidation, memory and
/// concurrency; the cache itself is a [`SnapshotCache`].
#[derive(Default)]
pub struct RankingStatsCache {
    cache: SnapshotCache<Arc<RankingStats>>,
}

impl RankingStatsCache {
    /// An empty cache that keeps at most what `limits` allow.
    pub fn new(limits: RankingCacheLimits) -> Self {
        Self {
            cache: SnapshotCache::new(limits),
        }
    }

    /// The statistics of `graph`, built by the first caller and shared by
    /// the rest for as long as the entry lives.
    pub fn stats_for(&self, graph: &Arc<CallGraph>) -> Arc<RankingStats> {
        self.cache
            .get_or_build(graph, || Arc::new(RankingStats::build(graph)))
    }

    /// What the cache has done and holds right now.
    pub fn counters(&self) -> RankingCacheCounters {
        self.cache.counters()
    }
}
