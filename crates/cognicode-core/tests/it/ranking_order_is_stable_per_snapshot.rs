//! The ranking of `graph_search_idf` is a function of the snapshot and the
//! query, and nothing else.

use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::analytics::search_ranker::SearchRanker;

/// A graph with ties (the same name in several files), hubs (a symbol
/// that most others call) and compound names in three spellings.
fn fixture_graph() -> CallGraph {
    let verbs = ["get", "set", "load", "build"];
    let nouns = ["user", "order", "index", "graph"];
    let files = ["a.rs", "b.rs", "c.rs"];
    let mut graph = CallGraph::new();
    let mut ids: Vec<SymbolId> = Vec::new();
    let mut line = 1;
    for file in files {
        for verb in verbs {
            for noun in nouns {
                let name = match file {
                    "a.rs" => format!("{verb}_{noun}"),
                    "b.rs" => format!("{verb}{}{}", &noun[..1].to_uppercase(), &noun[1..]),
                    _ => format!("{verb}::{noun}"),
                };
                ids.push(graph.add_symbol(Symbol::new(
                    &name,
                    SymbolKind::Function,
                    Location::new(file, line, 1),
                )));
                line += 1;
            }
        }
    }
    let hub = graph.add_symbol(Symbol::new(
        "process",
        SymbolKind::Function,
        Location::new("hub.rs", 1, 1),
    ));
    for (i, id) in ids.iter().enumerate() {
        // Most symbols call the hub; a few call a neighbour.
        if i % 5 != 0 {
            graph
                .add_dependency(id, &hub, DependencyType::Calls)
                .unwrap();
        }
        let next = &ids[(i * 7 + 3) % ids.len()];
        if next != id {
            graph
                .add_dependency(id, next, DependencyType::Calls)
                .unwrap();
        }
    }
    graph
}

const QUERIES: &[&str] = &[
    "user",
    "get_user",
    "getUser",
    "process",
    "graph index",
    "BUILD",
    "set",
    "",
    "zzz-no-match",
];

/// `(symbol id, score bits, degree)` of every result: the whole observable
/// outcome, with the score compared bit for bit.
fn observable(graph: &CallGraph, query: &str, k: usize) -> Vec<(String, u64, usize)> {
    SearchRanker::search(graph, query, k)
        .into_iter()
        .map(|r| {
            (
                r.symbol_id.as_str().to_string(),
                r.idf_score.to_bits(),
                r.degree,
            )
        })
        .collect()
}

/// Expected top 5 per query, recorded from the ranking before any
/// statistics were reused: `(query, [(symbol id, score bits, degree)])`.
#[allow(clippy::type_complexity)]
const GOLDEN_TOP5: &[(&str, &[(&str, u64, usize)])] = &[
    (
        "user",
        &[
            ("a.rs:build_user:13", 4644204066119821087, 3),
            ("a.rs:get_user:1", 4644204066119821087, 2),
            ("a.rs:load_user:9", 4644204066119821087, 3),
            ("a.rs:set_user:5", 4644204066119821087, 3),
            ("b.rs:buildUser:29", 4644204066119821087, 3),
        ],
    ),
    (
        "get_user",
        &[
            ("a.rs:get_user:1", 4660027400548313272, 2),
            ("b.rs:getUser:17", 4644204066119821087, 3),
            ("c.rs:get::user:33", 4644204066119821087, 3),
            ("a.rs:build_user:13", 4639700466492450591, 3),
            ("a.rs:get_graph:4", 4639700466492450591, 3),
        ],
    ),
    (
        "getUser",
        &[
            ("b.rs:getUser:17", 4660027400548313272, 3),
            ("a.rs:get_user:1", 4644204066119821087, 2),
            ("c.rs:get::user:33", 4644204066119821087, 3),
            ("a.rs:build_user:13", 4639700466492450591, 3),
            ("a.rs:get_graph:4", 4639700466492450591, 3),
        ],
    ),
    ("process", &[("hub.rs:process:1", 4644974913917147753, 38)]),
    ("graph index", &[]),
    (
        "BUILD",
        &[
            ("a.rs:build_index:15", 4641074578974503143, 3),
            ("b.rs:buildIndex:31", 4641074578974503143, 2),
            ("c.rs:build::index:47", 4641074578974503143, 3),
            ("a.rs:build_user:13", 4641019614475221041, 3),
            ("b.rs:buildUser:29", 4641019614475221041, 3),
        ],
    ),
    (
        "set",
        &[
            ("a.rs:set_graph:8", 4644204066119821087, 3),
            ("a.rs:set_index:7", 4644204066119821087, 3),
            ("a.rs:set_order:6", 4644204066119821087, 2),
            ("a.rs:set_user:5", 4644204066119821087, 3),
            ("b.rs:setGraph:24", 4644204066119821087, 3),
        ],
    ),
    ("", &[]),
    ("zzz-no-match", &[]),
];

