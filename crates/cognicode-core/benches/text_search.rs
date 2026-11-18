//! Measurement harness for content search over a real tree of files.
//!
//! Not a criterion bench: it reports p50/p95 per query class together with
//! the spread across runs, the host load, the cold cost of building the text
//! index and the heap it retains, because on a shared host a single mean says
//! little.
//!
//! Run: `cargo bench -p cognicode-core --bench text_search -- crates docs`
//! Tokens are directories (relative to the current directory) that are copied
//! to a temporary directory and searched there, so the incremental rows can
//! rewrite files. `SEARCH_RUNS` sets the timed runs per row (default 3) and
//! `SEARCH_QUERIES` the queries per class (default 20).
//!
//! Queries are drawn from the corpus: words that occur in a few files, in
//! some and in many, plus a word that occurs in none, so the classes mean the
//! same on every corpus. Rows:
//! - `plain`: the content search as it is without the index.
//! - `indexed`: the same requests through a service with the text index,
//!   after the index was built by a first search.
//! - `cold`: the first search with an empty index, which reads and indexes
//!   every file, and the heap the index retains afterwards.
//! - `one file changed`: a file is rewritten and then searched for a word
//!   that is not in it; only the search is timed.
//!
//! Every answer of the indexed service is compared with the plain one; the
//! harness stops if one differs.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use cognicode_core::application::dto::{SearchContentRequest, SearchContentResult};
use cognicode_core::application::services::file_operations::FileOperationsService;
use cognicode_core::application::services::text_index::TextIndexConfig;
use cognicode_core::infrastructure::parser::syntax_analysis::TreeSitterSyntaxAnalysis;
use cognicode_core::infrastructure::verification::RustVerifier;
use cognicode_core::interface::mcp::security::InputValidator;
use ignore::WalkBuilder;

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

const MAX_RESULTS: usize = 100;

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
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn service(root: &Path, index: Option<TextIndexConfig>) -> FileOperationsService {
    let service = FileOperationsService::new(
        root.to_string_lossy().to_string(),
        Arc::new(InputValidator::new().with_workspace(vec![root.to_path_buf()])),
        Arc::new(RustVerifier::new()),
        Arc::new(TreeSitterSyntaxAnalysis::new()),
    );
    match index {
        Some(config) => service.with_text_index(config),
        None => service,
    }
}

/// Copies the files of `source` that a search would visit, with the same
/// relative layout, into `target`; returns how many and how many bytes.
fn copy_corpus(source: &Path, target: &Path) -> (usize, u64) {
    let (mut files, mut bytes) = (0, 0);
    for entry in WalkBuilder::new(source)
        .require_git(false)
        .build()
        .flatten()
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let relative = path.strip_prefix(source).expect("inside the source");
        let destination = target.join(relative);
        fs::create_dir_all(destination.parent().expect("parent")).expect("directory");
        if let Ok(len) = fs::copy(path, &destination) {
            files += 1;
            bytes += len;
        }
    }
    (files, bytes)
}

fn files_of(root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkBuilder::new(root)
        .require_git(false)
        .build()
        .flatten()
        .map(|e| e.into_path())
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    files
}

/// For each word of at least six identifier characters, in how many files it
/// occurs.
fn document_frequency(files: &[PathBuf]) -> HashMap<String, usize> {
    let mut frequency: HashMap<String, usize> = HashMap::new();
    for path in files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let mut seen = std::collections::HashSet::new();
        for word in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')) {
            if word.len() >= 6 && !word.chars().next().unwrap().is_ascii_digit() {
                seen.insert(word);
            }
        }
        for word in seen {
            *frequency.entry(word.to_string()).or_default() += 1;
        }
    }
    frequency
}

struct Query {
    class: &'static str,
    request: SearchContentRequest,
}

fn request(pattern: String, regex: bool, insensitive: bool) -> SearchContentRequest {
    SearchContentRequest {
        regex: Some(regex),
        case_insensitive: Some(insensitive),
        max_results: Some(MAX_RESULTS),
        context_lines: Some(0),
        ..SearchContentRequest::new(pattern)
    }
}

