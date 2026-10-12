//! B6.W4 — Skills catalog loader + validator.
//!
//! [`load_skills`] reads `product/skills.json` from the same on-disk
//! anchors that [`cognicode_core::interface::catalog::load_tool_catalog`]
//! uses (cwd and its parents, then `CARGO_MANIFEST_DIR` and its parents,
//! bounded by 32 hops). Every skill that names a tool is checked against
//! the catalog: an unknown tool is a hard error, and any `feature_gates`
//! entry must match a real `ToolDescriptor::feature` value in the
//! catalog. The validator is **fail-closed**: a typo never silently
//! passes as if it resolved.
//!
//! ## Why a separate file (not the existing `cmd/skill.rs`)
//!
//! `cmd/skill.rs` validates the *portable* skill bundles under
//! `skills/<bundle>/` (the `cogh skill validate` command). This file
//! validates the *agent-first* skills under `product/skills.json`
//! (consumed by `cogh skills list` and the eval suite). Different
//! inputs, different contract, different failure modes — keeping them
//! apart avoids entangling portable bundle policy with catalog
//! alignment.
//!
//! ## Skill schema (`cognicode.skills/v1`)
//!
//! ```text
//! {
//!   "schema_version": "cognicode.skills/v1",
//!   "skills": [
//!     {
//!       "name": "cognicode-discover",
//!       "description": "...",
//!       "tools_used": ["codebase_map", "query_symbol_index"],
//!       "feature_gates": [],
//!       "evaluation": "evals/discover.json"
//!     },
//!     ...
//!   ]
//! }
//! ```
//!
//! `tools_used` MUST resolve to a real tool name in the catalog;
//! `feature_gates` MUST resolve to a real `feature` value (so a skill
//! can declare the gate without leaking a typo). `evaluation` is the
//! path to the JSON eval file referenced by tests; it is recorded but
//! not parsed here.
//!
//! ## Stability contract (B6.W4)
//!
//! A skill that names a tool with `stability: experimental` is
//! **forbidden** by policy R-B6-W4 ("skills prescriptivas que asumen
//! capacidades experimentales como estables engañan al agente"). The
//! [`validate_skill_against_catalog`] check accepts a `Skill` that uses
//! stable tools only; any experimental tool in `tools_used` triggers
//! [`SkillError::ExperimentalToolAdvertised`]. Same rule for any
//! `feature_gates` entry that is not a real feature value in the
//! catalog: typos like `magic-llm` fail closed.

use std::path::{Path, PathBuf};

