//! Measurement harness for symbol-name search over the in-memory index.
//!
//! Not a criterion bench: it reports p50/p95 per query class together with
//! the spread across runs, the host load and the live heap held by the
//! index, because on a shared host a single mean says little.
//!
//! Run: `cargo bench -p cognicode-core --bench symbol_search -- 10000 100000 real`
//! Tokens are corpus sizes (symbol counts) or `real`, the declarations found
//! by a line scan of the `.rs` files under `crates/` of this checkout.
//! `SEARCH_RUNS` sets the timed runs per row (default 3) and
//! `SEARCH_QUERIES` the queries per class (default 30).
//!
//! Only the public surface that existed before the candidate index is used
//! for the shared rows, so the same file runs against the previous code and
//! the new code. Rows:
//! - `build`: indexing the whole corpus, one file at a time, plus the heap the
//!   index retains afterwards (counting allocator, live bytes).
//! - `exact`, `prefix`, `contains`, `fuzzy`, `short2`, `miss`, `kind`:
//!   per-call latency of `search` for each query class (limit 20).
//! - `reindex`: replacing one file of 40 symbols (one renamed).
//! - `concurrent4`: four reader threads, queries per second.
//! - `readers+writer`: four readers while one thread keeps reindexing files.
//! - recall rows compare against an independent brute-force oracle written
//!   in this file from the documented semantics.
//!
//! The synthetic corpus is deterministic (fixed seed): names are built from
//! a skewed vocabulary, in snake_case, lowerCamel and PascalCase, so token
//! frequencies and homonyms look like source code rather than uniform noise.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use cognicode_core::domain::aggregates::Symbol;
use cognicode_core::domain::value_objects::{Location, SymbolKind, SymbolSearchQuery};
use cognicode_core::infrastructure::semantic::semantic_search::{MatchType, SearchIndex};

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_add(new_size, Ordering::Relaxed);
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

const LIMIT: usize = 20;
const SYMBOLS_PER_FILE: usize = 40;

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
    fn skewed(&mut self, n: usize) -> usize {
        let a = self.below(n);
        let b = self.below(n);
        a.min(b).min(self.below(n))
    }
}

#[derive(Clone)]
struct Decl {
    name: String,
    kind: SymbolKind,
    file: String,
    line: u32,
}

fn vocabulary(rng: &mut Rng) -> Vec<String> {
    const COMMON: &[&str] = &[
        "get", "set", "new", "handle", "request", "response", "parse", "config", "user", "node",
        "graph", "symbol", "index", "search", "build", "read", "write", "file", "path", "name",
        "value", "result", "error", "context", "service", "query", "filter", "item", "list", "map",
    ];
    const ON: &[&str] = &["b", "c", "d", "f", "g", "h", "k", "l", "m", "n", "p", "r", "s", "t"];
    const VO: &[&str] = &["a", "e", "i", "o", "u", "ai", "ou"];
    let mut words: Vec<String> = COMMON.iter().map(|w| w.to_string()).collect();
    while words.len() < 3000 {
        let mut w = String::new();
        for _ in 0..(2 + rng.below(3)) {
            w.push_str(ON[rng.below(ON.len())]);
            w.push_str(VO[rng.below(VO.len())]);
        }
        words.push(w);
    }
    words
}

fn compose(rng: &mut Rng, words: &[String]) -> String {
    let parts = 1 + rng.below(3);
    let picked: Vec<&String> = (0..parts).map(|_| &words[rng.skewed(words.len())]).collect();
    match rng.below(3) {
        0 => picked.iter().map(|w| w.as_str()).collect::<Vec<_>>().join("_"),
        1 => picked
            .iter()
            .enumerate()
            .map(|(i, w)| if i == 0 { (*w).clone() } else { capitalize(w) })
            .collect(),
        _ => picked.iter().map(|w| capitalize(w)).collect(),
    }
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

const KINDS: &[SymbolKind] = &[
    SymbolKind::Function,
    SymbolKind::Function,
    SymbolKind::Method,
    SymbolKind::Method,
    SymbolKind::Class,
    SymbolKind::Variable,
    SymbolKind::Trait,
    SymbolKind::Constant,
];

fn synthetic(size: usize) -> Vec<Decl> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let words = vocabulary(&mut rng);
    (0..size)
        .map(|i| Decl {
            name: compose(&mut rng, &words),
            kind: KINDS[rng.below(KINDS.len())],
            file: format!("src/m{}/f{}.rs", (i / SYMBOLS_PER_FILE) / 10, i / SYMBOLS_PER_FILE),
            line: (i % SYMBOLS_PER_FILE) as u32 * 5 + 1,
        })
        .collect()
}