fn pick<'a>(words: &'a [&'a String], rng: &mut Rng, n: usize) -> Vec<&'a String> {
    if words.is_empty() {
        return Vec::new();
    }
    (0..n).map(|_| words[rng.below(words.len())]).collect()
}

fn queries(frequency: &HashMap<String, usize>, files: usize, per_class: usize) -> Vec<Query> {
    let mut sorted: Vec<&String> = frequency.keys().collect();
    sorted.sort();
    let within = |low: usize, high: usize| -> Vec<&String> {
        sorted
            .iter()
            .copied()
            .filter(|w| (low..=high).contains(&frequency[*w]))
            .collect()
    };
    let rare = within(1, 2);
    let mid = within((files / 100).max(3), (files / 20).max(6));
    let common = within((files / 4).max(10), usize::MAX);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut out = Vec::new();
    for word in pick(&rare, &mut rng, per_class) {
        out.push(Query {
            class: "lit rare",
            request: request(word.to_string(), false, false),
        });
        out.push(Query {
            class: "lit rare ci",
            request: request(word.to_uppercase(), false, true),
        });
    }
    for word in pick(&mid, &mut rng, per_class) {
        out.push(Query {
            class: "lit mid",
            request: request(word.to_string(), false, false),
        });
        out.push(Query {
            class: "regex prefix",
            request: request(format!("{word}\\w+"), true, false),
        });
    }
    for word in pick(&common, &mut rng, per_class) {
        out.push(Query {
            class: "lit common",
            request: request(word.to_string(), false, false),
        });
    }
    for _ in 0..per_class {
        let (a, b, c) = (
            pick(&rare, &mut rng, 1),
            pick(&rare, &mut rng, 1),
            pick(&rare, &mut rng, 1),
        );
        if let (Some(a), Some(b), Some(c)) = (a.first(), b.first(), c.first()) {
            out.push(Query {
                class: "regex alt",
                request: request(format!("({a}|{b}|{c})"), true, false),
            });
        }
    }
    for n in 0..per_class {
        out.push(Query {
            class: "regex suffix",
            request: request(
                format!("\\w+_{}", ["id", "ok", "len", "key"][n % 4]),
                true,
                false,
            ),
        });
        out.push(Query {
            class: "regex open",
            request: request("[a-z]+\\d+".to_string(), true, false),
        });
        out.push(Query {
            class: "lit short",
            request: request(["fn", "ab", "xy", "if"][n % 4].to_string(), false, false),
        });
        out.push(Query {
            class: "miss",
            request: request(format!("zq{n}xk{}wv", n * 7), false, false),
        });
    }
    out
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn range(values: &[f64]) -> String {
    let low = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let high = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    format!("{low:>9.1}-{high:<9.1}")
}

/// A stable rendering of what a search answered, to compare two services.
fn digest(result: &SearchContentResult) -> String {
    serde_json::json!({
        "matches": result.matches,
        "total": result.total,
        "files_scanned": result.files_scanned,
        "unread": result.unread,
    })
    .to_string()
}

fn timed(
    service: &FileOperationsService,
    request: &SearchContentRequest,
) -> (f64, SearchContentResult) {
    let start = Instant::now();
    let result = service.search_content(request.clone()).expect("search");
    (start.elapsed().as_secs_f64() * 1e6, black_box(result))
}

fn main() {
    let tokens: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .collect();
    let tokens = if tokens.is_empty() {
        vec!["crates".to_string(), "docs".to_string()]
    } else {
        tokens
    };
    let runs = env_usize("SEARCH_RUNS", 3);
    let per_class = env_usize("SEARCH_QUERIES", 20);
    println!(
        "text_search: runs={runs} queries/class={per_class} max_results={MAX_RESULTS} load at start: {}",
        std::fs::read_to_string("/proc/loadavg")
            .map(|s| s.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
            .unwrap_or_default()
    );
    for token in &tokens {
        corpus(Path::new(token), runs, per_class);
    }
}

fn corpus(source: &Path, runs: usize, per_class: usize) {
    let scratch = tempfile::tempdir().expect("tempdir");
    let root = scratch.path().join("corpus");
    fs::create_dir_all(&root).expect("root");
    let (file_count, bytes) = copy_corpus(source, &root);
    // A file changed less than the quiet period ago is read, not indexed
    // (see the `one file changed` row for that case); let the copy age out so
    // the other rows measure the steady state.
    std::thread::sleep(std::time::Duration::from_millis(2500));
    let files = files_of(&root);
    let frequency = document_frequency(&files);
    let all = queries(&frequency, files.len(), per_class);
    println!(
        "\n== corpus {}: {file_count} files, {:.1} MiB",
        source.display(),
        bytes as f64 / (1024.0 * 1024.0)
    );

    let mut classes: Vec<&'static str> = Vec::new();
    for query in &all {
        if !classes.contains(&query.class) {
            classes.push(query.class);
        }
    }

    // Per run: the plain service answers everything, then a fresh indexed
    // service builds its index with one search and answers everything again.
    let mut plain_us: HashMap<&str, Vec<Vec<f64>>> = HashMap::new();
    let mut indexed_us: HashMap<&str, Vec<Vec<f64>>> = HashMap::new();
    let mut cold_s = Vec::new();
    let mut heap = Vec::new();
    let mut shares: HashMap<&str, (usize, usize, usize, usize, usize)> = HashMap::new();
    let mut not_used: HashMap<&str, String> = HashMap::new();
    let mut warm_cold_check = 0usize;
    for _ in 0..runs {
        let plain = service(&root, None);
        let mut reference = Vec::with_capacity(all.len());
        for query in &all {
            let (us, result) = timed(&plain, &query.request);
            plain_us.entry(query.class).or_default().push(vec![us]);
            reference.push(digest(&result));
        }

        let before = LIVE.load(Ordering::Relaxed);
        let indexed = service(&root, Some(TextIndexConfig::default()));
        let start = Instant::now();
        let first = indexed
            .search_content(request("zq_cold_build".to_string(), false, false))
            .expect("cold search");
        cold_s.push(start.elapsed().as_secs_f64());
        heap.push(LIVE.load(Ordering::Relaxed).saturating_sub(before));
        let cold_report = first.text_index.expect("report");
        warm_cold_check = cold_report.indexed_now;

        for (query, expected) in all.iter().zip(&reference) {
            let (us, result) = timed(&indexed, &query.request);
            assert_eq!(
                &digest(&result),
                expected,
                "the index changed the answer of {:?}",
                query.request
            );
            indexed_us.entry(query.class).or_default().push(vec![us]);
            let report = result.text_index.expect("report");
            let entry = shares.entry(query.class).or_default();
            entry.0 += 1;
            entry.1 += usize::from(report.used);
            entry.2 += report.skipped;
            entry.3 += report.candidates;
            entry.4 += report.read_directly + report.indexed_now;
            if let Some(reason) = report.reason {
                not_used.entry(query.class).or_insert(reason);
            }
        }
    }

    println!(
        "  cold             {} s (runs: {:?}), {warm_cold_check} files indexed, heap retained {:.1} MiB = {:.0} B/file",
        range(&cold_s),
        cold_s
            .iter()
            .map(|s| (s * 1000.0).round() / 1000.0)
            .collect::<Vec<_>>(),
        heap[0] as f64 / (1024.0 * 1024.0),
        heap[0] as f64 / file_count.max(1) as f64,
    );
    println!(
        "  {:<14} {:>10} {:>21} {:>21}   {:>21} {:>21}   index: used / skipped / candidates / read-now (per query)",
        "class", "queries", "plain p50 us", "plain p95 us", "indexed p50 us", "indexed p95 us"
    );
    for class in &classes {
        let per_run = |table: &HashMap<&str, Vec<Vec<f64>>>| -> (Vec<f64>, Vec<f64>) {
            let samples: Vec<f64> = table[class].iter().flatten().copied().collect();
            let per = samples.len() / runs;
            let (mut p50, mut p95) = (Vec::new(), Vec::new());
            for run in 0..runs {
                let mut slice: Vec<f64> = samples[run * per..(run + 1) * per].to_vec();
                slice.sort_by(|a, b| a.partial_cmp(b).unwrap());
                p50.push(percentile(&slice, 0.5));
                p95.push(percentile(&slice, 0.95));
            }
            (p50, p95)
        };
        let (plain50, plain95) = per_run(&plain_us);
        let (idx50, idx95) = per_run(&indexed_us);
        let (n, used, skipped, candidates, read_now) = shares[class];
        println!(
            "  {:<14} {:>10} {:>21} {:>21}   {:>21} {:>21}   {}/{} / {:.0} / {:.0} / {:.0}{}",
            class,
            n / runs,
            range(&plain50),
            range(&plain95),
            range(&idx50),
            range(&idx95),
            used,
            n,
            skipped as f64 / n as f64,
            candidates as f64 / n as f64,
            read_now as f64 / n as f64,
            not_used
                .get(class)
                .map(|r| format!("  [{r}]"))
                .unwrap_or_default(),
        );
    }

    // One file changed, then a search that does not match it.
    let plain = service(&root, None);
    let indexed = service(&root, Some(TextIndexConfig::default()));
    let probe = request("zq_cold_build".to_string(), false, false);
    indexed.search_content(probe.clone()).expect("warm up");
    let mut touched_plain = Vec::new();
    let mut touched_indexed = Vec::new();
    for n in 0..(runs * 20) {
        let path = &files[(n * 37) % files.len()];
        if let Ok(mut text) = fs::read_to_string(path) {
            text.push_str(&format!("\n// touched {n}\n"));
            fs::write(path, text).expect("rewrite");
        }
        touched_plain.push(timed(&plain, &probe).0);
        touched_indexed.push(timed(&indexed, &probe).0);
    }
    let summarise = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        (percentile(&v, 0.5), percentile(&v, 0.95))
    };
    let (p50, p95) = summarise(touched_plain);
    let (i50, i95) = summarise(touched_indexed);
    println!(
        "  one file changed (search only, {} samples): plain p50 {p50:.0} p95 {p95:.0} us; indexed p50 {i50:.0} p95 {i95:.0} us",
        runs * 20
    );
}
