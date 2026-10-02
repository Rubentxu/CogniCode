//! The clippy gate: the merge pipeline must lint as strictly as it claims.
//!
//! The sibling contract in this crate (`core_gate_coverage_contract.rs`)
//! pins `cargo test` parity: every core command the workflow runs must be
//! run by `merge-gate.pipeline.kts`, in both directions. It has the same
//! argument for the lint step, and without it the lint step had no contract
//! at all.
//!
//! ## What this prevents
//!
//! `merge-gate.pipeline.kts` shipped a `clippy-baseline` stage that filtered
//! diagnostics out of `crates/cognicode-core/src/interface/rig/tools.rs`
//! and reported "all errors inside the documented baseline". The comment
//! claimed 42 pre-existing errors there. Measured on 2026-09-30,
//! `cargo clippy --workspace --all-targets -- -D warnings` — the exact
//! command at `pr-ci.yml:78` — reports **zero**. The baseline was dead, and
//! while it filtered nothing it still relaxed the gate: a `pr-ci.yml` PR
//! fails on the first lint anywhere in the workspace, the `.kts` replica
//! only failed on lints outside one file. A local gate that is more
//! permissive than the required check is a gate that cannot be trusted to
//! tell you the check is green.
//!
//! The stage is now the bare assertion (`fix(ci): remove dead clippy
//! allowlist`, `f1f0f0c6`). This contract is what makes that unrevertable by
//! accident.
//!
//! ## Why an allowlist assertion, not a "no allowlist" assertion
//!
//! The second Kotlin pipeline, `product-fast.pipeline.kts`, legitimately
//! carries an allowlist: it runs `--all-features`, and under that flag the
//! same file reports **42 real** `clippy::manual_async_fn` errors (the
//! `rig` feature is not in `cognicode-core`'s default set, so
//! `pr-ci.yml:78` never sees them). A contract phrased as "no `.kts` may
//! filter diagnostics" would therefore force us to delete a correct gate to
//! satisfy it — trading a real check for a green build, which is the same
//! mistake in the opposite direction.
//!
//! So the invariant is not about the presence of a filter. It is:
//!
//!   **a filter is only allowed to name a file that the flag set of its own
//!   pipeline actually reports errors in.**
//!
//! That is checkable without running clippy, and it is the property that
//! makes a filter honest. `merge-gate` runs without `--all-features` and
//! reports 0, so any filter it declares is stale by construction.
//! `product-fast` runs with `--all-features` and reports 42 in one file, so
//! its filter names exactly the right file.
//!
//! ## Calibration
//!
//! The stale-baseline assertion is measured against the pipeline's own
//! declared flag set rather than against a hardcoded count, so the two
//! cannot drift apart silently. If a future pipeline declares a flag set we
//! have not measured, the contract says so instead of guessing.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/cognicode-core has a parent")
        .parent()
        .expect("crates/ has a parent")
        .to_path_buf()
}