fn real_corpus(root: &std::path::Path) -> Vec<Decl> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(root, &mut files);
    files.sort();
    let keywords: &[(&str, SymbolKind)] = &[
        ("fn ", SymbolKind::Function),
        ("struct ", SymbolKind::Class),
        ("enum ", SymbolKind::Class),
        ("trait ", SymbolKind::Trait),
        ("const ", SymbolKind::Constant),
        ("static ", SymbolKind::Variable),
        ("mod ", SymbolKind::Module),
    ];
    let mut out = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let file_name = file.to_string_lossy().into_owned();
        for (n, line) in text.lines().enumerate() {
            let mut rest = line.trim_start();
            for prefix in ["pub(crate) ", "pub ", "async ", "unsafe "] {
                rest = rest.strip_prefix(prefix).unwrap_or(rest);
            }
            for (kw, kind) in keywords {
                if let Some(after) = rest.strip_prefix(kw) {
                    let name: String = after
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        out.push(Decl {
                            name,
                            kind: *kind,
                            file: file_name.clone(),
                            line: n as u32,
                        });
                    }
                    break;
                }
            }
        }
    }
    out
}

fn by_file(decls: &[Decl]) -> Vec<(String, Vec<Decl>)> {
    let mut map: HashMap<&str, Vec<Decl>> = HashMap::new();
    for d in decls {
        map.entry(d.file.as_str()).or_default().push(d.clone());
    }
    let mut files: Vec<(String, Vec<Decl>)> =
        map.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn symbols(decls: &[Decl]) -> Vec<Symbol> {
    decls
        .iter()
        .map(|d| Symbol::new(d.name.clone(), d.kind, Location::new(d.file.clone(), d.line, 0)))
        .collect()
}

#[derive(Clone)]
struct Case {
    class: &'static str,
    query: String,
    kinds: Vec<SymbolKind>,
}

fn query_cases(decls: &[Decl], per_class: usize) -> Vec<Case> {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    let mut cases = Vec::new();
    let pick = |rng: &mut Rng| -> &Decl { &decls[rng.below(decls.len())] };
    for _ in 0..per_class {
        let name = pick(&mut rng).name.to_lowercase();
        cases.push(Case { class: "exact", query: name, kinds: vec![] });

        let name = pick(&mut rng).name.to_lowercase();
        let take = name.chars().count().min(5);
        cases.push(Case {
            class: "prefix",
            query: name.chars().take(take).collect(),
            kinds: vec![],
        });

        let name = pick(&mut rng).name.to_lowercase();
        let chars: Vec<char> = name.chars().collect();
        if chars.len() >= 6 {
            let start = 1 + rng.below(chars.len() - 5);
            let len = (4 + rng.below(3)).min(chars.len() - start);
            cases.push(Case {
                class: "contains",
                query: chars[start..start + len].iter().collect(),
                kinds: vec![],
            });
        }

        let name = pick(&mut rng).name.to_lowercase();
        let chars: Vec<char> = name.chars().collect();
        if chars.len() >= 6 {
            let q: String = chars.iter().step_by(2).collect();
            cases.push(Case { class: "fuzzy", query: q, kinds: vec![] });
        }

        let name = pick(&mut rng).name.to_lowercase();
        cases.push(Case {
            class: "short2",
            query: name.chars().skip(1).take(2).collect(),
            kinds: vec![],
        });
        cases.push(Case {
            class: "miss",
            query: format!("qzx{}wv", rng.below(1000)),
            kinds: vec![],
        });

        let name = pick(&mut rng).name.to_lowercase();
        let take = name.chars().count().min(4);
        cases.push(Case {
            class: "kind",
            query: name.chars().take(take).collect(),
            kinds: vec![SymbolKind::Function],
        });
    }
    cases.retain(|c| !c.query.is_empty());
    cases
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx]
}

fn loadavg() -> String {
    std::fs::read_to_string("/proc/loadavg")
        .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|_| "?".into())
}

fn build(files: &[(String, Vec<Decl>)]) -> (SearchIndex, f64, usize) {
    let before = LIVE.load(Ordering::Relaxed);
    let started = Instant::now();
    let index = SearchIndex::new();
    for (file, decls) in files {
        index.index_file(file, symbols(decls));
    }
    let secs = started.elapsed().as_secs_f64();
    let held = LIVE.load(Ordering::Relaxed).saturating_sub(before);
    (index, secs, held)
}

fn query(index: &SearchIndex, case: &Case, limit: usize) -> Vec<(String, String, u32, MatchType, f32)> {
    index
        .search(&SymbolSearchQuery {
            query: case.query.clone(),
            kinds: case.kinds.clone(),
            max_results: limit,
        })
        .into_iter()
        .map(|r| {
            (
                r.symbol.name().to_string(),
                r.symbol.location().file().to_string(),
                r.symbol.location().line(),
                r.match_type,
                r.score,
            )
        })
        .collect()
}

