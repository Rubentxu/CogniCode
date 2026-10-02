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

/// Every `sh(...)` body in `text`, with its byte offset in that text.
///
/// Extracting bodies rather than scanning lines is what lets a command that
/// wraps count. The line-based check this replaced documented the limitation
/// and prescribed moving the command onto one line — the wrong remedy, because
/// it distorts a pipeline to satisfy a checker, and the distortion is invisible
/// to whoever reads the pipeline next. It also diverged: `pipeline_authority.py`
/// had already worked on bodies, and the Rust copies had not caught up, so a
/// contract could fail against a pipeline that does run the command.
///
/// A command outside an `sh(` body still does not count: a `val`, a stage name,
/// or a comment that survived stripping would run nothing.
pub fn sh_bodies_with_offsets(text: &str) -> Vec<(usize, String)> {
    const RAW: &str = "\"\"\"";
    let bytes = text.as_bytes();
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut cursor = 0usize;

    while let Some(offset) = text[cursor..].find("sh(") {
        let after = cursor + offset + 3;
        let rest = text[after..].trim_start();
        let body_start = after + (text[after..].len() - rest.len());

        if rest.starts_with(RAW) {
            let from = body_start + RAW.len();
            let Some(end) = text[from..].find(RAW) else {
                break;
            };
            out.push((from, text[from..from + end].replace("${'$'}", "$")));
            cursor = from + end + RAW.len();
            continue;
        }

        if rest.starts_with('"') {
            let from = body_start + 1;
            let mut i = from;
            let mut end = None;
            while i < bytes.len() {
                match bytes[i] {
                    b'\\' => i += 2,
                    b'"' => {
                        end = Some(i);
                        break;
                    }
                    _ => i += 1,
                }
            }
            let Some(end) = end else { break };
            out.push((
                from,
                text[from..end].replace('\\', "").replace("${'$'}", "$"),
            ));
            cursor = end + 1;
            continue;
        }

        cursor = after;
    }

    out
}

/// The merge authority's source, whole-line comments removed.
///
/// A comment that quotes a command is otherwise indistinguishable from a step
/// that runs it. That cuts both ways: a comment naming a removed gate would
/// satisfy "the gate exists", and a comment explaining why something is
/// pinned would trip "it is pinned".
pub fn merge_authority_text() -> String {
    let path = repo_root().join(MERGE_AUTHORITY);
    let text =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The merge authority's source as trimmed, comment-free lines.
pub fn merge_authority_lines() -> Vec<String> {
    merge_authority_text()
        .lines()
        .map(|l| l.trim().to_owned())
        .collect()
}

/// Whether the merge authority runs `command` inside an `sh(...)` body.
///
/// The `sh(` requirement is what makes this fail-closed. A command quoted
/// anywhere else — a `val`, a stage name, a comment that survived stripping —
/// does not count, because it would run nothing.
pub fn merge_authority_runs(command: &str) -> bool {
    sh_bodies_with_offsets(&merge_authority_text())
        .iter()
        .any(|(_, body)| body.contains(command))
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
