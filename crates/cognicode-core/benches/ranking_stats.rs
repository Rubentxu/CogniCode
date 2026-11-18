//! Measurement harness for `graph_search_idf` ranking cost at corpus scale.
//!
//! Not a criterion bench: a ranking over 100k+ symbols takes long enough per
//! call that a handful of timed runs, reported as p50/p95 together with the
//! spread and the host load, is more honest than a statistical model.
//!
//! Run: `cargo bench -p cognicode-core --bench ranking_stats -- 10000 100000`
//! (sizes are symbol counts; default `10000 100000`). Set `RANKING_RUNS` to
//! change the number of timed runs per row (default 9) and
//! `RANKING_LEGACY_MAX` to change the largest size at which the pre-change
//! reference is also run (default 100000: it clones every symbol into a
//! projection, which does not fit in memory at 1M on the 15 GB host).
//!
//! Rows: `legacy` is the ranking as it was before the statistics were
//! reused (IDF, projection and degrees rebuilt per call), kept here as the
//! reference; `fresh` is `SearchRanker::search` now (build + rank per call);
//! `build` is the cost of constructing the statistics once; `warm` ranks
//! from statistics that already exist; `cold` is a first query on an empty
//! cache (lookup + build + rank); `concurrent` is N threads asking the same
//! warm cache at once.
//!
//! The corpus is synthetic and deterministic (fixed seed): symbol names are
//! drawn from a skewed vocabulary so token document frequencies are
//! realistic, and call targets are skewed so a few hubs exist.

use std::collections::{HashMap, HashSet};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::CallGraphProjection;
use cognicode_core::infrastructure::graph::analytics::ranking_stats::{
    RankingStats, RankingStatsCache,
};
use cognicode_core::infrastructure::graph::analytics::search_ranker::SearchRanker;
use petgraph::Direction;

const VOCAB: usize = 3000;
const QUERIES: &[&str] = &[
    "handle_request",
    "user",
    "parseConfig",
    "word42",
    "zzz_none",
];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    /// Skewed index in `0..n`: small values are far more likely.
    fn skewed(&mut self, n: usize) -> usize {
        let a = self.below(n);
        let b = self.below(n);
        (a * b) / n
    }
}

fn word(i: usize) -> String {
    match i {
        0 => "handle".into(),
        1 => "request".into(),
        2 => "user".into(),
        3 => "parse".into(),
        4 => "config".into(),
        _ => format!("word{i}"),
    }
}

fn build_graph(n: usize) -> CallGraph {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut graph = CallGraph::new();
    let mut ids: Vec<SymbolId> = Vec::with_capacity(n);
    for i in 0..n {
        let parts = 1 + rng.below(3);
        let tokens: Vec<String> = (0..parts).map(|_| word(rng.skewed(VOCAB))).collect();
        let name = if i % 3 == 0 {
            tokens.join("_")
        } else {
            let mut s = tokens[0].clone();
            for t in &tokens[1..] {
                s.push_str(&t[..1].to_uppercase());
                s.push_str(&t[1..]);
            }
            s
        };
        let file = format!("src/mod{}/file{}.rs", i / 500, i / 50);
        ids.push(graph.add_symbol(Symbol::new(
            &name,
            SymbolKind::Function,
            Location::new(&file, (i % 5000) as u32 + 1, 1),
        )));
    }
    for i in 0..n {
        for _ in 0..3 {
            let t = rng.skewed(n);
            if t != i {
                let _ = graph.add_dependency(&ids[i], &ids[t], DependencyType::Calls);
            }
        }
    }
    graph
}

/// The ranking as it was before the statistics were reused: the reference
/// the rows labelled `legacy` measure. Returns the number of hits so the
/// work cannot be optimised away.
fn legacy_search(graph: &CallGraph, query: &str, k: usize) -> usize {
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
    let degrees: Vec<usize> = g
        .node_indices()
        .map(|ni| {
            g.edges_directed(ni, Direction::Outgoing).count()
                + g.edges_directed(ni, Direction::Incoming).count()
        })
        .collect();
    let mut sorted = degrees.clone();
    sorted.sort();
    let p95 = sorted[((sorted.len() as f64 * 0.95) as usize).min(sorted.len() - 1)];
    let query_lower = query.to_lowercase();
    let query_tokens = SearchRanker::tokenize(query);
    let mut scored: Vec<(f64, SymbolId, String, String, usize)> = Vec::new();
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
        let degree = degrees[ni.index()];
        if degree > p95 {
            score *= 0.1;
        }
        scored.push((
            score,
            id.clone(),
            name,
            symbol.location().file().to_string(),
            degree,
        ));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);
    scored.len()
}

