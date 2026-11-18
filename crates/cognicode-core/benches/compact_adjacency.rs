//! Measurement harness for the array adjacency of a projection.
//!
//! Not a criterion bench: a handful of timed runs per row, reported as
//! p50/p95 with the spread and the host load, is more honest at these sizes
//! than a statistical model. A counting global allocator reports the
//! allocations, the bytes requested and the bytes left live by each row.
//!
//! Run: `cargo bench -p cognicode-core --bench compact_adjacency -- 10000 100000 1000000`
//! (sizes are node counts; default `10000 100000`). Environment:
//! `ADJ_RUNS` timed runs per row (default 9); `ADJ_PROJECTION_MAX` largest
//! size at which the rows over a real projection are run (default 100000:
//! a projection clones every symbol, and at 1M that does not fit in memory
//! on the 15 GB host next to the aggregate it was projected from).
//!
//! Two levels. `structure` rows compare the petgraph `StableGraph` the
//! projection owns with the array adjacency, over the same synthetic edge
//! list, at every size; node weights are `u32` there, so the retained bytes
//! of the petgraph side are a floor (the projection stores a `SymbolId` per
//! node). `projection` rows go through `CallGraphProjection` itself and
//! compare the public method against the petgraph walk it replaced, which
//! is kept here verbatim as the reference (`baseline_*`).
//!
//! Caveats: the counting allocator costs a few nanoseconds per allocation,
//! the same on both sides of every comparison; numbers are for the bench
//! profile, never for debug builds; the host is shared, so read the spread
//! column before trusting a difference.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::{HashSet, VecDeque};
use std::hint::black_box;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::time::Instant;

use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
use cognicode_core::domain::ports::{CallGraphProjectionPort, CompactAdjacency, EdgeSide};
use cognicode_core::domain::value_objects::{DependencyType, Location, SymbolKind};
use cognicode_core::infrastructure::graph::CallGraphProjection;
use cognicode_core::infrastructure::graph::projection_cache::ProjectionCache;
use cognicode_graph_algos::page_rank;
use petgraph::Direction;
use petgraph::algo::has_path_connecting;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableGraph;
use petgraph::visit::{EdgeRef, NodeIndexable};

// ---------------------------------------------------------------------------
// Counting allocator.
// ---------------------------------------------------------------------------

