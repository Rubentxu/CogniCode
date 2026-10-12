//! B6.W4 — Skills aligned to the live catalog, agent-first evals.
//!
//! This suite pins the contract that every skill declared in
//! `product/skills.json` is coherent with `product/tools.json`:
//!
//! - every `tools_used` entry resolves to a real tool in the catalog
//!   (`every_skill_tools_used_resolves_in_catalog`);
//! - every `feature_gates` entry matches a real `feature` value
//!   actually carried by some tool in the catalog
//!   (`every_skill_feature_gate_is_real`);
//! - every skill's `evaluation` file references tools that exist in
//!   the catalog, so an agent reading an eval cannot be told to use a
//!   tool that does not exist
//!   (`skill_evaluation_references_real_tools`);
//! - no skill advertises an experimental tool as if it were stable
//!   (B6.W4 / R-B6-W4)
//!   (`no_skill_advertises_experimental_feature_as_stable`);
//! - the `cogh skills` runtime command lists every skill with status
//!   `ok` (`cogh_skills_lists_documented_skills`).
//!
//! The contract is **fail-closed**: a missing tool or a typo'd feature
//! gate is a hard error. The negative tests deliberately break one
//! constraint at a time and observe the validator's response.
//!
//! TDD lineage: each test was RED before the corresponding invariant
//! was pinned in `crates/cognicode-cli/src/skills.rs`, GREEN after.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;

use cognicode_cli::skills::{
    Skill, load_skills, validate_skill_against_catalog,
};
use cognicode_core::interface::catalog::{
    ToolDescriptor, load_tool_catalog,
};

// =============================================================================
// Helpers — anchor discovery for product/, evals/, and the cogh binary.
// =============================================================================

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/cognicode-cli has a parent and the workspace has a parent")
        .to_path_buf()
}

/// All names from the live catalog, as a `HashSet` for fast lookup.
fn catalog_names(catalog: &[ToolDescriptor]) -> HashSet<String> {
    catalog.iter().map(|t| t.name.clone()).collect()
}

/// All `feature` values declared by tools in the catalog.
fn catalog_feature_values(catalog: &[ToolDescriptor]) -> HashSet<String> {
    catalog
        .iter()
        .filter_map(|t| t.feature.clone())
        .collect()
}

/// The `cogh` binary built by cargo for this test target.
fn cogh() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cogh"))
}

// =============================================================================
// Contract tests — every skill property is checked against the live
// catalog so a refactor in `tools.json` that breaks a skill is caught.
// =============================================================================

#[test]
fn every_skill_tools_used_resolves_in_catalog() {
    // RED: insert a skill that names a tool the catalog does not
    // carry. The validator rejects it; the on-disk file does not.
    let skills = load_skills().expect("skills.json must load");
    let catalog = load_tool_catalog();
    let names = catalog_names(&catalog);

    assert!(
        !skills.is_empty(),
        "skills.json must declare at least one skill; got an empty list"
    );

    for skill in &skills {
        for tool in &skill.tools_used {
            assert!(
                names.contains(tool),
                "skill `{}` references unknown tool `{}`; \
                 the catalog does not declare it. Either add the tool \
                 to product/tools.json or remove it from the skill.",
                skill.name,
                tool
            );
        }
    }
}

#[test]
fn every_skill_feature_gate_is_real() {
    // RED: insert a `magic-llm` feature gate into any skill. The
    // validator rejects it as a typo; the on-disk file does not.
    let skills = load_skills().expect("skills.json must load");
    let catalog = load_tool_catalog();
    let features = catalog_feature_values(&catalog);

    for skill in &skills {
        for feature in &skill.feature_gates {
            assert!(
                features.contains(feature),
                "skill `{}` declares feature gate `{}` but no tool in \
                 the catalog has this feature. Either add a tool that \
                 declares this feature in product/tools.json or remove \
                 the gate from the skill (typos like `magic-llm` fail \
                 closed by R-B6-W4).",
                skill.name,
                feature
            );
        }
    }
}

#[test]
fn skill_evaluation_references_real_tools() {
    // Every skill's `evaluation` file must reference tools that the
    // catalog actually exposes. An eval that names a non-existent
    // tool would have an agent answer "complete" using a capability
    // that does not exist, which is the kind of gap R-B6-W4 exists
    // to prevent.
    let skills = load_skills().expect("skills.json must load");
    let catalog = load_tool_catalog();
    let names = catalog_names(&catalog);

    for skill in &skills {
        let Some(rel) = skill.evaluation.as_ref() else {
            continue;
        };
        let path = repo_root().join(rel);
        assert!(
            path.exists(),
            "skill `{}` declares evaluation `{}` but the file does not exist at {}",
            skill.name,
            rel,
            path.display()
        );
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

        for tool in &skill.tools_used {
            let needle = format!("\"{tool}\"");
            assert!(
                text.contains(&needle),
                "skill `{}` evaluation `{}` does not mention tool `{}` \
                 (looking for `{needle}` in the JSON). The eval must \
                 exercise the tools the skill advertises.",
                skill.name,
                rel,
                tool
            );
            assert!(
                names.contains(tool),
                "skill `{}` evaluation `{}` references tool `{}` \
                 that is not in the catalog",
                skill.name,
                rel,
                tool
            );
        }
    }
}

