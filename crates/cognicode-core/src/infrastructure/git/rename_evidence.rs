//! Git shell-out adapter for the kernel [`RenameEvidencePort`] (E38
//! design D5).
//!
//! Runs `git -C <repo_root> diff --name-status --find-renames=50%
//! <before> <after>` via [`std::process::Command`] with a FIXED argv
//! (`.arg()` composition — no shell, no string interpolation; both
//! revisions arrive as opaque caller strings passed as separate
//! arguments). `R<nnn>\t<old>\t<new>` rows parse into [`FileRename`]s
//! with `similarity = nnn / 100`.
//!
//! Fail-closed (threat matrix rows "Shell/subprocess" and "Git repository
//! selection" + spec "Version control unavailable degrades safely"): ANY
//! failure — git absent, non-repository directory, non-zero exit (unborn
//! or unknown revision), non-UTF-8 output, or a malformed row — degrades
//! to an EMPTY result with a `tracing::warn`. The adapter never invents
//! partial evidence: one malformed row discards the whole diff (strict
//! parse, design D5 rationale).

use std::path::Path;
use std::process::Command;

use crate::domain::evidence_kernel::ports::{FileRename, RenameEvidencePort};

/// Rename similarity git must reach before it reports a rename.
const FIND_RENAMES: &str = "--find-renames=50%";

/// Production adapter resolving `RenameEvidencePort` by shelling out to
/// the `git` CLI (design D5; `git_history.rs` subprocess precedent).
#[derive(Debug, Clone)]
pub struct GitRenameEvidenceAdapter {
    /// Program spawned as git. `"git"` in production; overridable in tests
    /// to reproduce the "git absent from PATH" spawn failure WITHOUT
    /// mutating the process-wide environment (Rust 2024 makes
    /// `std::env::set_var` unsafe and global mutation would race sibling
    /// tests).
    git_program: std::ffi::OsString,
}

impl GitRenameEvidenceAdapter {
    /// Creates the production adapter spawning the system `git`.
    pub fn new() -> Self {
        Self {
            git_program: std::ffi::OsString::from("git"),
        }
    }

    /// Test-only constructor injecting a program path that cannot resolve,
    /// reproducing the git-absent spawn failure deterministically.
    #[cfg(test)]
    fn with_git_program(program: &str) -> Self {
        Self {
            git_program: std::ffi::OsString::from(program),
        }
    }