/// Every hit of "BUILD", in order, with ties broken by insertion order.
const GOLDEN_BUILD_ORDER: &[&str] = &[
    "a.rs:build_index:15",
    "b.rs:buildIndex:31",
    "c.rs:build::index:47",
    "a.rs:build_user:13",
    "b.rs:buildUser:29",
    "c.rs:build::user:45",
    "a.rs:build_order:14",
    "b.rs:buildOrder:30",
    "c.rs:build::order:46",
    "a.rs:build_graph:16",
    "b.rs:buildGraph:32",
    "c.rs:build::graph:48",
    "a.rs:load_index:11",
    "b.rs:loadIndex:27",
    "c.rs:load::index:43",
    "a.rs:load_user:9",
    "b.rs:loadUser:25",
    "c.rs:load::user:41",
    "a.rs:load_order:10",
    "b.rs:loadOrder:26",
    "c.rs:load::order:42",
    "a.rs:get_index:3",
    "a.rs:load_graph:12",
    "a.rs:set_index:7",
    "b.rs:getIndex:19",
    "b.rs:loadGraph:28",
    "b.rs:setIndex:23",
    "c.rs:get::index:35",
    "c.rs:load::graph:44",
    "c.rs:set::index:39",
    "a.rs:get_user:1",
    "a.rs:set_user:5",
    "b.rs:getUser:17",
    "b.rs:setUser:21",
    "c.rs:get::user:33",
    "c.rs:set::user:37",
    "a.rs:get_order:2",
    "a.rs:set_order:6",
    "b.rs:getOrder:18",
    "b.rs:setOrder:22",
    "c.rs:get::order:34",
    "c.rs:set::order:38",
];

#[test]
fn golden_top5_matches_recorded_ranking() {
    let graph = fixture_graph();
    assert_eq!(GOLDEN_TOP5.len(), QUERIES.len(), "every query has a golden");
    for (query, expected) in GOLDEN_TOP5 {
        let got = observable(&graph, query, 5);
        let want: Vec<(String, u64, usize)> = expected
            .iter()
            .map(|(id, bits, degree)| (id.to_string(), *bits, *degree))
            .collect();
        assert_eq!(got, want, "query {query:?}");
    }
}

#[test]
fn golden_build_order_is_total_and_stable() {
    let graph = fixture_graph();
    let ids: Vec<String> = observable(&graph, "BUILD", 100)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert_eq!(ids, GOLDEN_BUILD_ORDER);
}

// ---------------------------------------------------------------------------
// Reuse of the statistics per snapshot
// ---------------------------------------------------------------------------

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Barrier};

use cognicode_core::infrastructure::graph::CallGraphProjection;
use cognicode_core::infrastructure::graph::analytics::ranking_stats::{
    RankingCacheLimits, RankingStats, RankingStatsCache,
};
use petgraph::Direction;

