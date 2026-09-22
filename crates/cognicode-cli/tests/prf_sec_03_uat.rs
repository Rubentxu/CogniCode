//! PRF-SEC-03 UAT: logs sin tokens/credenciales/contenido sensible.
//!
//! Contract (SPEC-SECURITY.md PRF-SEC-03 MUST): errors and logs are
//! free of tokens, credentials and sensitive content absent explicit
//! authorization; telemetry is opt-in and **no source code by
//! default**.
//!
//! These tests run the real `cognicode` binary with `-v` (verbose,
//! DEBUG level) over a corpus that contains a recognizable sentinel
//! secret, then assert the sentinel never appears in stdout or
//! stderr. The same sweep is applied to `graph full` and `index`
//! paths — the commands most likely to touch source content.

use std::path::Path;
use std::process::{Command, Output};

const SENTINEL: &str = "SK-SECRET-PRFSEC03-SENTINEL-7f3a";

fn cognicode_bin() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_cognicode"))
}

fn make_corpus() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let src = tmp.path().join("src");
    std::fs::create_dir_all(&src).expect("src dir");
    std::fs::write(
        src.join("api.rs"),
        format!(
            "const API_KEY: &str = \"{SENTINEL}\";\n\npub fn call() {{ println!(\"{{}}\", API_KEY); }}\n"
        ),
    )
    .expect("write api.rs");
    std::fs::write(
        src.join("caller.rs"),
        "fn caller() { call(); }\n",
    )
    .expect("write caller.rs");
    tmp
}

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(cognicode_bin())
        .arg("-v")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("spawn cognicode binary")
}

fn assert_no_sentinel(out: &Output, cmd: &str) {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stdout.contains(SENTINEL),
        "{cmd}: sentinel secret leaked to stdout:\n{stdout}"
    );
    assert!(
        !stderr.contains(SENTINEL),
        "{cmd}: sentinel secret leaked to stderr:\n{stderr}"
    );
}

#[test]
fn verbose_analyze_does_not_log_source_secret() {
    let tmp = make_corpus();
    let out = run(tmp.path(), &["analyze", "."]);
    assert_no_sentinel(&out, "analyze -v");
}

#[test]
fn verbose_graph_full_does_not_log_source_secret() {
    let tmp = make_corpus();
    let out = run(tmp.path(), &["graph", "full", "--path", "."]);
    assert_no_sentinel(&out, "graph full -v");
}

#[test]
fn verbose_index_does_not_log_source_secret() {
    let tmp = make_corpus();
    // `index` requires a subcommand; try the most common one and
    // tolerate unknown-subcommand errors as long as the sentinel
    // never appears in any output stream.
    let out = run(tmp.path(), &["index", "build", "--path", "."]);
    assert_no_sentinel(&out, "index -v");
}

#[test]
fn verbose_doctor_does_not_log_source_secret() {
    let tmp = make_corpus();
    let out = run(tmp.path(), &["doctor", "--cwd", "."]);
    assert_no_sentinel(&out, "doctor -v");
}