#[test]
fn no_skill_advertises_experimental_feature_as_stable() {
    // R-B6-W4: a skill must NEVER name an experimental tool as if it
    // were stable. The validator enforces this directly; this test
    // asserts the on-disk file satisfies the validator.
    let skills = load_skills().expect("skills.json must load");
    let catalog = load_tool_catalog();

    for skill in &skills {
        validate_skill_against_catalog(skill, &catalog)
            .unwrap_or_else(|e| {
                panic!(
                    "skill `{}` fails the B6.W4 stability contract: {e}",
                    skill.name
                )
            });
    }
}

#[test]
fn cogh_skills_lists_documented_skills() {
    // The runtime entry point: `cogh skills` exits 0 and lists every
    // skill declared in `product/skills.json`. This is the contract
    // the agent invokes to discover what the runtime exposes.
    let output = cogh()
        .arg("skills")
        .output()
        .expect("spawn cogh skills");
    assert!(
        output.status.success(),
        "cogh skills exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("cognicode-discover"),
        "cogh skills output should mention cognicode-discover; got: {stdout}"
    );
    assert!(
        stdout.contains("cognicode-navigate"),
        "cogh skills output should mention cognicode-navigate; got: {stdout}"
    );
    assert!(
        stdout.contains("cognicode-quality"),
        "cogh skills output should mention cognicode-quality; got: {stdout}"
    );
    assert!(
        stdout.contains("cognicode-refactor"),
        "cogh skills output should mention cognicode-refactor; got: {stdout}"
    );
    // Every listed skill must be `[ok]`. A single `[INVALID` line
    // means the on-disk file desynchronised from the catalog.
    assert!(
        !stdout.contains("[INVALID"),
        "cogh skills listed at least one INVALID skill; check the catalog alignment: {stdout}"
    );
}

// =============================================================================
// Negative tests — break one constraint at a time and watch the
// validator fail closed. These are the regression guard: a future
// refactor that loosens `validate_skill_against_catalog` falls on
// these.
// =============================================================================

#[test]
fn unknown_tool_is_a_hard_error() {
    // Insert a deliberate orphan: a skill that names `nope_tool`.
    // The validator must refuse it. Catches a regression where the
    // check silently passes an unknown tool.
    let skill = Skill {
        name: "broken".into(),
        description: "negative test".into(),
        tools_used: vec!["nope_tool".into()],
        feature_gates: vec![],
        evaluation: None,
    };
    let catalog = vec![make_tool("codebase_map", "stable", None)];
    let err = validate_skill_against_catalog(&skill, &catalog)
        .expect_err("unknown tool must be rejected");
    assert!(
        format!("{err}").contains("nope_tool"),
        "error must name the offending tool; got: {err}"
    );
}

#[test]
fn unknown_feature_gate_is_a_hard_error() {
    // A `magic-llm` feature gate would otherwise be silently
    // accepted. The validator rejects it.
    let skill = Skill {
        name: "broken".into(),
        description: "negative test".into(),
        tools_used: vec![],
        feature_gates: vec!["magic-llm".into()],
        evaluation: None,
    };
    let catalog = vec![make_tool("list_evidence", "stable", Some("evidence-cli-ladybug"))];
    let err = validate_skill_against_catalog(&skill, &catalog)
        .expect_err("unknown feature gate must be rejected");
    assert!(
        format!("{err}").contains("magic-llm"),
        "error must name the offending feature; got: {err}"
    );
}

#[test]
fn experimental_tool_in_tools_used_is_a_hard_error() {
    // R-B6-W4 enforcement: a skill that uses `ask_about_code`
    // (stability=experimental) must be rejected at config-load
    // time, not surfaced to the agent.
    let skill = Skill {
        name: "leaky".into(),
        description: "negative test".into(),
        tools_used: vec!["ask_about_code".into()],
        feature_gates: vec![],
        evaluation: None,
    };
    let catalog = vec![make_tool("ask_about_code", "experimental", None)];
    let err = validate_skill_against_catalog(&skill, &catalog)
        .expect_err("experimental tool must be rejected");
    assert!(
        format!("{err}").contains("experimental"),
        "error must explain the stability contract; got: {err}"
    );
}

// =============================================================================
// Helpers for negative tests
// =============================================================================

fn make_tool(name: &str, stability: &str, feature: Option<&str>) -> ToolDescriptor {
    ToolDescriptor {
        name: name.into(),
        title: name.into(),
        description: "".into(),
        category: "graph".into(),
        authority: "read".into(),
        stability: stability.into(),
        estimated_latency_ms: 1,
        requirements: serde_json::Value::Null,
        requirements_status: "satisfied".into(),
        public_profiles: vec![],
        runtime_metadata: serde_json::Value::Null,
        input_schema: serde_json::Value::Null,
        output_schema: serde_json::Value::Null,
        intent_tags: vec![],
        precision_floor: "ast-heuristic".into(),
        feature: feature.map(|s| s.into()),
        prerequisites: None,
        follow_ups: None,
        required_args: None,
        limitations: None,
        impact_direction_floor: "inferred".into(),
    }
}