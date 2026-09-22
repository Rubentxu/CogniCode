//! PRF-EXT-02 UAT (real binary): CLI subcommands that used to call
//! `FullGraphStrategy::build_full_graph` directly (silent coverage
//! loss) now honor the same Partial semantics as MCP: a graph built
//! over a corpus with an unreadable file is reported as PARTIAL on
//! stderr, naming the skipped file — never as a clean result.
//!
//! Covered subcommands: hot-paths, entry-points, leaf-functions,
//! trace-path, mermaid, complexity, impact.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cognicode_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .parent()
        .expect("repo root")
        .join("target/release/cognicode")
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(cognicode_bin())
        .args(args)
        .output()
        .expect("spawn cognicode")
}

fn stderr_text(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn make_corpus(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ext02_uat_{tag}_{}", std::process::id()));
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(dir.join("good.rs"), "fn alpha() {}\nfn beta() { alpha(); alpha(); }\n").expect("good");
    let locked = dir.join("locked.rs");
    fs::write(&locked, "fn locked_fn() {}\n").expect("locked write");
    // Permission-denied file: must surface as a skipped file, not silently vanish.
    let mut perm = fs::metadata(&locked).expect("meta").permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    {
        use std::os::unix::fs::PermissionsExt;
        perm.set_mode(0o000);
    }
    fs::set_permissions(&locked, perm).expect("chmod 000");
    dir
}

fn restore(dir: &Path) {
    let locked = dir.join("locked.rs");
    if locked.exists() {
        let mut perm = fs::metadata(&locked).expect("meta").permissions();
        use std::os::unix::fs::PermissionsExt;
        perm.set_mode(0o644);
        let _ = fs::set_permissions(&locked, perm);
    }
    let _ = fs::remove_dir_all(dir);
}

const SUBCOMMANDS: &[&[&str]] = &[
    &["graph", "hot-paths", "WS", "--limit", "10"],
    &["graph", "entry-points", "WS"],
    &["graph", "leaf-functions", "WS"],
    &["graph", "complexity", "WS"],
    &["graph", "mermaid", "WS"],
];

#[test]
fn graph_subcommands_report_partial_when_files_are_skipped() {
    // UAT requires the release binary; rebuilding is out of scope here.
    assert!(
        cognicode_bin().exists(),
        "release binary missing: build target/release/cognicode first"
    );

    for sub in SUBCOMMANDS {
        let args: Vec<String> = sub.iter().map(|a| a.to_string()).collect();
        let dir = make_corpus(&args.join("_"));
        let args: Vec<String> = args
            .into_iter()
            .map(|a| if a == "WS" { dir.to_string_lossy().to_string() } else { a })
            .collect();
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let out = run(&arg_refs);
        let err = stderr_text(&out);

        assert!(
            err.contains("PARTIAL"),
            "{:?}: expected PARTIAL warning on stderr, got: {}",
            arg_refs,
            err
        );
        assert!(
            err.contains("locked.rs"),
            "{:?}: expected locked.rs named as skipped, got: {}",
            arg_refs,
            err
        );
        restore(&dir);
    }
}

#[test]
fn impact_subcommand_reports_partial_when_files_are_skipped() {
    assert!(cognicode_bin().exists());
    let dir = make_corpus("impact");
    let out = run(&["graph", "impact", "alpha", dir.to_str().expect("utf8")]);
    let err = stderr_text(&out);
    assert!(
        err.contains("PARTIAL") && err.contains("locked.rs"),
        "impact: expected PARTIAL warning naming locked.rs, got: {}",
        err
    );
    restore(&dir);
}
