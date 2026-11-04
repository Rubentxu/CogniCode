//! PR-SEC: every `deny.toml` advisory ignore must have a real backing row.
//!
//! ## Why this test exists
//!
//! `deny.toml` is the blocking supply-chain gate for releases: it runs at
//! `release.yml:128` and `release-validate.yml:123`, and `merge-gate` is the
//! only required check on `main`. Its header used to declare that every ignore
//! below it had "a reason and a recorded fix path in
//! `docs/prf/specs/RECONCILIATION-MATRIX.md` and JOURNAL §55", and that
//! "adding a new ignore without a matrix entry is a review failure".
//!
//! Verified 2026-10-01: none of the advisory ids appears in
//! `RECONCILIATION-MATRIX.md`, which carries only a summary sentence claiming
//! "5 deudas documentadas" at line 44. JOURNAL §55 is about the A-013
//! lifecycle gate, not advisories. So the file enforced a rule that its own
//! ignores had broken 5 times out of 5, by pointing at a record that does not
//! contain them. This is the same shape as the N+66 ghost filter and the N+70
//! allowlist rationale: a written guarantee with no mechanism behind it.
//!
//! The backing record is now `docs/debts/DEBT-SEC-001-advisory-ignores.md`.
////! It lives under `docs/debts/` rather than `docs/prf/`, which is frozen
//! historical evidence and must not be reopened.
//!
//! ## What it does NOT assert
//!
//! It does not assert that `deny.toml` is free of the string
//! "RECONCILIATION-MATRIX". It legitimately appears there, in the comment that
//! records the correction and explains that the old pointer was wrong. Asserting
//! the absence of a substring would forbid documenting the fix, which is the
//! opposite of what this test is for. What it asserts is that `deny.toml`
//! positively points at the register, and that the register actually backs
//! every ignore.
//!
//! ## Parsing
//!
//! Deliberately dependency-free. `cognicode-cli` has no TOML parser in
//! `[dev-dependencies]`, and adding one for a config file this small would be a
//! worse trade than a line scanner that fails closed: both the `ignore = [...]`
//! block and the register tables are pinned structurally here, so reformatting
//! either one fails the test rather than silently unpinning it.

use std::path::PathBuf;

/// The backing record. One row per ignore, plus a table of retirements.
const REGISTER: &str = "docs/debts/DEBT-SEC-001-advisory-ignores.md";

/// Number of columns in the register's row tables:
/// id, crate, version, class, why, fix path, owner, authorising record.
const REGISTER_COLUMNS: usize = 8;

/// Columns that must carry real content. Index into [`REGISTER_COLUMNS`].
const REQUIRED_NON_EMPTY: &[usize] = &[0, 1, 2, 4, 5, 6, 7];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Every id in the `ignore = [ ... ]` array of `deny.toml`.
///
/// The array is found by scanning **line by line** for the closing `]`, and
/// comment-only lines are skipped while scanning. Both halves are load-bearing
/// and both were learned the hard way:
///
/// - A `text[start..].find(']')` truncates the array at the first `]` *anywhere*,
///   including inside a comment. Measured 2026-10-01: the CR-07 correction
///   commit put a reproduction command in the `deny.toml` comment block whose
///   regex `grep -o '"vers":"0\.\(28\|29\)[^"]*"[^}]*'` contains a literal `]`.
///   That `]` closed the array early, so `RUSTSEC-2024-0437` — the last real
///   entry — fell outside the parsed region, and this file reported the
///   register and `deny.toml` as incoherent. `merge-gate` caught it as a real
///   red build on PR #321.
/// - Scanning lines and stopping at the first line that is only `]` would not
///   have helped on its own, because the closing bracket sits after comment
///   lines that mention brackets.
///
/// So the contract must survive a `deny.toml` comment that contains brackets,
/// which is a normal thing to write when documenting how to re-check an
/// advisory. A parser that a comment can silently break is the same defect
/// shape this register was created to prevent, one level down.
fn listed_ignores() -> Vec<String> {
    let text = read("deny.toml");
    let start = text
        .find("ignore = [")
        .unwrap_or_else(|| panic!("deny.toml has no `ignore = [` array under [advisories]"));
    let body_start = start + "ignore = [".len();

    // Walk forward line by line, skipping comment-only lines, to find the
    // line that actually closes the array.
    let mut body_end = None;
    let mut cursor = body_start;
    while cursor < text.len() {
        let rest = &text[cursor..];
        let line_end = rest.find('\n').map(|i| cursor + i).unwrap_or(text.len());
        let line = text[cursor..line_end].trim();
        if !line.is_empty() && !line.starts_with('#') && line.starts_with(']') {
            body_end = Some(cursor);
            break;
        }
        cursor = line_end + 1;
    }
    let body_end = body_end.unwrap_or_else(|| {
        panic!("the `ignore = [` array in deny.toml is not closed by a line starting with `]`")
    });

    let mut out = Vec::new();
    for line in text[body_start..body_end].lines() {
        // Each entry reads `"RUSTSEC-…", # reason`, so the id is whatever sits
        // between the first pair of quotes. Splitting on the quote is more
        // robust than trimming punctuation, which the trailing comma defeats.
        let mut parts = line.split('"');
        let _leading = parts.next();
        let Some(id) = parts.next() else {
            continue;
        };
        let id = id.trim();
        if id.is_empty() || !id.starts_with("RUSTSEC-") {
            continue;
        }
        out.push(id.to_string());
    }
    assert!(
        !out.is_empty(),
        "no advisory ids parsed out of the deny.toml ignore array — this contract would be \
         vacuous if the array were emptied or renamed"
    );
    out
}