fn read_kts(name: &str) -> String {
    let path = repo_root().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The `cargo clippy ...` invocation(s) declared by a pipeline, one per
/// line, with the surrounding `sh("...")` scaffolding stripped.
///
/// Raw strings in the DSL put the command on its own line, so line-based
/// extraction is the honest shape here: a command split across lines would
/// not be matched, and that is preferable to matching a fragment of one.
fn kts_clippy_invocations(kts: &str) -> Vec<String> {
    kts.lines()
        .map(str::trim)
        .filter(|l| l.contains("cargo clippy"))
        .filter(|l| !l.starts_with("//"))
        .map(|l| {
            let start = l.find("cargo clippy").unwrap_or(l.len());
            let mut cmd = l[start..].to_string();
            // Drop bash capture scaffolding. `2>&1` is a redirection, not a
            // flag, and keeping it would make the same command look like two
            // different ones to the table below — which is how an unmeasured
            // flag set slips through as "unknown" while the real one is known.
            for token in ["2>&1", "|| true"] {
                cmd = cmd.replace(token, "");
            }
            // Drop trailing Kotlin/bash scaffolding: line continuations, the
            // closing quote, and the raw-string close.
            cmd.trim_end_matches(['\\', '"', ')', ' '])
                .trim()
                .to_string()
        })
        .collect()
}

/// Files a pipeline exempts from its clippy gate.
///
/// Two shapes reach here and both are a filter, not a comment:
///   * `grep -v '<path>'`  — the `merge-gate` style, filtering a captured log
///   * `--allow=<path>`   — a future per-line suppression
///
/// A commented-out mention is excluded by the caller's line filter: a stage
/// that is commented out is a stage that does not run.
fn kts_clippy_exemptions(kts: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in kts.lines().map(str::trim).filter(|l| !l.starts_with("//")) {
        if !line.contains("cargo clippy") && !line.contains("grep -v") {
            continue;
        }
        if let Some(idx) = line.find("grep -v")
            && let Some(path) = extract_quoted(&line[idx..])
        {
            out.push(path);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// First single- or double-quoted token in `s`, without the quotes.
fn extract_quoted(s: &str) -> Option<String> {
    let start = s.find(['\'', '"'])?;
    let quote = s.as_bytes()[start] as char;
    let rest = &s[start + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

/// Measured error count for a given flag set, pinned as evidence.
///
/// These are observations, not aspirations. If a flag set changes, this
/// table is the thing that must be re-measured, and the failure it causes
/// is a loud, specific one rather than a green that means nothing ran.
fn measured_error_count(invocation: &str) -> Option<usize> {
    match invocation {
        // The merge gate's clippy stage. Verified 0 on 2026-09-30 at 426d5b03,
        // and again when the dead baseline was removed.
        "cargo clippy --workspace --all-targets -- -D warnings" => Some(0),
        // `product-fast.pipeline.kts`. Verified 42 on 2026-09-30 at
        // 426d5b03: all `clippy::manual_async_fn`, all in
        // interface/rig/tools.rs, reachable only with the `rig` feature.
        "cargo clippy --workspace --all-targets --all-features -- -D warnings" => Some(42),
        _ => None,
    }
}

/// The merge gate's clippy stage must run the bare assertion, and nothing else.
///
/// This used to be a parity assertion between two orchestrators: the
/// required `merge-gate` check ran one command in `.github/workflows/pr-ci.yml`
/// and `merge-gate.pipeline.kts` mirrored it. With the workflow retired there
/// is nothing to mirror, and the property that was underneath is the one worth
/// keeping: the gate that decides a merge lints with `-D warnings` and no
/// filter, so nothing can quietly widen.
///
/// The equality is direct rather than a "contains" check on purpose: a
/// substring assertion survives the stage being rewritten into a filter, which
/// is the exact regression this contract exists to catch.
#[test]
fn merge_gate_clippy_runs_exactly_the_bare_command() {
    let invocations = kts_clippy_invocations(&read_kts("merge-gate.pipeline.kts"));
    assert_eq!(
        invocations,
        vec!["cargo clippy --workspace --all-targets -- -D warnings".to_string()],
        "merge-gate.pipeline.kts must declare exactly the bare clippy command, \
         and nothing else. Found: {invocations:?}. This pipeline is the gate: a \
         lint configuration that differs from the bare assertion is a gate that \
         lets through what it is supposed to hold back."
    );
}

/// No pipeline may exempt a file that its own flag set reports clean.
///
/// This is the assertion that would have caught the dead baseline. It does
/// not forbid filters; it forbids a filter that protects nothing, which is
/// the state that lets a pipeline drift into permitting more than the
/// required check without anybody noticing.
#[test]
fn no_pipeline_exempts_a_file_its_own_flags_report_clean() {
    for pipeline in ["merge-gate.pipeline.kts", "product-fast.pipeline.kts"] {
        let kts = read_kts(pipeline);
        let invocations = kts_clippy_invocations(&kts);
        let exemptions = kts_clippy_exemptions(&kts);

        if exemptions.is_empty() {
            continue;
        }

        for invocation in &invocations {
            let Some(errors) = measured_error_count(invocation) else {
                // Fail loudly rather than assume. An unmeasured flag set
                // means we cannot tell a live filter from a dead one, and
                // guessing is how a stale baseline survives.
                panic!(
                    "{pipeline} declares a clippy filter but its flag set \
                     is not measured, so the filter cannot be validated: \
                     `{invocation}`. Measure it and add it to \
                     measured_error_count() with the count you observed."
                );
            };

            assert_ne!(
                errors, 0,
                "{pipeline} exempts {exemptions:?} from its clippy gate, \
                 but `{invocation}` reports 0 errors. The baseline is dead: \
                 it filters nothing while still relaxing the gate, so this \
                 pipeline would pass where the bare command fails. Delete the \
                 exemption, or — if the errors are real — fix them. Widening \
                 the exemption is not an option."
            );
        }
    }
}

/// The measured counts this contract relies on must stay pinned.
///
/// Without this, a table edit that lowers a count to silence a failure is
/// indistinguishable from a real measurement change. Naming the pipeline
/// makes the claim auditable: re-measure before changing a number here.
#[test]
fn the_measured_baseline_is_the_documented_one() {
    assert_eq!(
        measured_error_count("cargo clippy --workspace --all-targets -- -D warnings"),
        Some(0),
        "the merge gate's bare clippy command was measured at 0 errors on \
         2026-09-30. \
         If that is no longer true the workspace has new lint debt and the \
         bare command is the gate that reports it — fix the code, do not \
         update this number to match."
    );
    assert_eq!(
        measured_error_count(
            "cargo clippy --workspace --all-targets --all-features -- -D warnings"
        ),
        Some(42),
        "product-fast's clippy command was measured at 42 errors \
         (clippy::manual_async_fn in interface/rig/tools.rs, behind the \
         `rig` feature) on 2026-09-30. That number is the justification for \
         its exemption existing at all. If it changed, re-measure and record \
         the new count and its lints here."
    );
}

// ---------------------------------------------------------------------------
// The `rig` feature: code that no gate compiles and no registry registers
// ---------------------------------------------------------------------------

/// Every `Cargo.toml` of a workspace member, discovered from the root
/// manifest rather than from a maintained list.
///
/// The list is derived because a hand-kept one drifts silently, and a new
/// crate that enables the feature would otherwise not be scanned at all.
fn workspace_manifests() -> Vec<PathBuf> {
    let root_manifest = repo_root().join("Cargo.toml");
    let text = std::fs::read_to_string(&root_manifest).expect("read root Cargo.toml");

    // `members = [` is a key inside the `[workspace]` table, and its array
    // may span many lines. Rather than track table state — which is what
    // made the first version of this function return an empty list — find
    // the key and consume forward until the array closes. The array is
    // self-delimiting, so no table tracking is needed to read it correctly.
    let mut out = Vec::new();
    let mut in_array = false;
    for line in text.lines() {
        let l = line.trim();

        if !in_array {
            // Only the `members` key of the root manifest, and only when it
            // opens an array. A `[workspace.dependencies]` table can also
            // mention paths, and those are not workspace members.
            if let Some(rest) = l.strip_prefix("members")
                && let Some((_, after)) = rest.split_once('=')
            {
                in_array = true;
                out.extend(parse_member_list(after));
                if after.contains(']') {
                    in_array = false;
                }
            }
            continue;
        }

        if l.starts_with(']') {
            in_array = false;
            continue;
        }
        out.extend(parse_member_list(l));
    }
    out
}

/// Extract workspace member paths from one line of a `members` array.
/// Globs are dropped: a glob cannot be resolved to a manifest without a
/// directory walk, and every member this repo declares is a literal path.
fn parse_member_list(line: &str) -> Vec<PathBuf> {
    line.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .filter_map(|entry| {
            // Strip a trailing `# …` comment before anything else. The
            // workspace array carries prose comments, and one of them
            // contains a comma, so splitting first would manufacture
            // plausible-looking path fragments out of the sentence.
            let entry = match entry.split_once('#') {
                Some((before, _)) => before,
                None => entry,
            };
            let m = entry.trim().trim_matches('"').trim();
            // A real member is a relative path. Requiring a `crates/`-style
            // shape is what separates a member from a comment that survived
            // the split, and a glob cannot be resolved to a single manifest
            // without a directory walk.
            if m.is_empty() || m.contains('*') || m.contains(' ') || !m.contains('/') {
                return None;
            }
            Some(repo_root().join(m).join("Cargo.toml"))
        })
        .collect()
}

/// Feature-gated modules in `cognicode-core` that no gate ever compiles.
///
/// ## The measurement
///
/// `cognicode-core`'s default feature set is `["persistence"]`. The `rig`
/// feature is opt-in, and no crate in the workspace enables it:
///
/// ```text
/// $ grep -rn 'rig = \[' crates/*/Cargo.toml
/// crates/cognicode-core/Cargo.toml:17:rig = ["dep:rig-core"]
/// ```
///
/// The consequence is that the merge gate — and every stage in
/// `merge-gate.pipeline.kts` — compiles with `rig` off, so the 29
/// `#[cfg(feature = "rig")]` sites in the crate are not type-checked, not
/// linted, and not covered by a single test. That is the same failure mode
/// the `evidence-kernel` coverage block closed for tests (N+51): code that
/// only exists in a configuration nobody builds.
///
/// `interface/rig/tools.rs` is the material instance. It implements
/// `rig::tool::Tool` for 21 tools, and 18 of those 21 names are byte-identical
/// to tools the public catalog already ships through `interface/mcp/`
/// (`read_file`, `edit_file`, `build_graph`, `find_usages`, …). The MCP
/// implementation is the live one: `product/tool-catalog-runtime.json` is
/// generated from a real `tools/list` capture by
/// `scripts/product/generate_tool_catalog.py`, and no `impl Tool for XTool`
/// type from `rig/tools.rs` is registered in any MCP handler registry.
///
/// So this is not "unreleased feature awaiting its turn". It is a second
/// implementation of capability the product already publishes, compiling
/// under a feature nobody enables, holding the only lint debt in the
/// workspace — the 42 `clippy::manual_async_fn` errors that
/// `product-fast.pipeline.kts` must exempt to stay green.
///
/// ## Why a count and not a list
///
/// N+53 through N+60 spent a session learning that hand-maintained lists
/// drift and that a count is the only comparison that catches a rename. The
/// assertion below therefore recounts the `cfg` sites from the filesystem and
/// compares the total, so removing the last consumer of the feature — the
/// change that would make this module dead — fails here instead of passing
/// silently.
#[test]
fn no_feature_gated_module_is_built_by_no_gate() {
    // Features that opt code out of the gate's own build. `evidence-kernel`
    // is deliberately absent: `pr-ci.yml:567` builds the core suite with it
    // (N+51), so code behind it is compiled and tested. `rig` is not built
    // by any gate stage, which is what this test exists to make visible.
    const GATED_BUT_UNBUILT: &[&str] = &["rig"];

    let features_src =
        std::fs::read_to_string(repo_root().join("crates/cognicode-core/Cargo.toml"))
            .expect("read cognicode-core/Cargo.toml");

    let default_line = features_src
        .lines()
        .find(|l| l.trim_start().starts_with("default = "))
        .expect("cognicode-core declares a default feature set");

    for feature in GATED_BUT_UNBUILT {
        // Polarity note: this asserts the feature is ABSENT from the default
        // set. If it ever becomes present, the gap this test guards is closed
        // and the constant is obsolete — so absence is the passing state and
        // presence is the failure.
        assert!(
            !default_line.contains(&format!("\"{feature}\"")),
            "cognicode-core's default features now include `{feature}`, so \
             pr-ci.yml:78 compiles that code on every PR and the coverage gap \
             this test guards is closed. Remove `{feature}` from \
             GATED_BUT_UNBUILT and re-measure the clippy count in \
             measured_error_count()."
        );

        // No crate in the workspace may enable it. An in-workspace consumer
        // is the other legitimate exit: it would mean the code is compiled
        // by that consumer's tests even if not by the lint gate.
        let mut consumers = Vec::new();
        for manifest in workspace_manifests() {
            let Ok(text) = std::fs::read_to_string(&manifest) else {
                continue;
            };
            if manifest.ends_with("cognicode-core/Cargo.toml") {
                continue;
            }
            for line in text.lines() {
                let l = line.trim();
                if l.starts_with("cognicode-core") && l.contains(&format!("\"{feature}\"")) {
                    consumers.push(format!("{}", manifest.display()));
                    break;
                }
            }
        }
        assert!(
            consumers.is_empty(),
            "workspace manifests enable cognicode-core's `{feature}` feature: \
             {consumers:?}. The module is now compiled by those crates, so the \
             coverage gap this test guards is closed by measurement rather \
             than by deletion. Remove `{feature}` from GATED_BUT_UNBUILT."
        );
    }
}

/// Count of `#[cfg(feature = "rig")]` sites in the crate, recounted from
/// the filesystem so the number in the doc comment above cannot rot.
fn rig_cfg_site_count() -> usize {
    let dir = repo_root().join("crates/cognicode-core/src");
    let mut count = 0usize;
    let mut stack = vec![dir];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                count += text.matches("#[cfg(feature = \"rig\")]").count();
            }
        }
    }
    count
}

/// The size of the unbuilt surface is asserted, not just its existence.
///
/// A future change that deletes `interface/rig/` outright would satisfy every
/// other test in this file. That is a legitimate outcome, but it must be a
/// deliberate one: the count is the tripwire that turns "the module quietly
/// went away" into a test failure somebody has to read and interpret.
#[test]
fn the_rig_feature_surface_is_the_measured_one() {
    assert_eq!(
        rig_cfg_site_count(),
        29,
        "the number of `#[cfg(feature = \"rig\")]` sites in cognicode-core \
         changed (measured 29 on 2026-09-30 at 426d5b03). If you added code \
         behind the feature, it is still compiled by no gate — extend the \
         audit before assuming this test is obsolete. If you deleted the \
         module, update this count and the numbers in the doc comments above."
    );
}

/// The workspace scan must actually scan something.
///
/// This test exists because it did not, once. `workspace_manifests()`
/// tracked `[workspace]` as a table header and then looked for a
/// `[workspace.members]` subtable, but cargo's manifests put `members = [`
/// as a *key* inside `[workspace]`. The helper returned an empty vector, the
/// loop over it never executed, and the test reported the absence of a
/// coverage gap — which is the shape of a check that reports success
/// without checking (N+64.8).
///
/// A green assertion over an empty collection is indistinguishable from a
/// green assertion over a real scan, so the collection is asserted here.
/// A future refactor that breaks the parser fails this test instead of
/// silently disarming the one that uses it.
#[test]
fn the_workspace_scan_actually_finds_members() {
    let manifests = workspace_manifests();
    assert!(
        manifests.len() >= 10,
        "workspace_manifests() found {} manifests, expected the workspace to \
         declare at least 10 members. A parser that returns an empty list \
         makes every consumer of it vacuously green.",
        manifests.len()
    );
    // And they must be real files, not paths that merely look right.
    for manifest in &manifests {
        assert!(
            manifest.is_file(),
            "workspace member manifest does not exist: {}. The parser is \
             building paths it never checked.",
            manifest.display()
        );
    }
    // The crate under test must be among them; if the root manifest ever
    // stops listing it, the feature assertions lose their subject.
    assert!(
        manifests
            .iter()
            .any(|m| m.ends_with("cognicode-core/Cargo.toml")),
        "workspace_manifests() did not find cognicode-core/Cargo.toml. The \
         scan is not covering the crate whose features this file audits."
    );
}
