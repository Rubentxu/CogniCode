//! `cogh::lockfile` — `.cognicode.lock` per-project lockfile.
//!
// Spec: docs/specs/cognicode-lifecycle/spec.md §"`cogh install` reads
//! `.cognicode.lock`".
//!
// Format: JSON with shape `{ "plugins": { "<plugin>": "<version>", ... } }`.

//! Lint policy
//!
//! This module is part of the cogh CLI distribution surface.  Items here
//! are referenced from `cogh` and/or `cognicode-release` bin targets, but
//! each individual bin only links a subset; the items that are unused
//! in a given target are still part of the public API of the cogh
//! version manager and of the release factory.  Without the module-level
//! allow, `cargo clippy --workspace --all-targets -- -D warnings` (the
//! CI gate enforced by PRF-CI-01) would fail on every push.
//!
//! The allow is documented and re-anchored: H-06 (A→B update cycle) is
//! the next unit that will add live consumers for these items, at which
//! point the allow can be tightened item by item.  See D34-2 for the
//! historical decision to defer the cleanup out of the PRF program.
#![allow(dead_code)]
#![allow(unused_imports)]

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Lockfile {
    #[serde(default)]
    pub plugins: std::collections::BTreeMap<String, String>,
}

impl Lockfile {
    /// Read the project-root `.cognicode.lock`. Returns `Default` if
    /// the file does not exist.
    pub fn read_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read lockfile {}", path.display()))?;
        let lf: Self = serde_json::from_str(&text)
            .with_context(|| format!("parse lockfile {}", path.display()))?;
        Ok(lf)
    }

    /// Write the lockfile as JSON (atomic: tmp + rename).
    pub fn write(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("lock.tmp");
        let json = serde_json::to_string_pretty(self).context("serialize lockfile")?;
        std::fs::write(&tmp, json)
            .with_context(|| format!("write temp lockfile {}", tmp.display()))?;
        std::fs::rename(&tmp, path)
            .with_context(|| format!("rename lockfile {}", path.display()))?;
        Ok(())
    }

    pub fn pin(&mut self, plugin: &str, version: &str) {
        self.plugins.insert(plugin.to_string(), version.to_string());
    }

    pub fn get(&self, plugin: &str) -> Option<&str> {
        self.plugins.get(plugin).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut lf = Lockfile::default();
        lf.pin("mcp-server", "0.92.0");
        lf.pin("skills-cognicode-core", "0.92.0");
        let tmp = std::env::temp_dir().join(format!("cogh-lock-{}.json", std::process::id()));
        lf.write(&tmp).unwrap();
        let lf2 = Lockfile::read_or_default(&tmp).unwrap();
        assert_eq!(lf, lf2);
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn missing_returns_default() {
        let tmp =
            std::env::temp_dir().join(format!("cogh-lock-missing-{}.json", std::process::id()));
        let lf = Lockfile::read_or_default(&tmp).unwrap();
        assert!(lf.plugins.is_empty());
    }

    #[test]
    fn pin_then_get() {
        let mut lf = Lockfile::default();
        lf.pin("mcp-server", "0.92.0");
        assert_eq!(lf.get("mcp-server"), Some("0.92.0"));
        assert_eq!(lf.get("not-installed"), None);
    }
}
