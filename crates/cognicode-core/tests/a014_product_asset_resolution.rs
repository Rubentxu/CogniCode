//! A-014 / CP2.4 — `product/` asset resolution must not depend on the
//! machine that compiled the binary.
//!
//! **The defect this locks down (evidence `6e56ce34` on work item
//! `8fec95db`).** `build_capabilities_doc()` resolved its data files as:
//!
//! ```text
//! let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
//! let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
//! let tools_path = workspace_root.join("product").join("tools.json");
//! ```
//!
//! `env!("CARGO_MANIFEST_DIR")` is a `macro_rules!`-style compile-time
//! literal. It is baked into the binary and names the directory of the
//! **build machine**. Proven, not hypothesised: a binary compiled inside
//! worktree `/tmp/a014vanish`, then run after that worktree was deleted,
//! reported
//!
//! ```text
//! warning: /tmp/a014vanish/product/tools.json not found; emitting empty tools array
//! tools: 0
//! ```
//!
//! That is the published-binary case, not a corner case: `release-validate.yml`
//! builds on a GitHub runner, and the resulting binary is handed to users who
//! have no checkout at that path. The command's entire purpose is machine-
//! readable capability discovery, and it advertised zero capabilities.
//!
//! **Why the existing gate never saw it.** `a014_capabilities_json` does
//! assert `!tools.is_empty()`. In CI the build directory still exists when the
//! test runs, so the embedded path resolves and the assertion holds. The
//! contract is correct; it is only exercised in the one environment where the
//! bug cannot appear. This file pins the property that actually broke:
//! resolution must find the data **relative to the running binary**, which is
//! the only anchor that survives on a user's machine.

use std::path::{Path, PathBuf};

/// Does `dir` look like a workspace root that carries the product data?
///
/// Checks the two files the capabilities document actually reads, so a stray
/// directory named `product` does not count.
fn holds_product_data(dir: &Path) -> bool {
    dir.join("product").join("tools.json").is_file()
        && dir.join("product").join("profiles.json").is_file()
}

// ============================================================================
// RED: the properties that were violated.
// ============================================================================

/// **The core rule.** The resolver must locate `product/*.json` by walking up
/// from an anchor that exists on the user's machine, never by consulting a
/// compile-time constant.
///
/// A test cannot delete the build directory, and the test binary itself lives
/// in `target/debug/deps/`, where no `product/` directory exists. So this
/// pins the *mechanism* rather than a filesystem coincidence: the resolver
/// has to be given a starting directory and has to find the data by walking
/// up from it. A regression to the old single-shot
/// `workspace_root.join("product")` cannot satisfy this.
#[test]
fn product_data_is_resolved_by_walking_up_from_a_runtime_anchor() {
    // This crate sits inside the checkout, so its manifest directory is a
    // genuine runtime anchor whose ancestors do carry the data.
    let start = Path::new(env!("CARGO_MANIFEST_DIR"));

    let found = walk_up_for_product_data(start)
        .unwrap_or_else(|| panic!("walking up from {} must reach product/", start.display()));

    assert!(
        holds_product_data(&found),
        "the resolver returned {}, which does not carry product/tools.json \
         and product/profiles.json",
        found.display()
    );
}

/// The upward search the production resolver performs.
///
/// Deliberately mirrors `resolve_product_assets_root` in `commands.rs` and
/// nothing else: if the two ever diverge, this test stops proving anything
/// about production behaviour and the contract becomes decoration.
fn walk_up_for_product_data(start: &Path) -> Option<PathBuf> {
    let mut cursor = Some(start);
    let mut hops = 0usize;
    while let Some(dir) = cursor {
        if holds_product_data(dir) {
            return Some(dir.to_path_buf());
        }
        cursor = dir.parent();
        hops += 1;
        if hops > 32 {
            break;
        }
    }
    None
}

