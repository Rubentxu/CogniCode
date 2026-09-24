//! PRF-F4-W2: bit-a-bit determinism of the real `cognicode` binary
//! across two process restarts.
//!
//! F4 criterion: "el binario real, en el mismo corpus, produce el
//! mismo output byte-a-byte entre dos invocaciones distintas (simulando
//! un restart del proceso)". The W1 simulation tests covered the
//! HandlerContext API; W2 covers the actual `cognicode` CLI binary
//! end-to-end against a real corpus on disk.
//!
//! Comparison surface (narrowest observable shared by the binary):
//! stdout of `cognicode index query <sym> <corpus>` and
//! `cognicode graph per-file <file> <corpus>`. stderr (timestamps,
//! thread-pool init lines) is intentionally discarded because it
//! varies across runs even when the binary is deterministic.
//!
//! Non-vacuity guards (mandatory per operator rules): each test
//! asserts the query/find output is non-empty (i.e., the corpus
//! actually produces a result, so a "matches itself trivially"
//! assertion would never pass on an empty corpus).

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::binary_path;

/// Locate the `cognicode` binary under test. Uses the same
/// resolution chain as the other CLI tests so the binary matches
/// the test target build.
fn cognicode_bin() -> std::path::PathBuf {
    binary_path("cognicode")
}

/// Build a small Rust corpus with `unique_*` symbol names to avoid
/// collisions with real production source. Returns the corpus
/// root directory.
fn make_corpus(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("prf-f4-w2-corpus-{tag}"));
    let src = dir.join("src");
    fs::create_dir_all(&src).expect("create corpus src dir");
    fs::write(
        src.join("lib.rs"),
        format!(
            "//! F4.W2 fixture ({tag}).\n\
             pub fn unique_{tag}_alpha(x: u32) -> u32 {{ x + 1 }}\n\
             pub fn unique_{tag}_beta(x: u32) -> u32 {{ unique_{tag}_alpha(x) + 1 }}\n"
        ),
    )
    .expect("write lib.rs");
    dir
}