/// Per-call latencies in microseconds for one pass over the cases.
fn timed_pass(index: &SearchIndex, cases: &[Case]) -> Vec<(&'static str, f64)> {
    cases
        .iter()
        .map(|case| {
            let q = SymbolSearchQuery {
                query: case.query.clone(),
                kinds: case.kinds.clone(),
                max_results: LIMIT,
            };
            let started = Instant::now();
            black_box(index.search(black_box(&q)));
            (case.class, started.elapsed().as_secs_f64() * 1e6)
        })
        .collect()
}

// ---- the oracle: the documented semantics, written independently ----

fn oracle_class(name: &str, query: &str) -> Option<(MatchType, f32)> {
    let name = name.to_lowercase();
    if name == query {
        Some((MatchType::Exact, 1.0))
    } else if name.starts_with(query) {
        Some((MatchType::Prefix, 0.9))
    } else if let Some(pos) = name.find(query) {
        Some((MatchType::Contains, 0.7 + 0.2 * (1.0 - pos as f32 / name.len() as f32)))
    } else {
        let mut chars = query.chars().peekable();
        for c in name.chars() {
            if chars.peek() == Some(&c) {
                chars.next();
            }
        }
        chars.peek().is_none().then_some((MatchType::Fuzzy, 0.6))
    }
}

/// Recall of `search` against the brute-force oracle, tie aware: a hit
/// counts when it is a true match, reports the oracle's type and score, and
/// ranks at least as high as the oracle's k-th best.
fn recall(index: &SearchIndex, decls: &[Decl], cases: &[Case], limit: usize) -> (f64, usize) {
    let mut sum = 0.0;
    let mut counted = 0usize;
    let mut wrong = 0usize;
    for case in cases {
        let q = case.query.to_lowercase();
        let mut truth: Vec<(MatchType, f32, &Decl)> = decls
            .iter()
            .filter(|d| case.kinds.is_empty() || case.kinds.contains(&d.kind))
            .filter_map(|d| oracle_class(&d.name, &q).map(|(t, s)| (t, s, d)))
            .collect();
        truth.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.partial_cmp(&a.1).unwrap()));
        let expected = truth.len().min(limit);
        if expected == 0 {
            continue;
        }
        let threshold = (truth[expected - 1].0, truth[expected - 1].1);
        let got = query(index, case, limit);
        let mut good = 0usize;
        for (name, file, line, t, s) in &got {
            let valid = truth
                .iter()
                .any(|(tt, ts, d)| d.name == *name && d.file == *file && d.line == *line && tt == t && ts == s);
            let ranks = (*t, *s) >= threshold;
            if valid && ranks {
                good += 1;
            } else {
                wrong += 1;
            }
        }
        sum += good as f64 / expected as f64;
        counted += 1;
    }
    (if counted == 0 { 1.0 } else { sum / counted as f64 }, wrong)
}

fn row(label: &str, runs: &mut Vec<Vec<f64>>) {
    let mut p50s = Vec::new();
    let mut p95s = Vec::new();
    for run in runs.iter_mut() {
        run.sort_by(|a, b| a.partial_cmp(b).unwrap());
        p50s.push(percentile(run, 0.5));
        p95s.push(percentile(run, 0.95));
    }
    let range = |v: &[f64]| {
        let lo = v.iter().cloned().fold(f64::MAX, f64::min);
        let hi = v.iter().cloned().fold(0.0, f64::max);
        format!("{lo:>9.1}-{hi:<9.1}")
    };
    println!("  {label:<16} p50 us {}  p95 us {}", range(&p50s), range(&p95s));
}

