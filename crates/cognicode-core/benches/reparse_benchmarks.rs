//! Criterion benchmarks for the incremental reindex — what it costs.
//!
//! Run with: `cargo bench -p cognicode-core --bench reparse_benchmarks`
//!
//! What is measured: what `reparse_on_edit` costs over a workspace, in
//! three shapes — nothing changed (a cache hit), one file changed (a
//! partial rebuild) and a mass change (a quarter of the files read
//! again). Each shape is measured over a small and a large synthetic
//! workspace, so the reader can tell the cost of the work from the cost
//! of the tree.
//!
//! The corpus is generated, deterministic and never committed: the same
//! files with the same content on every run, so two runs measure the
//! same work.
//!
//! The preparation is not part of the measurement. `iter_batched`
//! rewrites the corpus before every iteration and the timer covers the
//! operation and nothing else. The content of a changed file carries a
//! counter, so a changed file never matches the manifest by content
//! whatever the clock says: the benchmark measures a rebuild every
//! time and never a cache hit by accident.
//!
//! This target is deliberately absent from `BENCH_SPECS` in
//! `scripts/perf-budget-check.sh`. It measures the incremental reindex,
//! which is not an operation `perf-budget.toml` budgets, and the
//! checker runs on demand at a cost of hundreds of seconds: adding a
//! target that no budget reads would make that run slower and say
//! nothing new.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};

use cognicode_core::interface::mcp::handlers::aix_handlers::handle_reparse_on_edit;
use cognicode_core::interface::mcp::handlers::{
    BuildGraphInput, HandlerContext, handle_build_graph,
};
use cognicode_core::interface::mcp::schemas::ReparseOnEditInput;

/// The small workspace: twelve files.
const SMALL: usize = 12;

/// The large workspace: one hundred and twenty files.
const LARGE: usize = 120;

/// How many samples each measurement takes.
///
/// Ten is the floor Criterion accepts, and it is enough here: the
/// measurement is of an operation whose cost is dominated by parsing
/// and graph assembly, not by a distribution of microseconds.
const SAMPLES: usize = 10;

/// How long the measurement lasts.
const MEASURE_TIME: Duration = Duration::from_secs(5);

/// How long the warm-up lasts.
const WARM_UP_TIME: Duration = Duration::from_secs(1);

/// The runtime the operation runs in.
///
/// The operation is asynchronous, and Criterion's async support needs a
/// feature this workspace does not enable; a runtime driven by hand is
/// the honest alternative to adding one.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("el runtime del benchmark")
}

/// The content of one file of the corpus.
///
/// The two functions and the call between them are what give the graph
/// something to hold: a corpus of empty files would measure the empty
/// case and call it a workspace.
fn original_body(index: usize) -> String {
    format!(
        "pub fn helper_{index}(value: u32) -> u32 {{\n    value + {index}\n}}\n\n\
         pub fn caller_{index}(value: u32) -> u32 {{\n    helper_{index}(value)\n}}\n"
    )
}

/// The content of a file that changed.
///
/// The `tag` makes the content different on every iteration: a file
/// whose bytes always differ from the manifest is a changed file
/// whatever the clock says.
fn changed_body(index: usize, tag: usize) -> String {
    format!(
        "pub fn helper_{index}(value: u32) -> u32 {{\n    value + {index}\n}}\n\n\
         pub fn caller_{index}(value: u32) -> u32 {{\n    helper_{index}(value) + {tag:08}\n}}\n"
    )
}

/// A workspace with its corpus and the context that observes it.
struct Workspace {
    /// The directory that holds the corpus. It has to outlive the
    /// context: the files are read on every iteration.
    _dir: tempfile::TempDir,
    /// The context the operation runs in.
    ctx: HandlerContext,
}

impl Workspace {
    /// Writes a corpus of `files` files and builds the graph over it.
    ///
    /// The graph is built before anything is measured: the rebuild the
    /// benchmark measures is the second one, over a tree the product
    /// already knows. A cold session would measure the cold session.
    fn build(files: usize) -> Self {
        let dir = tempfile::tempdir().expect("el directorio del workspace");
        let src = dir.path().join("src");
        std::fs::create_dir_all(&src).expect("se crea el directorio");
        for index in 0..files {
            std::fs::write(src.join(format!("file_{index}.rs")), original_body(index))
                .expect("se escribe el fichero");
        }

        let ctx = HandlerContext::builder()
            .with_working_dir(dir.path())
            .build();
        let rt = runtime();
        rt.block_on(async {
            let build = handle_build_graph(&ctx, BuildGraphInput { directory: None })
                .await
                .expect("build_graph responde");
            assert_eq!(
                build.status, "complete",
                "el grafo tiene que estar completo: si no, el manifiesto no se guarda y \
                 los ficheros se tratan como nuevos en vez de como modificados"
            );
        });

        Self { _dir: dir, ctx }
    }