    /// Resolves rename evidence or degrades to empty (fail-closed).
    ///
    /// Every failure mode below degrades to an EMPTY result plus a
    /// `tracing::warn` — "Version control unavailable degrades safely"
    /// (spec scenario): git absent (spawn error), non-repository
    /// directory / unborn or unknown revision (non-zero exit), non-UTF-8
    /// output, malformed rows (strict parse → whole-result discard).
    fn run(&self, repo_root: &Path, before_rev: &str, after_rev: &str) -> Vec<FileRename> {
        // Fixed argv via `.arg()` composition (threat matrix
        // "Shell/subprocess"): no shell, no string interpolation; the revs
        // are opaque caller strings passed as separate arguments.
        let output = Command::new(&self.git_program)
            .arg("-C")
            .arg(repo_root)
            .arg("diff")
            .arg("--name-status")
            .arg(FIND_RENAMES)
            .arg(before_rev)
            .arg(after_rev)
            .output();

        let output = match output {
            Ok(output) => output,
            Err(error) => {
                tracing::warn!(
                    "Version control unavailable ({error}); rename evidence degrades safely to empty"
                );
                return Vec::new();
            }
        };
        if !output.status.success() {
            tracing::warn!(
                "Version control unavailable for {} (git: {}); rename evidence degrades safely to empty",
                repo_root.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
            return Vec::new();
        }
        let stdout = match std::str::from_utf8(&output.stdout) {
            Ok(stdout) => stdout,
            Err(_) => {
                tracing::warn!(
                    "Version control output is not UTF-8; rename evidence degrades safely to empty"
                );
                return Vec::new();
            }
        };
        match parse_name_status(stdout) {
            Some(renames) => renames,
            None => {
                tracing::warn!(
                    "Version control emitted malformed name-status rows; rename evidence degrades safely to empty"
                );
                Vec::new()
            }
        }
    }
}

impl Default for GitRenameEvidenceAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl RenameEvidencePort for GitRenameEvidenceAdapter {
    fn renames_between(
        &self,
        repo_root: &Path,
        before_rev: &str,
        after_rev: &str,
    ) -> Vec<FileRename> {
        self.run(repo_root, before_rev, after_rev)
    }
}

/// Strict parser for `git diff --name-status` output (design D5).
///
/// `Some(renames)` only when EVERY line is a well-formed name-status row;
/// `None` on the first malformed row — the caller then degrades to an
/// empty result, never partial evidence. Plain `A`/`D`/`M`/… rows are
/// valid output (no rename) and are skipped; `R<nnn>` rows yield
/// [`FileRename`]s; `C<nnn>` copy rows are structurally validated but
/// carry no rename evidence.
fn parse_name_status(stdout: &str) -> Option<Vec<FileRename>> {
    let mut renames = Vec::new();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split('\t');
        let status = fields.next()?;
        match classify_status(status)? {
            StatusRow::Rename { score } => {
                let old_path = path_field(&mut fields)?;
                let new_path = path_field(&mut fields)?;
                if fields.next().is_some() {
                    return None;
                }
                renames.push(FileRename {
                    old_path: old_path.to_string(),
                    new_path: new_path.to_string(),
                    similarity: f64::from(score) / 100.0,
                });
            }
            // Copy rows have rename-row shape but are not rename evidence.
            StatusRow::Copy => {
                let _old = path_field(&mut fields)?;
                let _new = path_field(&mut fields)?;
                if fields.next().is_some() {
                    return None;
                }
            }
            StatusRow::Plain => {
                let _path = path_field(&mut fields)?;
                if fields.next().is_some() {
                    return None;
                }
            }
        }
    }
    Some(renames)
}

/// One classified `--name-status` status token.
enum StatusRow {
    /// `R<nnn>` — a rename with similarity score `nnn` (percent).
    Rename { score: u8 },
    /// `C<nnn>` — a copy (validated like a rename, never evidence).
    Copy,
    /// `A`/`D`/`M`/`T`/`U`/`X`/`B` — single-path row.
    Plain,
}

/// Classifies one status token; `None` marks the row malformed.
fn classify_status(status: &str) -> Option<StatusRow> {
    let mut chars = status.chars();
    let letter = chars.next()?;
    let score_text = chars.as_str();
    match letter {
        // Both R and C rows carry a score; only renames report it.
        'R' | 'C' => {
            let score = parse_score(score_text)?;
            if letter == 'R' {
                Some(StatusRow::Rename { score })
            } else {
                Some(StatusRow::Copy)
            }
        }
        'A' | 'D' | 'M' | 'T' | 'U' | 'X' | 'B' => {
            if score_text.is_empty() {
                Some(StatusRow::Plain)
            } else {
                None // scores ride only R/C rows; anything else is malformed
            }
        }
        _ => None,
    }
}

/// Parses git's rename/copy score: 1–3 ASCII digits, at most 100.
fn parse_score(score_text: &str) -> Option<u8> {
    if score_text.is_empty()
        || score_text.len() > 3
        || !score_text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let score: u8 = score_text.parse().ok()?;
    (score <= 100).then_some(score)
}