fn main() {
    let runs: usize = std::env::var("SEARCH_RUNS").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    let per_class: usize = std::env::var("SEARCH_QUERIES").ok().and_then(|v| v.parse().ok()).unwrap_or(30);
    let tokens: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .collect();
    let tokens = if tokens.is_empty() { vec!["10000".into(), "100000".into()] } else { tokens };

    println!("symbol_search: runs={runs} queries/class={per_class} limit={LIMIT} load at start: {}", loadavg());
    for token in tokens {
        let decls = if token == "real" {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates");
            real_corpus(&root)
        } else {
            synthetic(token.parse().expect("size or `real`"))
        };
        let files = by_file(&decls);
        println!("\n== corpus {token}: {} declarations in {} files", decls.len(), files.len());

        let mut build_secs = Vec::new();
        let mut held = 0;
        for _ in 0..runs {
            let (index, secs, bytes) = build(&files);
            build_secs.push(secs);
            held = bytes;
            drop(index);
        }
        println!(
            "  build            {:?} s (runs), heap retained by the index {:.1} MiB = {:.0} B/symbol",
            build_secs.iter().map(|s| (s * 1000.0).round() / 1000.0).collect::<Vec<_>>(),
            held as f64 / 1048576.0,
            held as f64 / decls.len() as f64
        );

        let (index, _, _) = build(&files);
        let cases = query_cases(&decls, per_class);
        let _ = timed_pass(&index, &cases); // warm
        let mut per_class_runs: HashMap<&'static str, Vec<Vec<f64>>> = HashMap::new();
        for _ in 0..runs {
            let mut this: HashMap<&'static str, Vec<f64>> = HashMap::new();
            for _ in 0..3 {
                for (class, us) in timed_pass(&index, &cases) {
                    this.entry(class).or_default().push(us);
                }
            }
            for (class, v) in this {
                per_class_runs.entry(class).or_default().push(v);
            }
        }
        for class in ["exact", "prefix", "contains", "fuzzy", "short2", "miss", "kind"] {
            if let Some(mut r) = per_class_runs.remove(class) {
                row(class, &mut r);
            }
        }

        // reindex one file
        let mut rng = Rng(7);
        let mut reindex: Vec<Vec<f64>> = Vec::new();
        for _ in 0..runs {
            let mut samples = Vec::new();
            for _ in 0..200 {
                let (file, decls) = &files[rng.below(files.len())];
                let mut changed: Vec<Decl> = decls.iter().take(SYMBOLS_PER_FILE).cloned().collect();
                if let Some(first) = changed.first_mut() {
                    first.name.push_str("_renamed");
                }
                let syms = symbols(&changed);
                let started = Instant::now();
                index.index_file(file, syms);
                samples.push(started.elapsed().as_secs_f64() * 1e6);
                index.index_file(file, symbols(decls));
            }
            reindex.push(samples);
        }
        row("reindex-file", &mut reindex);

        // recall against the oracle (skipped at the largest sizes: O(n) per query)
        if decls.len() <= 200_000 {
            for class in ["exact", "prefix", "contains", "fuzzy", "kind"] {
                let subset: Vec<Case> = cases.iter().filter(|c| c.class == class).cloned().collect();
                let limit = if class == "exact" { 1_000_000 } else { LIMIT };
                let (r, wrong) = recall(&index, &decls, &subset, limit);
                println!("  recall {class:<9} {:.4} (hits that were wrong: {wrong}, limit {limit})", r);
            }
        }

        // concurrent readers
        let index = Arc::new(index);
        let cases = Arc::new(cases);
        let mut rates = Vec::new();
        for _ in 0..runs {
            let started = Instant::now();
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let index = Arc::clone(&index);
                    let cases = Arc::clone(&cases);
                    std::thread::spawn(move || {
                        let mut n = 0usize;
                        for _ in 0..2 {
                            n += timed_pass(&index, &cases).len();
                        }
                        n
                    })
                })
                .collect();
            let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
            rates.push(total as f64 / started.elapsed().as_secs_f64());
        }
        println!(
            "  concurrent4      {:?} queries/s (runs)",
            rates.iter().map(|r| r.round()).collect::<Vec<_>>()
        );

        // readers while a writer keeps reindexing files
        let mut rw: Vec<Vec<f64>> = Vec::new();
        let mut writes = Vec::new();
        for _ in 0..runs {
            let stop = Arc::new(AtomicBool::new(false));
            let writer = {
                let index = Arc::clone(&index);
                let stop = Arc::clone(&stop);
                let files = files.clone();
                std::thread::spawn(move || {
                    let mut rng = Rng(11);
                    let mut n = 0usize;
                    let started = Instant::now();
                    while !stop.load(Ordering::Relaxed) {
                        let (file, decls) = &files[rng.below(files.len())];
                        index.index_file(file, symbols(decls));
                        n += 1;
                    }
                    n as f64 / started.elapsed().as_secs_f64()
                })
            };
            let readers: Vec<_> = (0..4)
                .map(|_| {
                    let index = Arc::clone(&index);
                    let cases = Arc::clone(&cases);
                    std::thread::spawn(move || {
                        timed_pass(&index, &cases).into_iter().map(|(_, us)| us).collect::<Vec<_>>()
                    })
                })
                .collect();
            let mut all = Vec::new();
            for r in readers {
                all.extend(r.join().unwrap());
            }
            stop.store(true, Ordering::Relaxed);
            writes.push(writer.join().unwrap());
            rw.push(all);
        }
        row("readers+writer", &mut rw);
        println!(
            "  writer           {:?} reindexed files/s (runs)",
            writes.iter().map(|w| w.round()).collect::<Vec<_>>()
        );
    }
}
