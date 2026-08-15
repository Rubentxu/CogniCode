//! `cogh::skill` — Portable skill bundle management.
//!
// Spec: `openspec/specs/portable-skill-bundle/spec.md`.
//!
// A skill bundle is a directory with:
//! - `SKILL.md` (frontmatter + body)
//! - `manifest.yaml` (cogh metadata)
//! - `references/` (scripts)
//! - `assets/` (data files)
//!
// The skill is IDE-agnostic — no `compatibility: opencode` field.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillManifest {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub maturity: String,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub ide_compatibility: Vec<String>,
    #[serde(default)]
    pub scripts: Vec<String>,
    #[serde(default)]
    pub assets: Vec<String>,
}

fn default_kind() -> String {
    "SkillBundle".to_string()
}

impl SkillManifest {
    /// Parse a `manifest.yaml` from a path.
    pub fn from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read skill manifest {}", path.display()))?;
        Self::from_str(&text)
    }

    /// Parse a YAML string.
    pub fn from_str(s: &str) -> Result<Self> {
        let m: SkillManifest =
            serde_yaml::from_str(s).with_context(|| "parse skill manifest.yaml")?;
        m.validate()?;
        Ok(m)
    }

    /// Validate the manifest.
    pub fn validate(&self) -> Result<()> {
        if self.api_version != "cognicode/v1" {
            return Err(anyhow!(
                "unsupported apiVersion: {} (expected cognicode/v1)",
                self.api_version
            ));
        }
        if self.name.is_empty() {
            return Err(anyhow!("skill name is empty"));
        }
        if !["experimental", "beta", "stable", "deprecated"].contains(&self.maturity.as_str()) {
            return Err(anyhow!(
                "skill {}: maturity must be one of experimental|beta|stable|deprecated (got {})",
                self.name,
                self.maturity
            ));
        }
        Ok(())
    }
}

/// Validate a portable skill bundle directory.
pub fn validate_bundle(path: &Path) -> Result<SkillManifest> {
    if !path.is_dir() {
        return Err(anyhow!(
            "skill bundle path is not a directory: {}",
            path.display()
        ));
    }
    let manifest_path = path.join("manifest.yaml");
    if !manifest_path.exists() {
        return Err(anyhow!(
            "skill bundle missing manifest.yaml: {}",
            path.display()
        ));
    }
    let skill_md = path.join("SKILL.md");
    if !skill_md.exists() {
        return Err(anyhow!("skill bundle missing SKILL.md: {}", path.display()));
    }
    let manifest = SkillManifest::from_path(&manifest_path)?;

    // Check IDE-agnostic: frontmatter must NOT have `compatibility: opencode`
    // (only the YAML frontmatter between the `---` markers).
    let skill_text = std::fs::read_to_string(&skill_md)
        .with_context(|| format!("read {}", skill_md.display()))?;
    let frontmatter = skill_text.split("---").nth(1).unwrap_or("");
    if frontmatter.contains("compatibility: opencode") {
        return Err(anyhow!(
            "skill {} SKILL.md has IDE-specific 'compatibility: opencode' field; \
             portable skill bundles must be IDE-agnostic",
            manifest.name
        ));
    }

    // Check referenced scripts exist
    for script in &manifest.scripts {
        let s = path.join(script);
        if !s.exists() {
            return Err(anyhow!(
                "skill {}: referenced script missing: {}",
                manifest.name,
                s.display()
            ));
        }
    }
    // Check referenced assets exist
    for asset in &manifest.assets {
        let a = path.join(asset);
        if !a.exists() {
            return Err(anyhow!(
                "skill {}: referenced asset missing: {}",
                manifest.name,
                a.display()
            ));
        }
    }

    Ok(manifest)
}

/// CLI handler for `cogh skill validate`.
pub fn cmd_skill_validate(path: &Path) -> Result<()> {
    let m = validate_bundle(path)?;
    println!("✓ skill bundle valid: {} v{}", m.name, m.version);
    println!("  description: {}", m.description);
    println!("  maturity: {}", m.maturity);
    println!("  requires: {:?}", m.requires);
    println!("  ide_compatibility: {:?}", m.ide_compatibility);
    println!("  scripts: {} referenced", m.scripts.len());
    println!("  assets: {} referenced", m.assets.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest() {
        let yaml = r#"
apiVersion: cognicode/v1
name: test
description: minimal
version: "0.92.0"
maturity: stable
"#;
        let m = SkillManifest::from_str(yaml).unwrap();
        assert_eq!(m.name, "test");
        assert_eq!(m.maturity, "stable");
    }

    #[test]
    fn reject_bad_maturity() {
        let yaml = r#"
apiVersion: cognicode/v1
name: test
description: bad
version: "0.92.0"
maturity: "weird"
"#;
        let err = SkillManifest::from_str(yaml).unwrap_err();
        assert!(err.to_string().contains("maturity"));
    }

    #[test]
    fn reject_empty_name() {
        let yaml = r#"
apiVersion: cognicode/v1
name: ""
description: empty
version: "0.92.0"
maturity: stable
"#;
        let err = SkillManifest::from_str(yaml).unwrap_err();
        assert!(err.to_string().contains("name"));
    }

    #[test]
    fn validate_cognicode_mcp_driven_bundle() {
        // The actual portable skill bundle in the repo. Use the
        // workspace root (CARGO_MANIFEST_DIR is the crate dir; we need
        // to go up two levels to reach the repo root).
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
        let path = repo_root.join("skills/cognicode-mcp-driven");
        let m = validate_bundle(&path).expect("bundle must validate");
        assert_eq!(m.name, "cognicode-mcp-driven");
        assert_eq!(m.version, "0.92.0");
        assert!(m.requires.contains(&"mcp-server".to_string()));
    }

    #[test]
    fn reject_ide_specific_field() {
        // Build a temp skill with `compatibility: opencode` in SKILL.md.
        let tmp = std::env::temp_dir().join(format!("cogh-skill-{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("manifest.yaml"),
            "apiVersion: cognicode/v1\nname: x\ndescription: y\nversion: 0.1.0\nmaturity: stable\n",
        )
        .unwrap();
        std::fs::write(
            tmp.join("SKILL.md"),
            "---\nname: x\ncompatibility: opencode\ndescription: y\n---\n",
        )
        .unwrap();
        let err = validate_bundle(&tmp).unwrap_err();
        assert!(err.to_string().contains("compatibility"));
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