/// Run `cognicode` with stderr discarded. Returns the stdout as a
/// String. Failing fast on non-zero exit because a non-zero exit
/// would mean the corpus or binary is broken and the comparison
/// is meaningless.
fn run_cognicode(args: &[&str], cwd: &Path) -> String {
    let bin = cognicode_bin();
    assert!(
        bin.exists(),
        "cognicode binary not found at {}; build with `cargo build --release --bin cognicode`",
        bin.display()
    );
    let output = Command::new(&bin)
        .args(args)
        .current_dir(cwd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .expect("spawn cognicode");
    assert!(
        output.status.success(),
        "cognicode {:?} failed in {}: exit={:?}, stderr suppressed",
        args,
        cwd.display(),
        output.status.code()
    );
    String::from_utf8(output.stdout).expect("stdout is utf-8")
}

/// F4.W2 / test 1: `index query` is bit-a-bit deterministic across
/// two distinct process invocations (simulating restart).
///
/// RED/GREEN invariant: if the corpus file is modified between
/// run A and run B, the two outputs must differ; on the
/// unmodified corpus, the two outputs must be byte-equal. The
/// `index query` command takes `(symbol, corpus)` and prints a
/// list of `(file:line:column, kind)` locations.
#[test]
fn prf_f4_w2_index_query_bit_a_bit_equal_across_restarts() {
    let corpus = make_corpus("query");
    let query_sym = format!("unique_query_alpha");

    // Cold invocation
    let out_a = run_cognicode(
        &["index", "query", &query_sym, corpus.to_str().unwrap()],
        &corpus,
    );
    // Warm invocation (simulated restart: separate process)
    let out_b = run_cognicode(
        &["index", "query", &query_sym, corpus.to_str().unwrap()],
        &corpus,
    );

    // Non-vacuity guard: the corpus must actually contain the symbol.
    assert!(
        !out_a.trim().is_empty(),
        "non-vacuity: cold index query returned empty output for {}",
        query_sym
    );
    assert!(
        out_a.contains("Found 1 location"),
        "non-vacuity: expected exactly 1 location for {}, got: {}",
        query_sym,
        out_a
    );

    // Bit-a-bit equality
    assert_eq!(
        out_a, out_b,
        "bit-a-bit determinism violated for index query across restarts:\n--- A ---\n{}\n--- B ---\n{}",
        out_a, out_b
    );

    // Cleanup
    let _ = fs::remove_dir_all(&corpus);
}

/// F4.W2 / test 2: `graph per-file` is bit-a-bit deterministic across
/// two distinct process invocations (simulating restart).
///
/// RED/GREEN invariant: same as test 1 but for `graph per-file`.
/// The corpus must contain at least one symbol so the output is
/// non-empty (non-vacuity guard). Note: `graph per-file` takes
/// only `<FILE>` as a positional argument; the corpus is inferred
/// from the cwd or the absolute file path. We pass the absolute
/// path so the test is invariant to cwd, and run with `cwd=corpus`
/// so the binary can locate the file either way.
#[test]
fn prf_f4_w2_graph_per_file_bit_a_bit_equal_across_restarts() {
    let corpus = make_corpus("perfile");
    let abs_file = corpus.join("src/lib.rs");
    let abs_file_str = abs_file.to_str().unwrap();

    // Cold
    let out_a = run_cognicode(&["graph", "per-file", abs_file_str], &corpus);
    // Warm (restart)
    let out_b = run_cognicode(&["graph", "per-file", abs_file_str], &corpus);

    // Non-vacuity guard: per-file output must list symbols
    assert!(
        out_a.contains("Symbols:"),
        "non-vacuity: graph per-file output missing Symbols: header:\n{}",
        out_a
    );
    assert!(
        out_a.contains("Symbols: 2"),
        "non-vacuity: expected exactly 2 symbols in fixture (alpha + beta), got:\n{}",
        out_a
    );

    // Bit-a-bit equality
    assert_eq!(
        out_a, out_b,
        "bit-a-bit determinism violated for graph per-file across restarts:\n--- A ---\n{}\n--- B ---\n{}",
        out_a, out_b
    );

    // Cleanup
    let _ = fs::remove_dir_all(&corpus);
}

/// F4.W2 / test 3: restart of the binary against workspace A does
/// not contaminate workspace B (filesystem isolation end-to-end).
///
/// RED/GREEN invariant: running the binary against corpus A and
/// then corpus B (both with unique prefixes so their symbols do
/// not collide) must produce outputs that reference A's symbols
/// and B's symbols respectively, never cross-contaminating.
#[test]
fn prf_f4_w2_restart_in_a_does_not_contaminate_b() {
    let corpus_a = make_corpus("alpha");
    let corpus_b = make_corpus("beta");

    // Run against A
    let out_a = run_cognicode(
        &[
            "index",
            "query",
            "unique_alpha_alpha",
            corpus_a.to_str().unwrap(),
        ],
        &corpus_a,
    );
    // Restart, run against B
    let out_b = run_cognicode(
        &[
            "index",
            "query",
            "unique_beta_beta",
            corpus_b.to_str().unwrap(),
        ],
        &corpus_b,
    );

    // Non-vacuity guards
    assert!(
        out_a.contains("Found 1 location"),
        "non-vacuity: corpus A did not return 1 location for unique_alpha_alpha:\n{}",
        out_a
    );
    assert!(
        out_b.contains("Found 1 location"),
        "non-vacuity: corpus B did not return 1 location for unique_beta_beta:\n{}",
        out_b
    );

    // Cross-contamination check: A's output must NOT contain B's
    // unique symbol, and vice versa.
    assert!(
        !out_a.contains("unique_beta_beta"),
        "contamination: corpus A output contains B's symbol:\n{}",
        out_a
    );
    assert!(
        !out_b.contains("unique_alpha_alpha"),
        "contamination: corpus B output contains A's symbol:\n{}",
        out_b
    );

    // Cleanup
    let _ = fs::remove_dir_all(&corpus_a);
    let _ = fs::remove_dir_all(&corpus_b);
}