/// The next tab field, which must be a non-empty path.
fn path_field<'a>(fields: &mut std::str::Split<'a, char>) -> Option<&'a str> {
    let path = fields.next()?;
    (!path.is_empty()).then_some(path)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;

    // ---------------------------------------------------------------------
    // Throwaway-repo helpers (adapter tests need REAL git; sandbox
    // fixtures are NOT git repos — exploration confirmed).
    // ---------------------------------------------------------------------

    /// Runs git in `repo`, panicking with stderr on failure.
    fn run_git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git CLI is available for adapter tests");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git stdout is UTF-8")
    }

    /// Builds a throwaway repo: commit `old.txt`, then rename it to
    /// `new.txt` in a second commit. Returns the repo and both revs.
    fn repo_with_rename() -> (TempDir, String, String) {
        let repo = tempfile::tempdir().expect("tempdir");
        run_git(repo.path(), &["init", "-q"]);
        run_git(repo.path(), &["config", "user.name", "e38 Test"]);
        run_git(
            repo.path(),
            &["config", "user.email", "e38-test@example.invalid"],
        );
        run_git(repo.path(), &["config", "commit.gpgsign", "false"]);
        fs::write(repo.path().join("old.txt"), "alpha\nbeta\n").expect("write old.txt");
        run_git(repo.path(), &["add", "-A"]);
        run_git(repo.path(), &["commit", "-q", "-m", "first"]);
        let before = run_git(repo.path(), &["rev-parse", "HEAD"]);
        fs::rename(repo.path().join("old.txt"), repo.path().join("new.txt"))
            .expect("rename old.txt");
        run_git(repo.path(), &["add", "-A"]);
        run_git(repo.path(), &["commit", "-q", "-m", "rename"]);
        let after = run_git(repo.path(), &["rev-parse", "HEAD"]);
        (repo, before.trim().to_string(), after.trim().to_string())
    }

    /// Builds a throwaway repo where a file is only MODIFIED between the
    /// two commits (no rename).
    fn repo_with_edit_only() -> (TempDir, String, String) {
        let repo = tempfile::tempdir().expect("tempdir");
        run_git(repo.path(), &["init", "-q"]);
        run_git(repo.path(), &["config", "user.name", "e38 Test"]);
        run_git(
            repo.path(),
            &["config", "user.email", "e38-test@example.invalid"],
        );
        run_git(repo.path(), &["config", "commit.gpgsign", "false"]);
        fs::write(repo.path().join("stay.txt"), "one\n").expect("write");
        run_git(repo.path(), &["add", "-A"]);
        run_git(repo.path(), &["commit", "-q", "-m", "first"]);
        let before = run_git(repo.path(), &["rev-parse", "HEAD"]);
        fs::write(repo.path().join("stay.txt"), "one\ntwo\n").expect("edit");
        run_git(repo.path(), &["add", "-A"]);
        run_git(repo.path(), &["commit", "-q", "-m", "edit"]);
        let after = run_git(repo.path(), &["rev-parse", "HEAD"]);
        (repo, before.trim().to_string(), after.trim().to_string())
    }

    // ---------------------------------------------------------------------
    // Threat matrix rows (design D5) — RED first (task 2.1 hard gate).
    // ---------------------------------------------------------------------

    /// Valid rename detected: the happy path that discriminates a real
    /// implementation from an always-empty fail-closed stub.
    #[test]
    fn adapter_detects_rename_on_throwaway_repo() {
        let (repo, before, after) = repo_with_rename();
        let adapter = GitRenameEvidenceAdapter::new();

        let renames = adapter.renames_between(repo.path(), &before, &after);

        assert_eq!(
            renames,
            vec![FileRename {
                old_path: "old.txt".to_string(),
                new_path: "new.txt".to_string(),
                similarity: 1.0,
            }],
            "identical content renamed between commits must surface as R100 evidence"
        );
    }

    /// Threat matrix "Shell/subprocess": git absent from PATH → empty
    /// evidence (spawn failure degrades safely).
    #[test]
    fn adapter_returns_empty_when_git_is_absent() {
        let adapter = GitRenameEvidenceAdapter::with_git_program("/nonexistent-e38-test/git");

        let renames = adapter.renames_between(Path::new("/tmp"), "HEAD", "HEAD");

        assert!(
            renames.is_empty(),
            "git absence must degrade to empty evidence, never invented rows"
        );
    }

    /// Threat matrix "Git repository selection": a directory with no
    /// repository metadata → empty evidence.
    #[test]
    fn adapter_returns_empty_outside_a_repository() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Precondition: the environment must actually be non-repo (git
        // walks UP from -C; a degenerate TMPDIR inside a repo would make
        // this test meaningless, so fail loudly instead of lying).
        let probe = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(dir.path())
            .output()
            .expect("git CLI is available for adapter tests");
        assert!(
            !probe.status.success(),
            "precondition violated: tempdir unexpectedly resolves to a git repository"
        );

        let adapter = GitRenameEvidenceAdapter::new();
        let renames = adapter.renames_between(dir.path(), "HEAD", "HEAD");
        assert!(renames.is_empty(), "non-repo dir must yield empty evidence");
    }

    /// Threat matrix "Commit state": an unborn repository (git init, no
    /// commits) → empty evidence.
    #[test]
    fn adapter_returns_empty_for_unborn_repository() {
        let repo = tempfile::tempdir().expect("tempdir");
        run_git(repo.path(), &["init", "-q"]);

        let adapter = GitRenameEvidenceAdapter::new();
        let renames = adapter.renames_between(repo.path(), "HEAD", "HEAD");
        assert!(
            renames.is_empty(),
            "unborn rev must degrade to empty evidence"
        );
    }

    /// Valid diff with no rename rows → empty evidence (SUCCESS, not
    /// failure: no rename occurred).
    #[test]
    fn adapter_returns_empty_when_no_rename_occurred() {
        let (repo, before, after) = repo_with_edit_only();
        let adapter = GitRenameEvidenceAdapter::new();

        let renames = adapter.renames_between(repo.path(), &before, &after);

        assert!(
            renames.is_empty(),
            "an edit-only diff carries no rename evidence"
        );
    }

    // ---------------------------------------------------------------------
    // Strict parser (malformed row → fail-closed whole-result, design D5).
    // ---------------------------------------------------------------------

    #[test]
    fn parser_extracts_rename_rows_and_skips_plain_rows() {
        let renames = parse_name_status("M\tstay.txt\nR100\told.txt\tnew.txt\nD\tgone.txt\n")
            .expect("well-formed rows parse");
        assert_eq!(
            renames,
            vec![FileRename {
                old_path: "old.txt".to_string(),
                new_path: "new.txt".to_string(),
                similarity: 1.0,
            }]
        );
    }

    #[test]
    fn parser_scores_partial_renames_and_ignores_copies() {
        let renames = parse_name_status("C90\tcopy_src.txt\tcopy_dst.txt\nR50\tx.txt\ty.txt\n")
            .expect("well-formed rows parse");
        assert_eq!(
            renames,
            vec![FileRename {
                old_path: "x.txt".to_string(),
                new_path: "y.txt".to_string(),
                similarity: 0.5,
            }],
            "C rows are validated but carry no rename evidence"
        );
    }

    #[test]
    fn parser_accepts_empty_diff() {
        assert_eq!(parse_name_status(""), Some(Vec::new()));
    }

    /// Malformed row → the WHOLE result degrades (strict parse: never
    /// partial evidence, design D5).
    #[test]
    fn parser_degrades_to_none_on_malformed_rows() {
        let malformed = [
            // Missing the new path.
            "R100\told.txt\n",
            // No similarity score.
            "R\told.txt\tnew.txt\n",
            // Extra field.
            "R100\told.txt\tnew.txt\textra\n",
            // Non-numeric score.
            "R1x0\told.txt\tnew.txt\n",
            // Score above 100.
            "R150\told.txt\tnew.txt\n",
            // Empty path field.
            "R100\t\tnew.txt\n",
            // Unknown status letter (not a name-status row at all).
            "junk line\n",
            // Status without its path.
            "M\n",
            // Plain row carrying an extra field.
            "M\ta.txt\tb.txt\n",
        ];
        for row in malformed {
            assert!(
                parse_name_status(row).is_none(),
                "malformed row {row:?} must degrade the whole parse to None"
            );
        }
    }

    // ---------------------------------------------------------------------
    // Port contract.
    // ---------------------------------------------------------------------

    #[test]
    fn port_and_adapter_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<GitRenameEvidenceAdapter>();
        assert_send_sync::<Box<dyn RenameEvidencePort>>();
    }
}
