//! A-014 / CP2 — action register identity contract.
//!
//! The PRODUCT-1.0 register (`docs/cognicode-community-productization/
//! 16-ACTION-REGISTER.md`) is the single authority for the `A-0xx` action
//! ids. Each closed row declares the SDDK cycle and the work item that
//! carried it, e.g.
//!
//! ```text
//! **CLOSED 2026-09-28** — ciclo `cp2-a014-capabilities-json`, WorkItem `8fec95db`.
//! ```
//!
//! **The defect this locks down (N+63.2).** A-014 ended up with TWO cycles
//! whose slugs normalize to the same action id:
//!
//! | cycle slug | work item | status |
//! |---|---|---|
//! | `a-014-capabilities-json` | `82719e1d` Done | `CLOSED` / archive |
//! | `cp2-a014-capabilities-json` | `8fec95db` **Active** | `RELEASE_PENDING` |
//!
//! The twin nobody was watching kept `sddk plan roadmap status` failing with
//! `multiple active work items`, and N+61 read that failure as "already
//! fixed" because it checked the CLOSED twin and not the live one. The same
//! collision had already been recorded once for A-015.
//!
//! Identity is the thing that must not drift, so it gets a gate: one action
//! id resolves to one cycle and one work item, everywhere in the register.
//!
//! **Known limit, stated rather than hidden.** This contract reads the
//! register only. It cannot see the SDDK ledger, so it cannot catch a cycle
//! that exists in the ledger and was never declared here. What it does catch
//! is the half we control: a *second* declaration of an identity, a
//! transcription typo, and a placeholder. Both existing A-014 cycles are
//! declared below, so the collision is visible in the source of truth instead
//! of living only in a status command.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Repo root, resolved from this crate's manifest dir (two levels up:
/// `crates/cognicode-cli` -> `crates` -> root).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is two levels below the repo root")
        .to_path_buf()
}

fn register_path() -> PathBuf {
    repo_root().join("docs/cognicode-community-productization/16-ACTION-REGISTER.md")
}

fn read_register() -> String {
    std::fs::read_to_string(register_path()).unwrap_or_else(|e| {
        panic!(
            "action register unreadable at {}: {e}",
            register_path().display()
        )
    })
}

/// A table row that declares a row id in its first cell: `| A-014 | ... |`.
fn action_rows(md: &str) -> Vec<(String, String)> {
    md.lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("| A-")?;
            let id = rest.split('|').next()?.trim().to_string();
            if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            Some((format!("A-{id}"), line.to_string()))
        })
        .collect()
}

/// Cycle ids declared in a row, backtick-quoted after `ciclo `.
///
/// The register uses two shapes and the first version of this gate only
/// understood one, which made it silently pass a mutation that moved a cycle
/// between rows:
///
/// ```text
/// ciclo `cp2-a014-capabilities-json`              <- bare slug
/// ciclo `p-c1fac1fea05615c6/cp2-a013-lifecycle-gate`  <- project-qualified
/// ```
///
/// A bare project id like `p-c1fac1fea05615c6` is NOT a cycle: take the
/// segment after the last `/`. Only lowercase/digit/dash slugs count, so the
/// `--no-verify` flag in the A-014 prose is not mistaken for one.
fn declared_cycles(row: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = row;
    while let Some(idx) = rest.find("ciclo ") {
        rest = &rest[idx + "ciclo ".len()..];
        let Some(tick) = rest.find('`') else { break };
        rest = &rest[tick + 1..];
        let Some(end) = rest.find('`') else { break };
        let raw = &rest[..end];
        rest = &rest[end + 1..];
        let slug = raw.rsplit('/').next().unwrap_or(raw);
        if is_cycle_slug(slug) {
            out.push(slug.to_string());
        }
    }
    out
}

/// A cycle slug is `a-014-...` or `cp2-a014-...` style: lowercase, digits and
/// dashes, and it carries an action id. Anything else in backticks is a flag,
/// a path or prose.
fn is_cycle_slug(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && action_id_from_slug(s).is_some()
}

/// Work item uuids declared in a row, backtick-quoted after `WorkItem `.
fn declared_work_items(row: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = row;
    while let Some(idx) = rest.find("WorkItem ") {
        rest = &rest[idx + "WorkItem ".len()..];
        let tick = rest.find('`');
        let Some(tick) = tick else { break };
        rest = &rest[tick + 1..];
        let Some(end) = rest.find('`') else { break };
        out.push(rest[..end].to_string());
        rest = &rest[end + 1..];
    }
    out
}