use cognicode_core::interface::catalog::ToolDescriptor;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One entry from `product/skills.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skill {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tools_used: Vec<String>,
    #[serde(default)]
    pub feature_gates: Vec<String>,
    #[serde(default)]
    pub evaluation: Option<String>,
}

/// Wire shape of `product/skills.json`.
#[derive(Debug, Clone, Deserialize)]
struct SkillsFile {
    #[allow(dead_code)]
    #[serde(default)]
    schema_version: Option<String>,
    skills: Vec<Skill>,
}

/// Errors raised by [`load_skills`] and [`validate_skill_against_catalog`].
#[derive(Debug, Error)]
pub enum SkillError {
    /// The skills file exists at every anchor but none deserialised.
    #[error("could not read product/skills.json from any on-disk anchor")]
    SkillsFileMissing,
    /// `tools_used` names a tool that the catalog does not know.
    #[error("skill `{skill}` references unknown tool `{tool}` (not in product/tools.json)")]
    UnknownTool { skill: String, tool: String },
    /// `feature_gates` names a feature that the catalog does not have
    /// for any tool. Catches typos like `magic-llm`.
    #[error("skill `{skill}` declares unknown feature gate `{feature}` (no tool in the catalog declares this feature)")]
    UnknownFeature { skill: String, feature: String },
    /// B6.W4 stability contract: a skill that names an experimental tool
    /// is rejected. The agent must not be told to use an experimental
    /// capability as if it were stable.
    #[error("skill `{skill}` advertises experimental tool `{tool}` as stable (stability=experimental); R-B6-W4 forbids this")]
    ExperimentalToolAdvertised { skill: String, tool: String },
}

/// Anchor discovery: cwd and parents first, then `CARGO_MANIFEST_DIR`
/// and parents. Mirrors `cognicode_core::interface::catalog::walk_up`.
///
/// Returned candidates are absolute paths; the first one that exists is
/// the file we read.
fn skills_candidate_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        walk_up(cwd.as_path(), &mut out);
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        walk_up(Path::new(&manifest), &mut out);
    }
    out
}

fn walk_up(start: &Path, out: &mut Vec<PathBuf>) {
    let mut cursor: Option<&Path> = Some(start);
    let mut hops = 0usize;
    while let Some(dir) = cursor {
        out.push(dir.join("product").join("skills.json"));
        cursor = dir.parent();
        hops += 1;
        if hops > 32 {
            break;
        }
    }
}

/// Load every skill declared in `product/skills.json`.
///
/// Returns [`SkillError::SkillsFileMissing`] if no anchor carries a
/// readable file. Returns the parsed [`Skill`] list otherwise (empty
/// is a valid value — an empty `skills` array means "no skills yet").
pub fn load_skills() -> Result<Vec<Skill>, SkillError> {
    for path in skills_candidate_paths() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<SkillsFile>(&text) {
                Ok(file) => return Ok(file.skills),
                Err(_) => continue,
            }
        }
    }
    Err(SkillError::SkillsFileMissing)
}

/// Validate one [`Skill`] against the live catalog.
///
/// Fails closed:
/// - every entry of `tools_used` MUST resolve to a tool name in
///   `catalog`. Unknown tools raise [`SkillError::UnknownTool`].
/// - every entry of `feature_gates` MUST match a `feature` value
///   actually declared by some tool. Unknown feature names raise
///   [`SkillError::UnknownFeature`]. This is the guard that catches
///   typos like `magic-llm` (R-B6-W4).
/// - any tool that is `stability: experimental` in the catalog cannot
///   appear in `tools_used`. This is the B6.W4 stability contract;
///   raising [`SkillError::ExperimentalToolAdvertised`] makes the
///   violation visible at config-load time, not at agent-runtime time.
pub fn validate_skill_against_catalog(
    skill: &Skill,
    catalog: &[ToolDescriptor],
) -> Result<(), SkillError> {
    let known_features: Vec<&str> = catalog
        .iter()
        .filter_map(|t| t.feature.as_deref())
        .collect();

    for tool in &skill.tools_used {
        let Some(descriptor) = catalog.iter().find(|t| &t.name == tool) else {
            return Err(SkillError::UnknownTool {
                skill: skill.name.clone(),
                tool: tool.clone(),
            });
        };
        if descriptor.stability == "experimental" {
            return Err(SkillError::ExperimentalToolAdvertised {
                skill: skill.name.clone(),
                tool: tool.clone(),
            });
        }
    }

    for feature in &skill.feature_gates {
        if !known_features.iter().any(|f| f == feature) {
            return Err(SkillError::UnknownFeature {
                skill: skill.name.clone(),
                feature: feature.clone(),
            });
        }
    }

    Ok(())
}

/// CLI handler for `cogh skills` — list every skill declared in
/// `product/skills.json`, validated against the live catalog.
///
/// `json` controls output shape: false prints a plain text table
/// (one skill per line: `name | tools`), true prints a JSON array.
/// Each skill is validated before listing; a skill that fails
/// validation is printed with `INVALID: <reason>` instead of being dropped,
/// so the operator sees the gap rather than a silent empty list.
pub fn cmd_skills_list(json: bool) -> Result<(), SkillError> {
    let catalog = cognicode_core::interface::catalog::load_tool_catalog();
    let skills = load_skills()?;

    if json {
        // The summary shape: name + first tool, no internal error
        // types. The contract test reads this back to count skills.
        let summaries: Vec<serde_json::Value> = skills
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "description": s.description,
                    "tools_used": s.tools_used,
                    "feature_gates": s.feature_gates,
                    "evaluation": s.evaluation,
                    "valid": validate_skill_against_catalog(s, &catalog).is_ok(),
                })
            })
            .collect();
        let out = serde_json::to_string_pretty(&summaries)
            .unwrap_or_else(|_| "[]".to_string());
        println!("{out}");
    } else {
        println!("Skills ({}):", skills.len());
        for s in &skills {
            let status = match validate_skill_against_catalog(s, &catalog) {
                Ok(()) => "ok".to_string(),
                Err(e) => format!("INVALID: {e}"),
            };
            println!(
                "  - {:<24} tools={:>2}  [{}]",
                s.name,
                s.tools_used.len(),
                status
            );
        }
    }
    Ok(())
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal stable tool descriptor for tests.
    fn stable_tool(name: &str) -> ToolDescriptor {
        ToolDescriptor {
            name: name.into(),
            title: name.into(),
            description: "".into(),
            category: "graph".into(),
            authority: "read".into(),
            stability: "stable".into(),
            estimated_latency_ms: 1,
            requirements: serde_json::Value::Null,
            requirements_status: "satisfied".into(),
            public_profiles: vec![],
            runtime_metadata: serde_json::Value::Null,
            input_schema: serde_json::Value::Null,
            output_schema: serde_json::Value::Null,
            intent_tags: vec![],
            precision_floor: "ast-heuristic".into(),
            feature: None,
            prerequisites: None,
            follow_ups: None,
            required_args: None,
            limitations: None,
            impact_direction_floor: "inferred".into(),
        }
    }

    fn experimental_tool(name: &str) -> ToolDescriptor {
        let mut t = stable_tool(name);
        t.stability = "experimental".into();
        t
    }

    fn feature_gated_tool(name: &str, feature: &str) -> ToolDescriptor {
        let mut t = stable_tool(name);
        t.feature = Some(feature.into());
        t
    }

    #[test]
    fn load_skills_reads_the_file() {
        // Anchor discovery must find the on-disk skills.json that ships
        // with the repo. If this fails, either the file is gone or the
        // walk-up anchor helper is broken.
        let skills = load_skills().expect("skills.json must load");
        assert!(
            skills.iter().any(|s| s.name == "cognicode-discover"),
            "expected cognicode-discover in the loaded skills, got: {:?}",
            skills.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        assert!(skills.iter().any(|s| s.name == "cognicode-navigate"));
        assert!(skills.iter().any(|s| s.name == "cognicode-quality"));
        assert!(skills.iter().any(|s| s.name == "cognicode-refactor"));
    }

    #[test]
    fn validate_skill_against_catalog_rejects_unknown_tool() {
        // Build a skill that names a tool the catalog does not carry.
        // The validator must fail closed at config-load time, not let
        // the agent discover the gap later.
        let skill = Skill {
            name: "broken".into(),
            description: "test".into(),
            tools_used: vec!["nonexistent_tool".into()],
            feature_gates: vec![],
            evaluation: None,
        };
        let catalog = vec![stable_tool("codebase_map")];
        let err = validate_skill_against_catalog(&skill, &catalog)
            .expect_err("unknown tool must be rejected");
        match err {
            SkillError::UnknownTool { tool, skill } => {
                assert_eq!(tool, "nonexistent_tool");
                assert_eq!(skill, "broken");
            }
            other => panic!("expected UnknownTool, got {other:?}"),
        }
    }

    #[test]
    fn validate_skill_against_catalog_rejects_unknown_feature() {
        // Skills must declare only feature gates that match a real
        // `feature` value in the catalog. A typo like `magic-llm` would
        // otherwise be silently accepted.
        let skill = Skill {
            name: "broken".into(),
            description: "test".into(),
            tools_used: vec![],
            feature_gates: vec!["magic-llm".into()],
            evaluation: None,
        };
        let catalog = vec![feature_gated_tool("list_evidence", "evidence-cli-ladybug")];
        let err = validate_skill_against_catalog(&skill, &catalog)
            .expect_err("unknown feature gate must be rejected");
        match err {
            SkillError::UnknownFeature { feature, .. } => {
                assert_eq!(feature, "magic-llm");
            }
            other => panic!("expected UnknownFeature, got {other:?}"),
        }
    }

    #[test]
    fn empty_skills_returns_empty_vec_not_error() {
        // When the file is well-formed but declares zero skills, the
        // loader returns an empty Vec, not an error. This keeps
        // "nothing yet" distinct from "broken file".
        let yaml = r#"{"schema_version":"cognicode.skills/v1","skills":[]}"#;
        let parsed: SkillsFile = serde_json::from_str(yaml).expect("parses");
        assert!(parsed.skills.is_empty());
    }

    #[test]
    fn experimental_tool_in_tools_used_is_rejected() {
        // R-B6-W4: a skill that names an experimental tool is
        // misleading the agent about stability. The validator must
        // refuse, not let the skill pass.
        let skill = Skill {
            name: "leaky".into(),
            description: "test".into(),
            tools_used: vec!["ask_about_code".into()],
            feature_gates: vec![],
            evaluation: None,
        };
        let catalog = vec![experimental_tool("ask_about_code")];
        let err = validate_skill_against_catalog(&skill, &catalog)
            .expect_err("experimental tool must be rejected");
        match err {
            SkillError::ExperimentalToolAdvertised { tool, .. } => {
                assert_eq!(tool, "ask_about_code");
            }
            other => panic!("expected ExperimentalToolAdvertised, got {other:?}"),
        }
    }

    #[test]
    fn stable_skill_with_real_feature_passes() {
        // The success path: stable tools only, a real feature name, the
        // validator is happy. Pins the "happy path" of the contract so a
        // future refactor that over-tightens the rules is caught here.
        let skill = Skill {
            name: "with-feature".into(),
            description: "test".into(),
            tools_used: vec!["list_evidence".into()],
            feature_gates: vec!["evidence-cli-ladybug".into()],
            evaluation: None,
        };
        let catalog = vec![feature_gated_tool("list_evidence", "evidence-cli-ladybug")];
        validate_skill_against_catalog(&skill, &catalog)
            .expect("stable tool with real feature gate must validate");
    }
}