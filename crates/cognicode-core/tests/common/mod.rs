//! Where the merge gate is decided, for the core contract tests.
//!
//! Six contract tests in this crate used to read `.github/workflows/pr-ci.yml`
//! to find out whether the merge gate ran them. That file is being retired:
//! the pipeline that gates a merge is now `merge-gate.pipeline.kts`. Each
//! test that re-derived that fact on its own would be a second place to
//! update, and the failure mode is not a compile error — it is a contract
//! that silently stops asserting anything.
//!
//! So the fact lives here once. The three properties these tests actually
//! care about are unchanged by the move: a merge must not be able to pass
//! without the gate, the gate must be inside a command body rather than in
//! a comment, and a stage that protects a named thing must still say so.
//!
//! This is deliberately the same idea as `scripts/ci/pipeline_authority.py`,
//! which does the same job for the Python contracts. Two languages cannot
//! share a module, so there are two implementations of one small rule, and
//! both are deliberately strict in the same direction: a match is only
//! counted on a line that actually invokes it.

use std::fs;
use std::path::PathBuf;

/// The pipeline a merge is gated on. Declared here and nowhere else.
pub const MERGE_AUTHORITY: &str = "merge-gate.pipeline.kts";

/// The repository root, for tests that need to reach other files.
pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cognicode-core has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

/// The merge authority's source with whole-line comments removed.
///
/// A comment that quotes a command is otherwise indistinguishable from a step
/// that runs it. That cuts both ways: a comment naming a removed gate would
/// satisfy "the gate exists", and a comment explaining why something is
/// pinned would trip "it is pinned".
pub fn merge_authority_lines() -> Vec<String> {
    let path = repo_root().join(MERGE_AUTHORITY);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
        .map(str::to_owned)
        .collect()
}

/// Whether the merge authority runs `command` inside an `sh(...)` body.
///
/// The `sh(` requirement is what makes this fail-closed. A command quoted
/// anywhere else — a `val`, a stage name, a comment that survived stripping —
/// does not count, because it would run nothing. A command spread over a
/// multi-line `sh("""…""")` also does not count, which is a false negative
/// rather than a false pass: the caller fails, and someone moves the command
/// onto one line.
pub fn merge_authority_runs(command: &str) -> bool {
    merge_authority_lines()
        .iter()
        .any(|line| line.contains("sh(") && line.contains(command))
}

/// A failure message naming the authority, the command and what is there
/// instead, so a RED from these contracts is actionable without a diff.
pub fn not_run_message(command: &str, required: &str) -> String {
    let stages: Vec<String> = merge_authority_lines()
        .into_iter()
        .filter_map(|l| {
            l.strip_prefix("stage(\"")
                .and_then(|r| r.find('"').map(|e| r[..e].to_owned()))
        })
        .collect();
    format!(
        "{MERGE_AUTHORITY} does not run `{command}`.\n\
         {required}\n\
         stages present: {}",
        if stages.is_empty() {
            "none".to_owned()
        } else {
            stages.join(", ")
        }
    )
}

/// Every PipelineK script at the repository root.
///
/// This is the set of files that decide what runs, so it is what a scan for
/// "is this suite named by machine-readable configuration" has to look at.
/// While GitHub Actions existed, that set was the `.github/workflows/*.yml`
/// files and the two were both scanned; now the workflows are being retired
/// and the pipelines are the only machine-readable orchestration, so scanning
/// anything else would make the rule quietly cover less than it claims.
pub fn pipeline_paths() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(repo_root())
        .unwrap_or_else(|e| panic!("cannot read the repository root: {e}"))
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()?.to_str()? == "kts").then_some(p)
        })
        .collect();
    out.sort();
    out
}