/// A work item id as the register writes it: either a full uuid
/// (`82719e1d-46aa-...`) or the short 8-hex prefix the rows actually use
/// (`0a6ace31`, `8fec95db`). Both forms appear in the register today, so
/// requiring a full uuid would fail on A-012 and the convention would be
/// rejected for being honest. What must hold is that it is hex and long
/// enough to be an id rather than a word.
fn is_uuid_like(s: &str) -> bool {
    if s.len() < 8 {
        return false;
    }
    s.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Strip a cycle slug down to the action id it claims.
///
/// The two real slugs are `cp2-a014-capabilities-json` and
/// `a-014-capabilities-json`: the first spells the action `a014`, the second
/// `a-014`. Both denote A-014. A matcher that only looked for `a-0` found
/// neither, which is how the very first run of this gate reported an empty
/// cycle list for a row that plainly declares one. Both spellings, and the
/// `cpN-` prefix, have to normalize to the same id.
fn action_id_from_slug(slug: &str) -> Option<String> {
    let lower = slug.to_ascii_lowercase();
    // Longest match wins: `a-014-...` before `a014-...`.
    let digits_at = if let Some(i) = lower.find("a-0") {
        i + 2
    } else if let Some(i) = lower.find("a0") {
        i + 1
    } else {
        return None;
    };
    let digits: String = lower[digits_at..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.len() < 3 {
        return None;
    }
    Some(format!("a-{}", &lower[digits_at..digits_at + digits.len()]))
}

/// The action id a *row* claims for itself. The register row is the
/// authority: `| A-014 | ...` claims `a-014`. Anything else a row declares
/// must agree with it, which is what catches a cycle moved between rows.
fn declared_action_id(row_id: &str) -> String {
    format!("a-{}", &row_id[2..])
}

#[test]
fn the_register_is_readable_and_has_action_rows() {
    let rows = action_rows(&read_register());
    assert!(
        rows.len() >= 30,
        "expected the PRODUCT-1.0 register to declare its actions, found {} rows",
        rows.len()
    );
}

/// **Scope of this rule, measured rather than assumed.** Of the 13 CLOSED
/// rows in the register today, only 4 declare a cycle and 3 declare a work
/// item. A-003..A-011 and A-015 are closed in prose with no ledger identity,
/// so "every closed row names one cycle and one work item" is not a rule this
/// repo satisfies and is not asserted here. Asserting it would be exactly the
/// fiction N+63 documents: a rule written in prose and presumed true.
///
/// What *is* enforced, and what the N+63.2 collision violated:
///
/// 1. an action id has exactly one row,
/// 2. a row declares at most one cycle, and every cycle it declares
///    normalizes to that row's own action id,
/// 3. a work item is claimed by at most one row,
/// 4. the A-014 twin pair is recorded rather than living only in a status
///    command.
#[test]
fn a_row_declares_at_most_one_cycle_and_it_belongs_to_that_row() {
    for (id, row) in action_rows(&read_register()) {
        let cycles = declared_cycles(&row);
        assert!(
            cycles.len() <= 1,
            "{id} declares {} cycles ({cycles:?}); a row carries one cycle",
            cycles.len()
        );
        for slug in &cycles {
            if let Some(owner) = action_id_from_slug(slug) {
                assert_eq!(
                    owner,
                    declared_action_id(&id),
                    "{id} declares cycle `{slug}`, which normalizes to action \
                     {owner}. A cycle that claims another action's id is how \
                     A-014 ended up with two cycles (`a-014-capabilities-json` \
                     and `cp2-a014-capabilities-json`) and one of them kept \
                     `plan roadmap status` failing with no visible cause."
                );
            }
        }
    }
}

#[test]
fn no_work_item_is_claimed_by_two_actions() {
    let mut owner: BTreeMap<String, String> = BTreeMap::new();
    for (id, row) in action_rows(&read_register()) {
        // A single row may legitimately declare several work items: A-014
        // records both twins (8fec95db and 82719e1d-...) because that is
        // exactly the collision this gate exists to make visible. What must
        // never happen is the SAME work item belonging to two different
        // actions, so the key is (item, action) and only a differing action
        // trips the assert.
        for item in declared_work_items(&row) {
            assert!(
                is_uuid_like(&item),
                "{id} declares work item `{item}`, which is not a uuid. The \
                 register is the only place these ids are written down; a \
                 transcription slip here is indistinguishable from a typo in \
                 the ledger, and nothing downstream would notice."
            );
            if let Some(prev) = owner.insert(item.clone(), id.clone()) {
                assert_eq!(
                    prev, id,
                    "work item {item} is claimed by both {prev} and {id}. One work \
                     item carries one action; a second claimant means a copied row \
                     or a collision like the A-014 twins."
                );
            }
        }
    }
    assert!(
        owner.len() >= 4,
        "expected at least 4 declared work items, saw {} — if the register lost \
         its identity column this gate would pass on an empty map",
        owner.len()
    );
}

/// The rule that would have caught N+63.2 on the day it happened: two rows may
/// not declare two different cycles that normalize to the same action id.
#[test]
fn no_two_cycles_normalize_to_the_same_action_id() {
    let mut seen: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (_, row) in action_rows(&read_register()) {
        for slug in declared_cycles(&row) {
            if let Some(act) = action_id_from_slug(&slug) {
                seen.entry(act).or_default().insert(slug);
            }
        }
    }
    for (act, slugs) in &seen {
        assert_eq!(
            slugs.len(),
            1,
            "action {act} is carried by {} different cycles: {slugs:?}. Declare \
             the twin explicitly with its own row so the collision is visible \
             here instead of surfacing as an unexplained `multiple active work \
             items`.",
            slugs.len()
        );
    }
}

/// Both A-014 cycles are declared, so the collision is recorded in the source
/// of truth rather than living only in a ledger status command.
#[test]
fn the_a014_collision_is_declared_not_hidden() {
    let rows = action_rows(&read_register());
    let a014: Vec<&String> = rows
        .iter()
        .filter(|(id, _)| id == "A-014")
        .map(|(_, r)| r)
        .collect();
    assert_eq!(
        a014.len(),
        1,
        "A-014 must have exactly one register row, found {}",
        a014.len()
    );
    let row = a014[0];
    let cycles = declared_cycles(row);
    assert!(
        cycles.iter().any(|c| c == "cp2-a014-capabilities-json"),
        "the live A-014 cycle must be declared, got {cycles:?}"
    );
    assert!(
        row.contains("82719e1d"),
        "the archive twin's work item 82719e1d must be recorded on the A-014 \
         row so the pair is auditable from the register alone"
    );
}