/// The ranking as it was before the statistics were reused: IDF, projection
/// and degrees rebuilt for the call. It is the oracle every reused value is
/// compared with.
fn legacy_observable(graph: &CallGraph, query: &str, k: usize) -> Vec<Seen> {
    if graph.symbol_count() == 0 {
        return Vec::new();
    }
    let n = graph.symbol_count() as f64;
    let mut doc_freq: HashMap<String, usize> = HashMap::new();
    for (_, symbol) in graph.symbol_ids() {
        let mut seen: HashSet<String> = HashSet::new();
        for token in SearchRanker::tokenize(symbol.name()) {
            if seen.insert(token.clone()) {
                *doc_freq.entry(token).or_insert(0) += 1;
            }
        }
        let full = symbol.name().to_lowercase();
        if seen.insert(full.clone()) {
            *doc_freq.entry(full).or_insert(0) += 1;
        }
    }
    let idf: HashMap<String, f64> = doc_freq
        .into_iter()
        .map(|(term, df)| (term, (1.0 + n / (1.0 + df as f64)).ln()))
        .collect();

    let projection = CallGraphProjection::from_call_graph(graph);
    let g = projection.graph();
    let degree_of = |ni| {
        g.edges_directed(ni, Direction::Outgoing).count()
            + g.edges_directed(ni, Direction::Incoming).count()
    };
    let mut sorted: Vec<usize> = g.node_indices().map(degree_of).collect();
    sorted.sort();
    let p95 = sorted[((sorted.len() as f64 * 0.95) as usize).min(sorted.len() - 1)];

    let query_lower = query.to_lowercase();
    let query_tokens = SearchRanker::tokenize(query);
    let mut scored: Vec<Seen> = Vec::new();
    for ni in g.node_indices() {
        let id = &g[ni];
        let symbol = graph.get_symbol(id).unwrap();
        let name = symbol.name().to_string();
        if name.is_empty() {
            continue;
        }
        let name_lower = name.to_lowercase();
        let name_tokens = SearchRanker::tokenize(&name);
        let mut score = 0.0f64;
        if name_lower == query_lower {
            score += idf.get(&query_lower).unwrap_or(&1.0) * 1000.0;
        } else if !query_lower.is_empty() && name_lower.contains(&query_lower) {
            score += idf.get(&query_lower).unwrap_or(&1.0) * 100.0;
        }
        for qt in &query_tokens {
            for nt in &name_tokens {
                let token_idf = idf.get(nt).unwrap_or(&1.0);
                if nt == qt {
                    score += token_idf * 100.0;
                } else if nt.starts_with(qt) {
                    score += token_idf * 10.0;
                } else if nt.contains(qt) {
                    score += token_idf * 1.0;
                }
            }
        }
        if score <= 0.0 {
            continue;
        }
        let degree = degree_of(ni);
        if degree > p95 {
            score *= 0.1;
        }
        scored.push(Seen {
            id: id.as_str().to_string(),
            name,
            file: symbol.location().file().to_string(),
            score_bits: score.to_bits(),
            degree,
        });
    }
    scored.sort_by(|a, b| {
        f64::from_bits(b.score_bits)
            .partial_cmp(&f64::from_bits(a.score_bits))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(k);
    scored
}

#[derive(Debug, PartialEq, Eq)]
struct Seen {
    id: String,
    name: String,
    file: String,
    score_bits: u64,
    degree: usize,
}

fn seen(
    results: Vec<cognicode_core::infrastructure::graph::analytics::search_ranker::ScoredSymbol>,
) -> Vec<Seen> {
    results
        .into_iter()
        .map(|r| Seen {
            id: r.symbol_id.as_str().to_string(),
            name: r.name,
            file: r.file,
            score_bits: r.idf_score.to_bits(),
            degree: r.degree,
        })
        .collect()
}

/// Deterministic generator: the same seed gives the same graph.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const WORDS: &[&str] = &[
    "get", "set", "user", "order", "parse", "Config", "http", "Request", "node", "id", "x",
];

/// A graph shaped to hit every branch of the ranking: repeated names in
/// several files, empty names, three spellings, self loops, parallel edges
/// of different kinds and a hub or two.
fn random_graph(seed: u64, symbols: usize) -> CallGraph {
    let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut graph = CallGraph::new();
    let mut ids: Vec<SymbolId> = Vec::new();
    for line in 0..symbols {
        let parts = 1 + rng.below(3);
        let words: Vec<&str> = (0..parts).map(|_| WORDS[rng.below(WORDS.len())]).collect();
        let name = match rng.below(5) {
            0 => words.join("_"),
            1 => words.concat(),
            2 => words.join("::"),
            3 => String::new(),
            _ => words[0].to_string(),
        };
        let file = format!("f{}.rs", rng.below(4));
        ids.push(graph.add_symbol(Symbol::new(
            &name,
            SymbolKind::Function,
            Location::new(&file, line as u32 + 1, 1),
        )));
    }
    let kinds = [DependencyType::Calls, DependencyType::Imports];
    for _ in 0..symbols * 2 {
        let from = &ids[rng.below(ids.len())];
        let to = if rng.below(4) == 0 {
            &ids[0]
        } else {
            &ids[rng.below(ids.len())]
        };
        let _ = graph.add_dependency(from, to, kinds[rng.below(2)]);
    }
    graph
}

fn random_queries(seed: u64) -> Vec<String> {
    let mut rng = Lcg(seed ^ 0xDEAD_BEEF_1234_5678 | 1);
    let mut queries = vec![
        String::new(),
        "zzz".to_string(),
        "_".to_string(),
        "::".to_string(),
    ];
    for _ in 0..8 {
        let a = WORDS[rng.below(WORDS.len())];
        let b = WORDS[rng.below(WORDS.len())];
        queries.push(a.to_string());
        queries.push(a.to_uppercase());
        queries.push(format!("{a}_{b}"));
        queries.push(format!("{a}{}{}", b[..1].to_uppercase(), &b[1..]));
        queries.push(format!("{a}::{b}"));
        queries.push(a[..1].to_string());
    }
    queries
}

#[test]
fn reused_stats_are_bit_for_bit_what_a_fresh_ranking_gives() {
    for seed in 1..=40u64 {
        let size = [1usize, 2, 7, 40, 150][seed as usize % 5];
        let graph = random_graph(seed, size);
        let stats = RankingStats::build(&graph);
        for query in random_queries(seed) {
            for k in [1usize, 3, 20, 10_000] {
                let reused = seen(stats.rank(&query, k));
                assert_eq!(
                    reused,
                    legacy_observable(&graph, &query, k),
                    "seed {seed} size {size} query {query:?} k {k}"
                );
                assert_eq!(
                    reused,
                    seen(SearchRanker::search(&graph, &query, k)),
                    "search seed {seed} size {size} query {query:?} k {k}"
                );
            }
        }
    }
}

#[test]
fn an_empty_graph_and_a_zero_budget_rank_nothing() {
    let empty = CallGraph::new();
    assert!(RankingStats::build(&empty).rank("user", 5).is_empty());
    let graph = fixture_graph();
    assert!(RankingStats::build(&graph).rank("user", 0).is_empty());
}

#[test]
fn golden_top5_holds_through_reused_stats() {
    let graph = fixture_graph();
    let stats = RankingStats::build(&graph);
    for (query, expected) in GOLDEN_TOP5 {
        let got: Vec<(String, u64, usize)> = stats
            .rank(query, 5)
            .into_iter()
            .map(|r| {
                (
                    r.symbol_id.as_str().to_string(),
                    r.idf_score.to_bits(),
                    r.degree,
                )
            })
            .collect();
        let want: Vec<(String, u64, usize)> = expected
            .iter()
            .map(|(id, bits, degree)| (id.to_string(), *bits, *degree))
            .collect();
        assert_eq!(got, want, "query {query:?}");
    }
}

#[test]
fn repeated_queries_share_one_build_and_one_value() {
    let cache = RankingStatsCache::default();
    let graph = Arc::new(fixture_graph());
    let first = cache.stats_for(&graph);
    for _ in 0..20 {
        assert!(Arc::ptr_eq(&first, &cache.stats_for(&graph)));
    }
    let counters = cache.counters();
    assert_eq!(counters.builds, 1);
    assert_eq!(counters.hits, 20);
    assert_eq!(counters.entries, 1);
    assert_eq!(counters.symbols, graph.symbol_count());
}

#[test]
fn instrumentation_counts_what_was_built_and_what_was_scanned() {
    let graph = Arc::new(fixture_graph());
    let stats = RankingStatsCache::default().stats_for(&graph);
    assert_eq!(
        stats.build_visits(),
        graph.symbol_count() + graph.edge_count(),
        "one visit per symbol and per edge, once"
    );
    assert_eq!(stats.scan_visits(), 0);
    for query in QUERIES {
        let _ = stats.rank(query, 5);
    }
    assert_eq!(
        stats.scan_visits() as usize,
        QUERIES.len() * graph.symbol_count(),
        "one visit per symbol per query, and no rebuild"
    );
}

#[test]
fn a_new_snapshot_never_serves_the_scores_of_the_old_one() {
    let cache = RankingStatsCache::default();
    let old = Arc::new(fixture_graph());
    let before = seen(cache.stats_for(&old).rank("authenticate", 5));
    assert!(before.is_empty());

    // The next snapshot has one more symbol; the old one is still alive.
    let mut grown = (*old).clone();
    grown.add_symbol(Symbol::new(
        "authenticate",
        SymbolKind::Function,
        Location::new("auth.rs", 1, 1),
    ));
    let new = Arc::new(grown);
    let stats = cache.stats_for(&new);
    assert_eq!(cache.counters().builds, 2);
    let after = seen(stats.rank("authenticate", 5));
    assert_eq!(after.len(), 1);
    assert_eq!(after, legacy_observable(&new, "authenticate", 5));
    // And the old snapshot still answers as itself.
    assert!(seen(cache.stats_for(&old).rank("authenticate", 5)).is_empty());
}

#[test]
fn an_equal_graph_in_another_allocation_is_another_snapshot() {
    let cache = RankingStatsCache::default();
    let a = Arc::new(fixture_graph());
    let b = Arc::new(fixture_graph());
    assert!(!Arc::ptr_eq(&cache.stats_for(&a), &cache.stats_for(&b)));
    assert_eq!(cache.counters().builds, 2);
}

#[test]
fn a_retired_snapshot_leaves_the_cache() {
    let cache = RankingStatsCache::new(RankingCacheLimits {
        max_entries: 8,
        max_symbols: usize::MAX,
    });
    let retired = Arc::new(fixture_graph());
    let _ = cache.stats_for(&retired);
    assert_eq!(cache.counters().entries, 1);
    drop(retired);

    let live = Arc::new(fixture_graph());
    let _ = cache.stats_for(&live);
    let counters = cache.counters();
    assert_eq!(counters.entries, 1, "only the live snapshot is held");
    assert_eq!(counters.evictions, 1);
}

#[test]
fn a_reused_address_never_aliases_a_new_snapshot() {
    let cache = RankingStatsCache::new(RankingCacheLimits {
        max_entries: 64,
        max_symbols: usize::MAX,
    });
    for seed in 1..=60u64 {
        let graph = Arc::new(random_graph(seed, 30));
        let query = "user";
        assert_eq!(
            seen(cache.stats_for(&graph).rank(query, 10)),
            legacy_observable(&graph, query, 10),
            "seed {seed}"
        );
        // The graph goes away and the allocator is free to hand its address
        // to the next one.
    }
}

#[test]
fn a_keyed_snapshot_cannot_be_edited_in_place() {
    let cache = RankingStatsCache::default();
    let mut graph = Arc::new(fixture_graph());
    let _ = cache.stats_for(&graph);
    assert!(
        Arc::get_mut(&mut graph).is_none(),
        "the cache's weak pointer must forbid in-place edits"
    );
}

#[test]
fn the_cache_stays_within_its_entry_budget_evicting_the_oldest() {
    let cache = RankingStatsCache::new(RankingCacheLimits {
        max_entries: 2,
        max_symbols: usize::MAX,
    });
    let graphs: Vec<Arc<CallGraph>> = (1..=4).map(|s| Arc::new(random_graph(s, 20))).collect();
    let first = cache.stats_for(&graphs[0]);
    for graph in &graphs[1..] {
        let _ = cache.stats_for(graph);
        assert!(cache.counters().entries <= 2);
    }
    let counters = cache.counters();
    assert_eq!(
        (counters.entries, counters.evictions, counters.builds),
        (2, 2, 4)
    );
    // The oldest is gone: asking again rebuilds, and the value is a new one.
    assert!(!Arc::ptr_eq(&first, &cache.stats_for(&graphs[0])));
    assert_eq!(cache.counters().builds, 5);
}

#[test]
fn a_recently_used_entry_outlives_an_idle_one() {
    let cache = RankingStatsCache::new(RankingCacheLimits {
        max_entries: 2,
        max_symbols: usize::MAX,
    });
    let (a, b, c) = (
        Arc::new(random_graph(1, 20)),
        Arc::new(random_graph(2, 20)),
        Arc::new(random_graph(3, 20)),
    );
    let kept = cache.stats_for(&a);
    let _ = cache.stats_for(&b);
    let _ = cache.stats_for(&a); // a is now the most recently used
    let _ = cache.stats_for(&c); // evicts b, not a
    assert!(Arc::ptr_eq(&kept, &cache.stats_for(&a)));
}

#[test]
fn the_symbol_budget_bounds_memory_and_oversized_snapshots_are_not_retained() {
    let cache = RankingStatsCache::new(RankingCacheLimits {
        max_entries: 8,
        max_symbols: 50,
    });
    let small: Vec<Arc<CallGraph>> = (1..=4).map(|s| Arc::new(random_graph(s, 20))).collect();
    for graph in &small {
        let _ = cache.stats_for(graph);
        assert!(cache.counters().symbols <= 50);
    }
    assert_eq!(
        cache.counters().entries,
        2,
        "20 + 20 fits in 50, a third does not"
    );

    let big = Arc::new(random_graph(9, 80));
    let first = cache.stats_for(&big);
    let second = cache.stats_for(&big);
    let counters = cache.counters();
    assert_eq!(counters.bypasses, 2);
    assert!(
        !Arc::ptr_eq(&first, &second),
        "an oversized value is not retained"
    );
    assert!(counters.symbols <= 50);
    assert_eq!(
        seen(first.rank("get", 10)),
        legacy_observable(&big, "get", 10),
        "and it ranks like any other"
    );
}

#[test]
fn concurrent_first_queries_build_once_and_agree() {
    let cache = Arc::new(RankingStatsCache::default());
    let graph = Arc::new(random_graph(5, 300));
    let threads = 16;
    let gate = Arc::new(Barrier::new(threads));
    let workers: Vec<_> = (0..threads)
        .map(|_| {
            let (cache, graph, gate) = (cache.clone(), graph.clone(), gate.clone());
            std::thread::spawn(move || {
                gate.wait();
                let stats = cache.stats_for(&graph);
                (stats.clone(), seen(stats.rank("get_user", 10)))
            })
        })
        .collect();
    let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    let expected = legacy_observable(&graph, "get_user", 10);
    for (stats, ranked) in &results {
        assert!(Arc::ptr_eq(stats, &results[0].0));
        assert_eq!(ranked, &expected);
    }
    assert_eq!(cache.counters().builds, 1, "single flight");
}

#[test]
fn concurrent_queries_over_many_snapshots_stay_within_budget() {
    let cache = Arc::new(RankingStatsCache::new(RankingCacheLimits {
        max_entries: 3,
        max_symbols: usize::MAX,
    }));
    let graphs: Vec<Arc<CallGraph>> = (1..=6).map(|s| Arc::new(random_graph(s, 40))).collect();
    let workers: Vec<_> = (0..8)
        .map(|t| {
            let (cache, graphs) = (cache.clone(), graphs.clone());
            std::thread::spawn(move || {
                for round in 0..40 {
                    let graph = &graphs[(t + round) % graphs.len()];
                    let stats = cache.stats_for(graph);
                    assert_eq!(
                        seen(stats.rank("user", 5)),
                        legacy_observable(graph, "user", 5)
                    );
                    assert!(cache.counters().entries <= 3);
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
}

// ---------------------------------------------------------------------------
// The tool
// ---------------------------------------------------------------------------

use cognicode_core::infrastructure::graph::GraphCache;
use cognicode_core::infrastructure::persistence::CachedGraphStore;
use cognicode_core::interface::mcp::handlers::HandlerContext;
use cognicode_core::interface::mcp::handlers::graph_handlers::handle_graph_search_idf;
use cognicode_core::interface::mcp::schemas::GraphSearchIdfInput;

async fn ask(ctx: &HandlerContext, query: &str) -> serde_json::Value {
    handle_graph_search_idf(
        ctx,
        GraphSearchIdfInput {
            query: query.to_string(),
            max_results: 5,
        },
    )
    .await
    .expect("the search tool answers")
}

/// The ids the tool answered, in order.
fn answered(payload: &serde_json::Value) -> Vec<String> {
    payload["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["symbol_id"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test]
async fn the_tool_builds_the_statistics_once_per_snapshot_and_follows_the_snapshot() {
    use cognicode_core::application::ports::SharedGraph;

    let cache = Arc::new(GraphCache::new());
    cache.replace(fixture_graph());
    let ctx = HandlerContext::builder()
        .with_graph_store(CachedGraphStore::new(cache.clone()))
        .build();

    let first = ask(&ctx, "get_user").await;
    for _ in 0..5 {
        assert_eq!(ask(&ctx, "get_user").await, first);
    }
    let _ = ask(&ctx, "BUILD").await;
    let counters = ctx.ranking_cache_counters();
    assert_eq!(counters.builds, 1, "seven calls, one snapshot, one build");
    assert_eq!(counters.hits, 6);
    assert_eq!(
        answered(&first),
        GOLDEN_TOP5
            .iter()
            .find(|(q, _)| *q == "get_user")
            .unwrap()
            .1
            .iter()
            .map(|(id, _, _)| id.to_string())
            .collect::<Vec<_>>(),
        "the tool answers what the ranking did before the statistics were reused"
    );

    // A new snapshot is published: the next call ranks it, not the old one.
    let mut next = fixture_graph();
    next.add_symbol(Symbol::new(
        "get_user_v2",
        SymbolKind::Function,
        Location::new("z.rs", 1, 1),
    ));
    cache.replace(next);
    let after = ask(&ctx, "get_user_v2").await;
    assert_eq!(answered(&after)[0], "z.rs:get_user_v2:1");
    assert!(!answered(&first).contains(&"z.rs:get_user_v2:1".to_string()));
    assert_eq!(ctx.ranking_cache_counters().builds, 2);
}