/// Rows of the `## <heading>` table whose first cell starts with `RUSTSEC-`.
fn register_rows(heading: &str) -> Vec<Vec<String>> {
    let text = read(REGISTER);
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim() == heading)
        .unwrap_or_else(|| panic!("{REGISTER} has no `{heading}` section"));

    let mut rows = Vec::new();
    for line in lines.iter().skip(start + 1) {
        // A new section header ends the table.
        if line.starts_with("## ") {
            break;
        }
        let trimmed = line.trim();
        if !trimmed.starts_with("| RUSTSEC-") {
            continue;
        }
        let cells: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            // Markdown wraps values in backticks; the cell value is the id,
            // not the punctuation around it.
            .map(|c| c.trim().trim_matches('`').trim().to_string())
            .collect();
        rows.push(cells);
    }
    rows
}

fn ids(rows: &[Vec<String>]) -> Vec<String> {
    rows.iter().map(|r| r[0].clone()).collect()
}

#[test]
fn every_listed_ignore_has_a_backing_row() {
    let rows = register_rows("## Current ignores");
    let backed = ids(&rows);

    let missing: Vec<String> = listed_ignores()
        .into_iter()
        .filter(|id| !backed.contains(id))
        .collect();

    assert!(
        missing.is_empty(),
        "deny.toml ignores {missing:?} without a row in {REGISTER} under `## Current \
         ignores`. That is the exact failure this register exists to prevent: the previous \
         header pointed at docs/prf/specs/RECONCILIATION-MATRIX.md, which contains none of \
         these ids. Add the row with crate, version, reason, fix path, owner and the \
         review that authorised it."
    );
}

#[test]
fn every_backing_row_is_well_formed() {
    for heading in ["## Current ignores", "## Retired ignores"] {
        let rows = register_rows(heading);
        assert!(
            !rows.is_empty(),
            "{REGISTER} section `{heading}` has no RUSTSEC rows. If these debts were paid \
             the rows should be removed rather than the section emptied, so an empty table \
             means the register lost its contents."
        );
        for row in &rows {
            assert_eq!(
                row.len(),
                REGISTER_COLUMNS,
                "{REGISTER} row `{}` has {} cells, expected {REGISTER_COLUMNS} \
                 (id, crate, version, class, why, fix path, owner, authorising record). \
                 Every backing row must be complete or it is decoration.",
                row[0],
                row.len(),
            );
            for &i in REQUIRED_NON_EMPTY {
                assert!(
                    !row[i].is_empty() && row[i] != "-",
                    "{REGISTER} row `{}` has an empty column {i}. A debt with no reason, \
                     no fix path, no owner or no authorising review is the same as an \
                     undocumented ignore.",
                    row[0],
                );
            }
        }
    }
}

#[test]
fn the_register_and_deny_toml_agree_in_both_directions() {
    let listed = listed_ignores();
    let current = ids(&register_rows("## Current ignores"));
    let retired = ids(&register_rows("## Retired ignores"));

    for id in &current {
        assert!(
            listed.contains(id),
            "{REGISTER} lists `{id}` as a current ignore, but deny.toml does not ignore it. \
             Either the ignore was removed without retiring the row, or the row is stale."
        );
    }
    for id in &retired {
        assert!(
            !listed.contains(id),
            "{REGISTER} lists `{id}` as retired, but deny.toml still ignores it. A retired \
             advisory that is still in the ignore list is dead weight: it can never match \
             anything again."
        );
    }

    // No id may sit in both tables: that would be an ignore simultaneously
    // claimed as live and as removed.
    for id in &current {
        assert!(
            !retired.contains(id),
            "`{id}` is in both the current and the retired table of {REGISTER}"
        );
    }
}

#[test]
fn the_register_is_not_frozen_prf_evidence() {
    assert!(
        !REGISTER.starts_with("docs/prf/"),
        "the backing register must not live under docs/prf/. That tree is frozen \
         historical evidence of the closed PRF programme; opening it to hold live debt \
         would reopen a programme this repository has declared closed."
    );
    assert!(
        repo_root().join(REGISTER).exists(),
        "{REGISTER} does not exist. deny.toml points at it as the provenance for every \
         ignore; a pointer to a missing file is worse than the false pointer it replaced."
    );
}

#[test]
fn deny_toml_points_its_provenance_at_the_register() {
    let text = read("deny.toml");
    assert!(
        text.contains(REGISTER),
        "deny.toml no longer names {REGISTER} as the backing record for its ignores. \
         Without the pointer the register is orphaned and the ignores become undocumented \
         again, which is the defect this file fixed."
    );
}

#[test]
fn a_dead_ignore_cannot_sit_in_the_list_unnoticed() {
    let text = read("deny.toml");
    assert!(
        text.contains("unused-ignored-advisory = \"deny\""),
        "deny.toml does not set unused-ignored-advisory = \"deny\". Measured 2026-10-01: \
         with the default policy an ignore that matches nothing is only a warning, which is \
         how RUSTSEC-2023-0057 sat in this file while libc had already moved past the \
         affected range. With \"deny\" cargo deny reports error[advisory-not-detected]."
    );
}
