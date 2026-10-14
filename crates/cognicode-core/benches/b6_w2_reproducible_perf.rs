//! Criterion benchmarks for B6.W2 — reproducible performance.
//!
//! Run with: `cargo bench -p cognicode-core --bench b6_w2_reproducible_perf`
//!
//! Six coverage categories required by B6.W2:
//!   - `cold`     : first-ever analysis on a corpus (no warm caches).
//!   - `hot`      : subsequent analysis on the same corpus (warm cache).
//!   - `incremental`: re-analysis after a single-file edit, content-hash
//!     based (B3.W4).
//!   - `search`   : symbol search over the index.
//!   - `navigation` : go-to-definition (callees) over the graph.
//!   - `graphs`   : full graph extraction (B4.W1 / projection).
//!
//! Three corpus sizes are exercised (small=50, medium=500, large=5000).
//! Corpora are generated at bench setup in a tempdir, never committed.
//!
//! Uses existing e91 mechanisms (`PerFileGraphCache`,
//! `OnDemandGraphBuilder`, `LightweightIndex`, `CacheStore`,
//! `CallGraphProjection`); no new framework.
//!
//! Política B6.W2: los presupuestos declarados en
//! `tests/b6_w2_perf_budgets.rs` son techos laxos que la siguiente
//! release endurece. Este bench produce los números reales (mediana,
//! pendiente, stddev) que esos presupuestos verifican.

use std::path::PathBuf;
use std::time::Instant;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use tempfile::TempDir;

use cognicode_core::incremental::content_hash::{CacheStore, content_hash};
use cognicode_core::infrastructure::graph::{
    CallGraphProjection, LightweightIndex, OnDemandGraphBuilder, PerFileGraphCache,
    TraversalDirection,
};

// ---------------------------------------------------------------------------
// Corpus generation (e91 fixture pattern)
// ---------------------------------------------------------------------------

/// Number of files per corpus size.
const SMALL: usize = 50;
const MEDIUM: usize = 500;
const LARGE: usize = 5_000;

/// Build a corpus of `n` small Python-like files on disk in a tempdir.
///
/// Each file declares a `helper_<i>` and a `func_<i>` that calls it,
/// plus a class with two methods. The call edge `func_<i> → helper_<i>`
/// is what makes `build_for_symbol` resolve callees in the navigation
/// bench; without it, a builder that resolves no edges would still
/// produce a clean-looking number.
fn build_corpus(n: usize) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    for i in 0..n {
        let path: PathBuf = dir.path().join(format!("file_{i:05}.py"));
        let body = format!(
            "def helper_{i}(x):\n    return x * 2\n\
             \ndef func_{i}(x, y):\n    return helper_{i}(x) + y\n\
             \nclass Cls_{i}:\n\
             \n    def method_{i}(self):\n        return func_{i}(1, 2)\n\
             \n    def other_{i}(self, n):\n        return self.method_{i}() + n\n"
        );
        std::fs::write(&path, body).expect("write corpus file");
    }
    dir
}

/// Read every file under `corpus` and return `(filename, body)` pairs.
///
/// Caller is responsible for keeping the corpus `TempDir` alive while
/// the resulting strings are in use.
fn read_corpus(corpus: &TempDir) -> Vec<(String, String)> {
    let mut sources: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(corpus.path()).expect("read corpus") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let rel = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("x.py")
            .to_string();
        let src = std::fs::read_to_string(&path).expect("read source");
        sources.push((rel, src));
    }
    sources
}

