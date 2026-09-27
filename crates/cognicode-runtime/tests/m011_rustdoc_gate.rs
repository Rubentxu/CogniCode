//! Gate for M0.11: the workspace must build documentation with zero warnings.
//!
//! M0.8 closed the mechanically-fixable rustdoc categories. M0.11 closes the
//! remaining `broken_intra_doc_links` drift, which the backlog had carried since
//! 2026-09-26 as an open-ended audit.
//!
//! This file is a gate, not a library. Run it with:
//!
//! ```text
//! cargo test -p cognicode-runtime --test m011_rustdoc_gate
//! ```
//!
//! It is deliberately excluded from the default test profile: `cargo doc` over
//! the whole workspace is expensive, and pinning a doc-warning count to every
//! `cargo test` invocation would tax every developer's inner loop for a
//! maintenance gate that only matters at release time. The gate is run
//! explicitly, in CI, and at cycle close.
//!
//! It parses `cargo metadata` with `serde_json`, which the workspace already
//! depends on from ten crates, so this costs no new build-time dependency in
//! the dependency graph. Three hand-rolled versions of this parsing were
//! discarded first: each one reported a plausible number of crates while
//! silently resolving the wrong set. A gate that cannot see the thing it gates
//! is worse than no gate, because it reports green.

use std::process::Command;

/// Warning categories M0.11 exists to eliminate. The gate counts by category
/// rather than by total, so a drop in these cannot hide a rise in another.
const CATEGORIES: &[&str] = &[
    "unresolved link to",
    "private_intra_doc_links",
    // A public doc linking a private item is broken for exactly the reader who
    // needs it: they follow the link from the published docs and land nowhere.
    "links to private item",
    "redundant explicit link",
    // Matches both `redundant_explicit_links` (lint id in the note) and
    // `redundant explicit link target` (the headline), so either form counts.
    "redundant_explicit_links",
];

/// Sanity floor on the derived workspace set. If this ever drops, the JSON
/// parsing above has broken and the gate is inspecting a subset while claiming
/// to inspect the workspace. Twelve members as of 2026-09-27.
const MIN_EXPECTED_WORKSPACE_CRATES: usize = 12;

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("runtime crate lives at <root>/crates/cognicode-runtime")
        .to_path_buf()
}

/// Workspace members whose documentation the gate covers. Resolved from
/// `cargo metadata`, not hardcoded: the set must track the real workspace so a
/// new crate cannot slip past the gate unexamined.
fn workspace_crate_names(root: &std::path::Path) -> Vec<String> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(root)
        .output()
        .expect("cargo metadata must be runnable");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata emits valid JSON");

    let members = metadata["workspace_members"]
        .as_array()
        .expect("workspace_members is an array");
    let by_id: std::collections::HashMap<&str, &str> = metadata["packages"]
        .as_array()
        .expect("packages is an array")
        .iter()
        .filter_map(|p| Some((p["id"].as_str()?, p["name"].as_str()?)))
        .collect();

    let mut names: Vec<String> = members
        .iter()
        .map(|m| {
            let id = m.as_str().expect("workspace member id is a string");
            *by_id
                .get(id)
                .unwrap_or_else(|| panic!("workspace member {id} has no package entry"))
        })
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
}

#[test]
fn gate_covers_every_workspace_crate() {
    // The gate derives its crate set from `cargo metadata`, so a new workspace
    // member is covered automatically and cannot be silently exempted. What
    // this test guards is the derivation itself: the first version parsed names
    // with `.lines()` over single-line JSON, saw exactly one package, and still
    // reported coverage while eleven crates went unexamined.
    let root = workspace_root();
    let names = workspace_crate_names(&root);

    assert!(
        names.len() >= MIN_EXPECTED_WORKSPACE_CRATES,
        "workspace_crate_names resolved {} crates ({names:?}), expected at least \
         {MIN_EXPECTED_WORKSPACE_CRATES}. The metadata parsing has likely broken: \
         the gate would inspect a subset while claiming to inspect the workspace.",
        names.len()
    );

    // Every member is a path package, so every id is a `path+file://...#name@version`
    // string. If resolution produced a registry-style id, the name lookup would
    // have matched an unrelated dependency instead.
    for name in &names {
        assert!(
            !name.contains(' '),
            "resolved crate name {name:?} looks like a package id, not a name"
        );
    }
}

#[test]
fn workspace_documentation_is_warning_free() {
    let root = workspace_root();
    let output = Command::new("cargo")
        .args(["doc", "--workspace", "--no-deps"])
        .current_dir(&root)
        .output()
        .expect("cargo doc must be runnable");

    // rustdoc reports warnings on stderr but still exits 0. Exit status alone
    // cannot gate this, which is exactly why this test exists.
    let stderr = String::from_utf8_lossy(&output.stderr);

    let all: Vec<&str> = stderr
        .lines()
        .filter(|line| line.starts_with("warning:"))
        .collect();

    let offenders: Vec<&&str> = all
        .iter()
        .filter(|line| CATEGORIES.iter().any(|cat| line.contains(cat)))
        .collect();

    // Also catches a category this gate does not know about. CATEGORIES is an
    // allow-list, so a new rustdoc lint could otherwise accumulate behind a
    // green gate. Scoped to the warning headline only, and to link-related
    // lints: this gate exists for intra-doc links, not for every lint rustdoc
    // will ever emit.
    //
    // The `line.starts_with("warning:")` filter is what keeps this meaningful.
    // rustdoc follows a private-item link with a `= note: this link will
    // resolve properly if you pass --document-private-items` line, which also
    // contains "link" but is an explanation, not a new warning.
    let unrecognised: Vec<&&str> = all
        .iter()
        .filter(|line| line.contains("link"))
        .filter(|line| !CATEGORIES.iter().any(|cat| line.contains(cat)))
        .collect();

    assert!(
        offenders.is_empty() && unrecognised.is_empty(),
        "M0.11 regression: {} rustdoc warning(s) in the gated categories, \
         {} in an ungated link-related category.\n\
         M0.11 closed these at zero; a new one must be fixed in the same commit \
         that introduces it, not deferred to a new backlog entry. A new category \
         also needs adding to CATEGORIES, or this gate under-reports.\n\n{}\n{}\n",
        offenders.len(),
        unrecognised.len(),
        offenders
            .iter()
            .take(20)
            .map(|l| format!("  {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        unrecognised
            .iter()
            .take(20)
            .map(|l| format!("  {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