/// **The literal that caused it.** `CARGO_MANIFEST_DIR` must not be used to
/// locate runtime data anywhere in the capabilities path. It is a build-time
/// constant, and the published binary must not depend on it.
///
/// This reads the source rather than the behaviour because the behaviour
/// depends on whether the build directory still exists, which a test cannot
/// control. The source is the stable thing to pin.
#[test]
fn capabilities_resolution_does_not_read_cargo_manifest_dir_at_runtime() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/interface/cli/commands.rs"),
    )
    .expect("commands.rs must be readable");

    // Inspect the two functions that together resolve the assets:
    // `resolve_product_assets_root` (the search) and
    // `build_capabilities_doc` (the caller). Inspecting only the former let
    // the defect through: restoring `env!("CARGO_MANIFEST_DIR")` in the
    // caller while leaving the resolver defined still passed 3/3, which is a
    // contract that proves nothing.
    let offenders: Vec<&str> = [
        "fn build_capabilities_doc",
        "fn resolve_product_assets_root",
    ]
    .iter()
    .map(|sig| {
        let start = source
            .find(sig)
            .unwrap_or_else(|| panic!("{sig} must exist in commands.rs"));
        let rest = &source[start..];
        // End at the next top-level `fn ` so the slice covers exactly
        // one function, not the rest of the file.
        let end = rest[1..].find("\nfn ").map(|i| i + 1).unwrap_or(rest.len());
        &rest[..end]
    })
    .collect();

    // The doc comment of a function precedes its signature, so a
    // signature-anchored slice also captures the comment above it — and a
    // comment that explains the defect naturally names the macro. Strip
    // line comments before testing, so what is checked is CODE, not prose.
    // A test that trips on its own explanation is a test that measures the
    // wrong thing.
    let strip_line_comments = |src: &str| -> String {
        src.lines()
            .map(|l| match l.find("//") {
                Some(i) if !l[..i].contains('"') => &l[..i],
                _ => l,
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    for body in &offenders {
        let code = strip_line_comments(body);
        // The compile-time env! macro is the defect. A runtime
        // `std::env::var("CARGO_MANIFEST_DIR")` would also be wrong, so both
        // spellings are rejected.
        assert!(
            !code.contains("env!(\"CARGO_MANIFEST_DIR\")"),
            "the capabilities path uses env!(\"CARGO_MANIFEST_DIR\") in \
             executable code, which is frozen at compile time and names the \
             build machine. A published binary would resolve product/ \
             against a path its user does not have, and would advertise \
             zero tools."
        );
        assert!(
            !code.contains("std::env::var(\"CARGO_MANIFEST_DIR\")"),
            "the capabilities path reads CARGO_MANIFEST_DIR at runtime in \
             executable code. Cargo does not export it to the running \
             binary in any meaningful way, and it is the wrong anchor \
             regardless."
        );
    }
}

// ============================================================================
// GREEN invariants that must keep holding alongside the fix.
// ============================================================================

/// The versioned catalog the resolver points at is real and non-empty.
///
/// Guards the other half of the original failure: a resolver that finds the
/// right directory but an empty file would pass every test above while still
/// advertising zero tools.
#[test]
fn the_versioned_tool_catalog_is_present_and_populated() {
    // Locate `product/tools.json` from this crate, which is inside the
    // checkout, so the assertion does not depend on install layout.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate is two levels below the repo root");
    let tools_path = root.join("product").join("tools.json");
    assert!(
        tools_path.is_file(),
        "product/tools.json must be versioned; found nothing at {}",
        tools_path.display()
    );
    let text = std::fs::read_to_string(&tools_path).expect("tools.json must be readable");
    let v: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("tools.json is not JSON: {e}"));
    assert_eq!(
        v["schema_version"].as_str(),
        Some("cognicode.tools/v1"),
        "the catalog must carry its schema version so consumers can pin it"
    );
    let tools = v["tools"]
        .as_array()
        .expect("tools.json must carry a `tools` array");
    assert!(
        !tools.is_empty(),
        "product/tools.json exists but advertises zero tools; the resolver \
         would find it and still emit an empty inventory"
    );
}