    /// The paths of the files the operation is asked about.
    fn paths(&self, indexes: &[usize]) -> Vec<String> {
        indexes
            .iter()
            .map(|index| format!("src/file_{index}.rs"))
            .collect()
    }

    /// Writes the changed content over the given files.
    ///
    /// The content carries the tag, so it never matches the manifest
    /// and the operation sees a changed file.
    fn change(&self, indexes: &[usize], tag: usize) {
        for &index in indexes {
            let path = self
                ._dir
                .path()
                .join("src")
                .join(format!("file_{index}.rs"));
            std::fs::write(path, changed_body(index, tag)).expect("se reescribe el fichero");
        }
    }
}

/// The indexes of the files a mass change touches: one in four.
fn mass_indexes(files: usize) -> Vec<usize> {
    (0..files).filter(|index| index % 4 == 0).collect()
}

/// Checks that the operation really does what the benchmark claims to
/// measure.
///
/// A benchmark that measures a cache hit while announcing a partial
/// rebuild is a number about the wrong work. The check runs once,
/// before the measurement, and reads the word the product publishes.
fn check_kind(
    rt: &tokio::runtime::Runtime,
    ctx: &HandlerContext,
    paths: &[String],
    expected: &str,
) {
    let published = rt.block_on(async {
        handle_reparse_on_edit(
            ctx,
            ReparseOnEditInput {
                file_paths: paths.to_vec(),
                edit_ranges: None,
            },
        )
        .await
        .expect("reparse_on_edit responde")
    });
    assert_eq!(
        published.cost.rebuild_kind.published_label(),
        expected,
        "el benchmark mide {expected} y el producto dice otra cosa"
    );
}

/// Measures the three shapes of a reparse over a workspace of `files`.
fn measure_shape(
    c: &mut Criterion,
    label: &str,
    shape: &str,
    kind: &str,
    files: usize,
    indexes: Vec<usize>,
) {
    let workspace = Workspace::build(files);
    let rt = runtime();
    let paths = workspace.paths(&indexes);
    let tag = AtomicUsize::new(1);

    // The operation is checked once before it is measured: the benchmark
    // has to announce the rebuild it is really measuring.
    workspace.change(&indexes, 0);
    check_kind(&rt, &workspace.ctx, &paths, "partial");

    let mut group = c.benchmark_group(format!("reparse/{label}/{shape}"));
    group.throughput(Throughput::Elements(indexes.len() as u64));
    group.sample_size(SAMPLES);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    group.bench_function(kind, |b| {
        b.iter_batched(
            || {
                // The preparation: the corpus goes back to the state the
                // operation has to find. It is not measured.
                let current = tag.fetch_add(1, Ordering::Relaxed);
                workspace.change(&indexes, current);
            },
            |()| {
                rt.block_on(async {
                    handle_reparse_on_edit(
                        &workspace.ctx,
                        ReparseOnEditInput {
                            file_paths: paths.clone(),
                            edit_ranges: None,
                        },
                    )
                    .await
                    .expect("reparse_on_edit responde")
                })
            },
            BatchSize::PerIteration,
        )
    });

    group.finish();
}

/// Measures a reparse that finds nothing changed.
///
/// The corpus is not touched at all: the operation reads the state,
/// compares it with the manifest and stops there.
fn measure_no_change(c: &mut Criterion, label: &str, files: usize) {
    let workspace = Workspace::build(files);
    let rt = runtime();
    let paths = workspace.paths(&[0]);

    // The operation is checked once before it is measured: the benchmark
    // has to announce the rebuild it is really measuring.
    check_kind(&rt, &workspace.ctx, &paths, "cache_hit");

    let mut group = c.benchmark_group(format!("reparse/{label}/no_change"));
    group.throughput(Throughput::Elements(1));
    group.sample_size(SAMPLES);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    group.bench_function("cache_hit", |b| {
        b.iter_batched(
            || (),
            |()| {
                rt.block_on(async {
                    handle_reparse_on_edit(
                        &workspace.ctx,
                        ReparseOnEditInput {
                            file_paths: paths.clone(),
                            edit_ranges: None,
                        },
                    )
                    .await
                    .expect("reparse_on_edit responde")
                })
            },
            BatchSize::PerIteration,
        )
    });

    group.finish();
}

/// The benchmarks: two workspace sizes and three shapes of change.
fn bench_reparse_costs(c: &mut Criterion) {
    for (label, files) in [("small", SMALL), ("large", LARGE)] {
        measure_no_change(c, label, files);
        measure_shape(c, label, "one_file", "partial", files, vec![0]);
        measure_shape(c, label, "mass", "partial_many", files, mass_indexes(files));
    }
}

criterion_group!(benches, bench_reparse_costs);
criterion_main!(benches);