struct Counting;

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static REQUESTED: AtomicU64 = AtomicU64::new(0);
static LIVE: AtomicI64 = AtomicI64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        REQUESTED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        LIVE.fetch_add(layout.size() as i64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as i64, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        REQUESTED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        LIVE.fetch_add(layout.size() as i64, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        REQUESTED.fetch_add(new_size as u64, Ordering::Relaxed);
        LIVE.fetch_add(new_size as i64 - layout.size() as i64, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// What one call allocated: the number of allocations, the bytes requested
/// and the bytes still live when it returned (`retained`).
#[derive(Clone, Copy, Default)]
struct Alloc {
    count: u64,
    requested: u64,
    retained: i64,
}

fn measured<T>(f: impl FnOnce() -> T) -> (T, Alloc) {
    let (c0, r0, l0) = (
        ALLOCS.load(Ordering::Relaxed),
        REQUESTED.load(Ordering::Relaxed),
        LIVE.load(Ordering::Relaxed),
    );
    let out = f();
    let alloc = Alloc {
        count: ALLOCS.load(Ordering::Relaxed) - c0,
        requested: REQUESTED.load(Ordering::Relaxed) - r0,
        retained: LIVE.load(Ordering::Relaxed) - l0,
    };
    (out, alloc)
}

// ---------------------------------------------------------------------------
// Synthetic graph.
// ---------------------------------------------------------------------------

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
    /// Skewed index in `0..n`: small values are far more likely, so a few
    /// hubs exist.
    fn skewed(&mut self, n: usize) -> usize {
        let a = self.below(n);
        let b = self.below(n);
        (a * b) / n
    }
}

const KINDS: [DependencyType; 4] = [
    DependencyType::Calls,
    DependencyType::Imports,
    DependencyType::References,
    DependencyType::Contains,
];

/// Three edges per node on average, skewed targets, a tenth of them of a
/// kind other than `Calls`; sorted and deduplicated the way the projection
/// inserts them.
fn edge_list(n: usize) -> Vec<(usize, usize, DependencyType)> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut edges = Vec::with_capacity(n * 3);
    for source in 0..n {
        for _ in 0..3 {
            let target = rng.skewed(n);
            let kind = if rng.below(10) == 0 {
                KINDS[1 + rng.below(3)]
            } else {
                DependencyType::Calls
            };
            edges.push((source, target, kind));
        }
    }
    edges.sort_unstable();
    edges.dedup();
    edges
}

fn raw_graph(
    n: usize,
    edges: &[(usize, usize, DependencyType)],
) -> StableGraph<u32, (DependencyType, f64)> {
    let mut graph = StableGraph::with_capacity(n, edges.len());
    for node in 0..n {
        graph.add_node(node as u32);
    }
    for &(source, target, kind) in edges {
        graph.add_edge(NodeIndex::new(source), NodeIndex::new(target), (kind, 1.0));
    }
    graph
}

fn compact(n: usize, edges: &[(usize, usize, DependencyType)]) -> CompactAdjacency {
    CompactAdjacency::build(
        n,
        edges.len(),
        edges.iter().map(|&(s, t, k)| (s, t, k, 1.0)),
    )
}

fn call_graph(n: usize) -> CallGraph {
    let mut graph = CallGraph::new();
    let ids: Vec<SymbolId> = (0..n)
        .map(|i| {
            graph.add_symbol(Symbol::new(
                format!("fn_{i}"),
                SymbolKind::Function,
                Location::new(
                    format!("src/mod{}/file{}.rs", i / 500, i / 50),
                    (i % 5000) as u32 + 1,
                    1,
                ),
            ))
        })
        .collect();
    for (source, target, kind) in edge_list(n) {
        let _ = graph.add_dependency(&ids[source], &ids[target], kind);
    }
    graph
}

// ---------------------------------------------------------------------------
// The petgraph walks the projection used before the array adjacency.
// ---------------------------------------------------------------------------

fn baseline_in_lists<N>(
    graph: &StableGraph<N, (DependencyType, f64)>,
) -> (Vec<Vec<usize>>, Vec<usize>) {
    let bound = graph.node_bound();
    let mut in_neighbors = vec![Vec::new(); bound];
    let mut out_degree = vec![0; bound];
    for ni in graph.node_indices() {
        in_neighbors[ni.index()] = graph
            .edges_directed(ni, Direction::Incoming)
            .map(|e| e.source().index())
            .collect();
        out_degree[ni.index()] = graph.edges_directed(ni, Direction::Outgoing).count();
    }
    (in_neighbors, out_degree)
}

fn baseline_impact<N>(
    graph: &StableGraph<N, (DependencyType, f64)>,
    root: usize,
    max_depth: usize,
) -> Vec<usize> {
    if max_depth == 0 {
        return Vec::new();
    }
    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut result = Vec::new();
    let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::from([(NodeIndex::new(root), 0)]);
    while let Some((ni, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for edge in graph.edges_directed(ni, Direction::Incoming) {
            let pred = edge.source();
            if visited.insert(pred) {
                result.push(pred.index());
                queue.push_back((pred, depth + 1));
            }
        }
    }
    result
}

fn baseline_impact_ids(
    graph: &StableGraph<SymbolId, (DependencyType, f64)>,
    root: usize,
    max_depth: usize,
) -> Vec<SymbolId> {
    baseline_impact(graph, root, max_depth)
        .into_iter()
        .map(|n| graph[NodeIndex::new(n)].clone())
        .collect()
}

// ---------------------------------------------------------------------------
// Reporting.
// ---------------------------------------------------------------------------

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

fn mib(bytes: i64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn print_row(label: &str, s: &Summary, a: Alloc) {
    println!(
        "  {label:<44} p50 {:>10.3} ms  p95 {:>10.3}  spread {:>5.1}%  allocs {:>9}  req {:>9.2} MiB  live {:>8.2} MiB",
        s.p50,
        s.p95,
        (s.max - s.min) / s.p50.max(f64::EPSILON) * 100.0,
        a.count,
        mib(a.requested as i64),
        mib(a.retained),
    );
}

/// Times `runs` calls of `f`; the allocation columns are those of the last
/// call, which is the steady state.
fn bench<T>(label: &str, runs: usize, mut f: impl FnMut() -> T) {
    black_box(f()); // warm up caches and the allocator
    let mut times = Vec::with_capacity(runs);
    let mut last = Alloc::default();
    for _ in 0..runs {
        let started = Instant::now();
        let (out, alloc) = measured(&mut f);
        times.push(started.elapsed().as_secs_f64() * 1e3);
        black_box(&out);
        // Drop the result outside the measured window, then correct the
        // retained figure: a returned value is live while measured.
        last = alloc;
        drop(out);
    }
    print_row(label, &summarize(times), last);
}

/// Times `ops` calls of `f` over a rotating set of arguments, one sample per
/// call, so the p95 is the tail of single queries.
fn bench_each<A: Copy, T>(label: &str, args: &[A], rounds: usize, mut f: impl FnMut(A) -> T) {
    for &a in args {
        black_box(f(a));
    }
    let mut times = Vec::with_capacity(args.len() * rounds);
    let (_, alloc) = measured(|| {
        for _ in 0..rounds {
            for &a in args {
                let started = Instant::now();
                black_box(f(a));
                times.push(started.elapsed().as_secs_f64() * 1e3);
            }
        }
    });
    let calls = (args.len() * rounds) as u64;
    let per_call = Alloc {
        count: alloc.count / calls,
        requested: alloc.requested / calls,
        retained: 0,
    };
    print_row(label, &summarize(times), per_call);
}

fn concurrent<F>(label: &str, threads: usize, per_thread: usize, runs: usize, roots: &[usize], f: F)
where
    F: Fn(usize) -> usize + Sync,
{
    let mut walls = Vec::new();
    for _ in 0..runs {
        let started = Instant::now();
        std::thread::scope(|scope| {
            for t in 0..threads {
                let f = &f;
                scope.spawn(move || {
                    let mut sink = 0;
                    for i in 0..per_thread {
                        sink += f(roots[(t * 7 + i) % roots.len()]);
                    }
                    black_box(sink)
                });
            }
        });
        walls.push(started.elapsed().as_secs_f64() * 1e3);
    }
    let s = summarize(walls);
    println!(
        "  {label:<44} wall p50 {:>10.3} ms  p95 {:>10.3}  spread {:>5.1}%  {:>9.0} queries/s",
        s.p50,
        s.p95,
        (s.max - s.min) / s.p50 * 100.0,
        (threads * per_thread) as f64 / (s.p50 / 1e3)
    );
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
    let env = |name: &str, default: usize| {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let runs = env("ADJ_RUNS", 9);
    let projection_max = env("ADJ_PROJECTION_MAX", 100_000);

    for n in sizes {
        println!(
            "\n== {n} nodes, {runs} runs per row, loadavg {} ==",
            loadavg()
        );
        let edges = edge_list(n);
        println!(
            "  edges {} ({:.2} per node)",
            edges.len(),
            edges.len() as f64 / n as f64
        );
        let mut rng = Rng(0xDEAD_BEEF_1234_5678);
        let roots: Vec<usize> = (0..64).map(|_| rng.skewed(n)).collect();
        let pairs: Vec<(usize, usize)> = (0..32).map(|_| (rng.below(n), rng.below(n))).collect();

        println!(" structure: petgraph StableGraph vs array adjacency (same edges)");
        let rss0 = status_kb("VmRSS:");
        let (graph, g_alloc) = measured(|| raw_graph(n, &edges));
        println!(
            "  petgraph retained {:.2} MiB live ({} allocs); RSS delta {} MiB",
            mib(g_alloc.retained),
            g_alloc.count,
            status_kb("VmRSS:").saturating_sub(rss0) / 1024
        );
        let (adjacency, c_alloc) = measured(|| compact(n, &edges));
        println!(
            "  array adjacency retained {:.2} MiB live ({} allocs); heap_bytes {:.2} MiB; ratio to petgraph {:.2}",
            mib(c_alloc.retained),
            c_alloc.count,
            mib(adjacency.heap_bytes() as i64),
            c_alloc.retained as f64 / g_alloc.retained as f64
        );
        bench("build petgraph (from sorted edges)", runs, || {
            raw_graph(n, &edges)
        });
        bench("build array adjacency (once per projection)", runs, || {
            compact(n, &edges)
        });

        println!(" per-query adjacency lists (what an analytics call builds)");
        bench("lists: petgraph in_lists+out_degree", runs, || {
            baseline_in_lists(&graph)
        });
        bench("lists: array adjacency in_lists (owned)", runs, || {
            adjacency.in_lists()
        });
        bench("rows: array adjacency borrowed, summed", runs, || {
            (0..n)
                .map(|u| adjacency.predecessors(u).len() + adjacency.successors(u).len())
                .sum::<usize>()
        });
        let iterations = 20;
        bench("page_rank end to end: petgraph lists", runs.min(5), || {
            let (lists, degree) = baseline_in_lists(&graph);
            page_rank(&lists, &degree, n, 0.85, iterations)
        });
        bench(
            "page_rank end to end: array lists (shared)",
            runs.min(5),
            || {
                let (lists, degree) = adjacency.in_lists();
                page_rank(&lists, &degree, n, 0.85, iterations)
            },
        );
        bench(
            "page_rank end to end: array lists + build",
            runs.min(5),
            || {
                let adjacency = compact(n, &edges);
                let (lists, degree) = adjacency.in_lists();
                page_rank(&lists, &degree, n, 0.85, iterations)
            },
        );

        println!(" per-query walks (one sample per query over 64 skewed roots)");
        for (label, depth) in [("depth 3", 3usize), ("unbounded", usize::MAX)] {
            let rounds = if depth == usize::MAX && n >= 1_000_000 {
                1
            } else {
                4
            };
            bench_each(
                &format!("impact {label}: petgraph + HashSet"),
                &roots,
                rounds,
                |r| baseline_impact(&graph, r, depth).len(),
            );
            bench_each(
                &format!("impact {label}: array adjacency"),
                &roots,
                rounds,
                |r| adjacency.reach(r, EdgeSide::Incoming, depth, true).len(),
            );
        }
        bench_each("has_path: petgraph", &pairs, 2, |(a, b)| {
            has_path_connecting(&graph, NodeIndex::new(a), NodeIndex::new(b), None)
        });
        bench_each("has_path: array adjacency", &pairs, 2, |(a, b)| {
            adjacency.connects(a, b)
        });

        println!(" concurrent readers over one shared structure (depth 3)");
        for threads in [4usize, 8] {
            concurrent(
                &format!("x{threads} petgraph"),
                threads,
                200,
                runs,
                &roots,
                |r| baseline_impact(&graph, r, 3).len(),
            );
            concurrent(
                &format!("x{threads} array adjacency"),
                threads,
                200,
                runs,
                &roots,
                |r| adjacency.reach(r, EdgeSide::Incoming, 3, true).len(),
            );
        }
        drop((graph, adjacency));

        if n > projection_max {
            println!(" projection rows skipped above {projection_max} nodes: memory");
            continue;
        }

        println!(
            " projection: CallGraphProjection, public method vs the petgraph walk it replaced"
        );
        let source = Arc::new(call_graph(n));
        let (projection, p_alloc) = measured(|| CallGraphProjection::from_call_graph(&source));
        println!(
            "  projection retained {:.2} MiB live ({} allocs); symbols {} edges {}",
            mib(p_alloc.retained),
            p_alloc.count,
            source.symbol_count(),
            source.edge_count()
        );
        bench("from_call_graph (every call, today)", runs, || {
            CallGraphProjection::from_call_graph(&source)
        });
        let (_, first) = measured(|| projection.compact_adjacency().edge_count());
        println!(
            "  first compact_adjacency(): {:.2} MiB live ({} allocs)",
            mib(first.retained),
            first.count
        );
        bench("fresh projection + compact_adjacency()", runs, || {
            let p = CallGraphProjection::from_call_graph(&source);
            p.compact_adjacency().edge_count()
        });
        let ids: Vec<SymbolId> = roots
            .iter()
            .map(|&r| projection.graph()[NodeIndex::new(r)].clone())
            .collect();
        let index: Vec<usize> = (0..ids.len()).collect();
        bench_each("warm impact depth 3: petgraph walk", &index, 4, |i| {
            baseline_impact_ids(projection.graph(), roots[i], 3).len()
        });
        bench_each("warm impact depth 3: find_impact_radius", &index, 4, |i| {
            projection.find_impact_radius(&ids[i], 3).len()
        });
        bench("one shot impact: fresh projection, petgraph", runs, || {
            let p = CallGraphProjection::from_call_graph(&source);
            baseline_impact_ids(p.graph(), roots[0], 3).len()
        });
        bench("one shot impact: fresh projection, array", runs, || {
            let p = CallGraphProjection::from_call_graph(&source);
            p.find_impact_radius(&ids[0], 3).len()
        });
        // One analytics call: the projection it runs over, then the lists it
        // reads (what `page_rank` asks the projection for).
        bench(
            "analytics call before: projection + petgraph lists",
            runs,
            || {
                let p = CallGraphProjection::from_call_graph(&source);
                baseline_in_lists(p.graph())
            },
        );
        bench(
            "analytics call, cold snapshot: projection + array lists",
            runs,
            || {
                let p = CallGraphProjection::from_call_graph(&source);
                CallGraphProjectionPort::build_adjacency(&p)
            },
        );
        let cache = ProjectionCache::default();
        let _ = cache.projection_for(&source);
        bench(
            "analytics call, warm snapshot: cached projection + array lists",
            runs,
            || cache.projection_for(&source).build_adjacency(),
        );
        bench(
            "analytics call, warm snapshot: cached projection, impact depth 3",
            runs,
            || {
                cache
                    .projection_for(&source)
                    .find_impact_radius(&ids[0], 3)
                    .len()
            },
        );
        bench("warm build_adjacency: petgraph lists", runs, || {
            baseline_in_lists(projection.graph())
        });
        bench("warm build_adjacency: array lists", runs, || {
            CallGraphProjectionPort::build_adjacency(&projection)
        });
    }
}