fn status_kb(field: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(field))
                .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

fn loadavg() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_default()
}

struct Summary {
    p50: f64,
    p95: f64,
    min: f64,
    max: f64,
}

fn summarize(mut ms: Vec<f64>) -> Summary {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |q: f64| ms[((ms.len() as f64 * q).ceil() as usize).clamp(1, ms.len()) - 1];
    Summary {
        p50: at(0.50),
        p95: at(0.95),
        min: ms[0],
        max: ms[ms.len() - 1],
    }
}

fn row(label: &str, s: &Summary) {
    println!(
        "  {label:<38} p50 {:>10.3} ms  p95 {:>10.3} ms  min {:>10.3}  max {:>10.3}  spread {:>5.1}%",
        s.p50,
        s.p95,
        s.min,
        s.max,
        (s.max - s.min) / s.p50 * 100.0
    );
}

fn time_ms<T>(mut f: impl FnMut() -> T) -> f64 {
    let t = Instant::now();
    black_box(f());
    t.elapsed().as_secs_f64() * 1e3
}

fn main() {
    let sizes: Vec<usize> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .filter_map(|a| a.parse().ok())
        .collect();
    let sizes = if sizes.is_empty() {
        vec![10_000, 100_000]
    } else {
        sizes
    };
    let runs: usize = std::env::var("RANKING_RUNS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9);

    for n in sizes {
        println!("\n== {n} symbols, {runs} runs per row ==");
        println!("  loadavg at start: {}", loadavg());
        let rss0 = status_kb("VmRSS:");
        let graph = build_graph(n);
        let rss_graph = status_kb("VmRSS:");
        println!(
            "  symbols {} edges {}  RSS graph {} MiB (delta {} MiB)",
            graph.symbol_count(),
            graph.edge_count(),
            rss_graph / 1024,
            (rss_graph - rss0) / 1024
        );

        // What the statistics hold, measured first while the heap is quiet:
        // the first build includes growing the heap, the second is the
        // steady-state cost per snapshot.
        let rss_0 = status_kb("VmRSS:");
        let stats_a = RankingStats::build(&graph);
        let rss_1 = status_kb("VmRSS:");
        let stats_b = RankingStats::build(&graph);
        let rss_2 = status_kb("VmRSS:");
        println!(
            "  stats resident: first {} MiB, second {} MiB ({} B/symbol, RSS deltas); build visits {}",
            rss_1.saturating_sub(rss_0) / 1024,
            rss_2.saturating_sub(rss_1) / 1024,
            rss_2.saturating_sub(rss_1) * 1024 / n as u64,
            stats_a.build_visits()
        );
        drop((stats_a, stats_b));

        let legacy_max: usize = std::env::var("RANKING_LEGACY_MAX")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(100_000);

        if n <= legacy_max {
            let proj = summarize(
                (0..runs)
                    .map(|_| time_ms(|| CallGraphProjection::from_call_graph(&graph)))
                    .collect(),
            );
            row("projection build (legacy part)", &proj);
            for q in QUERIES {
                let s = summarize(
                    (0..runs)
                        .map(|_| time_ms(|| legacy_search(&graph, q, 20)))
                        .collect(),
                );
                row(&format!("legacy {q:?} k=20"), &s);
            }
        } else {
            println!("  (legacy reference skipped above {legacy_max} symbols: memory)");
        }

        // Fresh now: every call pays for the statistics (it is `build` plus a
        // query, so it is not repeated above the legacy limit).
        for q in QUERIES.iter().filter(|_| n <= legacy_max) {
            let s = summarize(
                (0..runs)
                    .map(|_| time_ms(|| SearchRanker::search(&graph, q, 20)))
                    .collect(),
            );
            row(&format!("fresh {q:?} k=20"), &s);
        }

        let build = summarize(
            (0..runs)
                .map(|_| time_ms(|| RankingStats::build(&graph)))
                .collect(),
        );
        row("build (once per snapshot)", &build);

        // Warm: statistics exist, only the query is paid.
        let graph = Arc::new(graph);
        let cache = Arc::new(RankingStatsCache::default());
        let stats = cache.stats_for(&graph);
        for q in QUERIES {
            let s = summarize((0..runs).map(|_| time_ms(|| stats.rank(q, 20))).collect());
            row(&format!("warm {q:?} k=20"), &s);
        }
        let warm_lookup = summarize(
            (0..runs * 20)
                .map(|_| time_ms(|| cache.stats_for(&graph)))
                .collect(),
        );
        row("warm lookup (stats_for)", &warm_lookup);

        // Cold: the first query on an empty cache pays lookup + build + rank.
        for q in ["user", "zzz_none"] {
            let s = summarize(
                (0..runs)
                    .map(|_| {
                        let cold = RankingStatsCache::default();
                        time_ms(|| cold.stats_for(&graph).rank(q, 20))
                    })
                    .collect(),
            );
            row(&format!("cold {q:?} k=20"), &s);
        }

        // Concurrent queries against the same warm snapshot.
        for threads in [4usize, 8] {
            let per_thread = 25;
            let mut latencies: Vec<f64> = Vec::new();
            let mut walls: Vec<f64> = Vec::new();
            for _ in 0..runs {
                let started = Instant::now();
                let batch: Vec<Vec<f64>> = std::thread::scope(|scope| {
                    let workers: Vec<_> = (0..threads)
                        .map(|t| {
                            let (cache, graph) = (&cache, &graph);
                            scope.spawn(move || {
                                (0..per_thread)
                                    .map(|i| {
                                        let q = QUERIES[(t + i) % QUERIES.len()];
                                        time_ms(|| cache.stats_for(graph).rank(q, 20))
                                    })
                                    .collect::<Vec<f64>>()
                            })
                        })
                        .collect();
                    workers.into_iter().map(|w| w.join().unwrap()).collect()
                });
                walls.push(started.elapsed().as_secs_f64() * 1e3);
                latencies.extend(batch.into_iter().flatten());
            }
            row(
                &format!("concurrent x{threads} per call"),
                &summarize(latencies),
            );
            let wall = summarize(walls);
            println!(
                "  concurrent x{threads} batch of {} calls: wall p50 {:.1} ms, {:.0} calls/s",
                threads * per_thread,
                wall.p50,
                (threads * per_thread) as f64 / (wall.p50 / 1e3)
            );
        }
        if n <= legacy_max {
            let threads = 4usize;
            let per_thread = if n <= 10_000 { 6 } else { 2 };
            let mut latencies: Vec<f64> = Vec::new();
            let mut walls: Vec<f64> = Vec::new();
            for _ in 0..runs.min(5) {
                let started = Instant::now();
                let batch: Vec<Vec<f64>> = std::thread::scope(|scope| {
                    let workers: Vec<_> = (0..threads)
                        .map(|t| {
                            let graph = &graph;
                            scope.spawn(move || {
                                (0..per_thread)
                                    .map(|i| {
                                        let q = QUERIES[(t + i) % QUERIES.len()];
                                        time_ms(|| legacy_search(graph, q, 20))
                                    })
                                    .collect::<Vec<f64>>()
                            })
                        })
                        .collect();
                    workers.into_iter().map(|w| w.join().unwrap()).collect()
                });
                walls.push(started.elapsed().as_secs_f64() * 1e3);
                latencies.extend(batch.into_iter().flatten());
            }
            row(
                &format!("legacy concurrent x{threads} per call"),
                &summarize(latencies),
            );
            let wall = summarize(walls);
            println!(
                "  legacy concurrent x{threads} batch of {} calls: wall p50 {:.1} ms, {:.0} calls/s",
                threads * per_thread,
                wall.p50,
                (threads * per_thread) as f64 / (wall.p50 / 1e3)
            );
        }
        let counters = cache.counters();
        println!(
            "  cache: builds {} hits {} evictions {} entries {}",
            counters.builds, counters.hits, counters.evictions, counters.entries
        );
        println!(
            "  RSS after searches {} MiB, peak (VmHWM) {} MiB, loadavg at end: {}",
            status_kb("VmRSS:") / 1024,
            status_kb("VmHWM:") / 1024,
            loadavg()
        );
    }
}