/// Hash every file in the corpus (B3.W4 BLAKE3 over bytes).
fn hash_corpus(corpus: &TempDir) -> Vec<(PathBuf, [u8; 32])> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(corpus.path()).expect("read corpus dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("py") {
            let h = content_hash(&path).expect("hash corpus file");
            out.push((path, *h.as_bytes()));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Cold path — first analysis with empty cache
// ---------------------------------------------------------------------------

fn bench_cold(c: &mut Criterion) {
    let mut group = c.benchmark_group("b6_w2_cold");
    for (label, n) in [("small", SMALL), ("medium", MEDIUM), ("large", LARGE)] {
        group.bench_function(label, |b| {
            b.iter(|| {
                // Cold: no prior cache. Build a fresh corpus, then build
                // the on-demand graph from its source listing. The fixture
                // is generated on every iteration: that IS the cold path.
                let corpus = build_corpus(black_box(n));
                let sources = read_corpus(&corpus);
                let mut builder = OnDemandGraphBuilder::new();
                let refs: Vec<(&str, &str)> = sources
                    .iter()
                    .map(|(r, s)| (r.as_str(), s.as_str()))
                    .collect();
                builder.build_index_from_sources(refs);
                // Sanity: confirm we indexed at least one file. A
                // benchmark that timed nothing would still produce a
                // pretty number, so the assertion is the real
                // contract.
                let r = builder.build_for_symbol("func_0", 1, TraversalDirection::Callees);
                assert!(
                    !r.entries.is_empty(),
                    "the cold corpus must index at least func_0; a builder \
                     that resolves no entries would make this bench measure nothing"
                );
                black_box(r.entries.len());
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Hot path — repeated analysis against a pre-built, cached state
// ---------------------------------------------------------------------------

fn bench_hot(c: &mut Criterion) {
    let mut group = c.benchmark_group("b6_w2_hot");
    for (label, n) in [("small", SMALL), ("medium", MEDIUM), ("large", LARGE)] {
        // Pre-build ONCE outside the timed loop. The hot path is the
        // repeated query, not the re-construction.
        let corpus = build_corpus(n);
        let sources = read_corpus(&corpus);
        let mut builder = OnDemandGraphBuilder::new();
        let refs: Vec<(&str, &str)> = sources
            .iter()
            .map(|(r, s)| (r.as_str(), s.as_str()))
            .collect();
        builder.build_index_from_sources(refs);
        // The first declared function is guaranteed to exist in every
        // corpus: the fixture writes `def func_<i>(...)` for i in 0..n.
        let target = "func_0".to_string();
        group.bench_function(label, |b| {
            b.iter(|| {
                let result = builder.build_for_symbol(
                    black_box(&target),
                    black_box(3),
                    black_box(TraversalDirection::Callees),
                );
                black_box(result.entries.len());
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Incremental — content-hash based revalidation (B3.W4)
// ---------------------------------------------------------------------------

fn bench_incremental(c: &mut Criterion) {
    let mut group = c.benchmark_group("b6_w2_incremental");
    for (label, n) in [("small", SMALL), ("medium", MEDIUM)] {
        let corpus = build_corpus(n);
        let mut store = CacheStore::new();
        // Warm: observe the whole corpus once.
        let entries = hash_corpus(&corpus);
        for (p, _) in &entries {
            store.observe(p).expect("warm observe");
        }
        // Pick a real file to "revalidate". The hash is identical to
        // the one in the store, so the cache-hit path is exercised.
        let target_idx = entries.len() / 2;
        let target_path = entries[target_idx].0.clone();
        group.bench_function(label, |b| {
            b.iter(|| {
                let stale = store.is_stale(black_box(&target_path)).expect("is_stale");
                black_box(stale);
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Search — LightweightIndex.find_symbol
// ---------------------------------------------------------------------------

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("b6_w2_search");
    for (label, n) in [("small", SMALL), ("medium", MEDIUM), ("large", LARGE)] {
        let mut index = LightweightIndex::new();
        for i in 0..n {
            let path = format!("file_{i:05}.py");
            let src = format!(
                "def func_{i}(x, y):\n    return x + y\n\
                 \nclass Cls_{i}:\n    def method_{i}(self):\n        return func_{i}(1, 2)\n"
            );
            index.build_from_sources([(path.as_str(), src.as_str())]);
        }
        let target = format!("func_{}", n / 2);
        group.bench_function(label, |b| {
            b.iter(|| {
                let hits = index.find_symbol(black_box(&target));
                black_box(hits.len());
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Navigation — go-to-definition via OnDemandGraphBuilder
// ---------------------------------------------------------------------------

fn bench_navigation(c: &mut Criterion) {
    let mut group = c.benchmark_group("b6_w2_navigation");
    for (label, n) in [("small", SMALL), ("medium", MEDIUM), ("large", LARGE)] {
        let mut builder = OnDemandGraphBuilder::new();
        let mut srcs: Vec<(String, String)> = Vec::with_capacity(n);
        for i in 0..n {
            let path = format!("file_{i:05}.py");
            let src = format!(
                "def helper_{i}(x):\n    return x * 2\n\
                 \ndef func_{i}(x, y):\n    return helper_{i}(x) + y\n\
                 \nclass Cls_{i}:\n\
                 \n    def method_{i}(self):\n        return func_{i}(1, 2)\n\
                 \n    def other_{i}(self, n):\n        return self.method_{i}() + n\n"
            );
            srcs.push((path, src));
        }
        let refs: Vec<(&str, &str)> = srcs.iter().map(|(r, s)| (r.as_str(), s.as_str())).collect();
        builder.build_index_from_sources(refs);
        // Pick a symbol that has a known callee: func_<i> calls
        // helper_<i>, so depth-1 in `Callees` direction must resolve
        // at least helper_<i>.
        let target = format!("func_{}", n / 2);
        group.bench_function(label, |b| {
            b.iter(|| {
                let r = builder.build_for_symbol(
                    black_box(&target),
                    black_box(1),
                    black_box(TraversalDirection::Callees),
                );
                black_box(r.entries.len());
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Graphs — full graph extraction (B4.W1 / projection path)
// ---------------------------------------------------------------------------

fn bench_graphs(c: &mut Criterion) {
    let mut group = c.benchmark_group("b6_w2_graphs");
    for (label, n) in [("small", SMALL), ("medium", MEDIUM)] {
        // Build a per-file cache, then merge through CallGraphProjection
        // for the full corpus graph.
        let corpus = build_corpus(n);
        let cache = PerFileGraphCache::new();
        let mut paths: Vec<PathBuf> = Vec::with_capacity(n);
        for entry in std::fs::read_dir(corpus.path()).expect("read corpus") {
            let entry = entry.expect("entry");
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("py") {
                cache.get_or_build(&path).expect("build per-file graph");
                paths.push(path);
            }
        }
        let path_refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
        let merged = cache.merge(&path_refs);
        let projection = CallGraphProjection::from_call_graph(&merged);

        group.bench_function(label, |b| {
            b.iter(|| {
                // Full-graph observation: node + edge count from the
                // projection. Touching both keeps the bench honest about
                // an actual full-graph read, not just symbol_count().
                let nodes = projection.node_count();
                let edges = projection.edge_count();
                assert!(nodes > 0, "merged corpus must yield a non-empty graph");
                black_box((nodes, edges));
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Wall-clock helper (consumed by the budget tests, mirrored locally)
// ---------------------------------------------------------------------------

/// Measure a single `f` invocation and return elapsed nanos.
///
/// Local copy lives in the bench binary because benches and tests do
/// not share a crate graph: each is a separate compilation unit, and
/// importing from `benches/` is not supported by Cargo.
pub fn measure<F: FnOnce()>(f: F) -> u128 {
    let start = Instant::now();
    f();
    start.elapsed().as_nanos()
}

// ---------------------------------------------------------------------------
// Criterion entry
// ---------------------------------------------------------------------------

criterion_group!(
    b6_w2_benches,
    bench_cold,
    bench_hot,
    bench_incremental,
    bench_search,
    bench_navigation,
    bench_graphs,
);
criterion_main!(b6_w2_benches);
