//! `cogh::bundled` — Bundled plugins that ship with `cogh`.
//!
// At first release, `cogh` ships with 3 bundled plugins:
//! - \`mcp-server\`: the CogniCode MCP server
//! - \`skills-cognicode-core\`: portable skill bundles
//! - \`sandbox-templates\`: podman container specs
//!
// Embedded via `include_str!` at build time. `cogh init` copies these
//! into `~/.cognicode/plugins/` so the user can immediately run
//! `cogh install mcp-server` without network access.

/// Bundled plugin manifests (raw YAML).
pub const PLUGIN_MANIFESTS: &[(&str, &str)] = &[
    (
        "mcp-server",
        include_str!("bundled/mcp-server.yaml"),
    ),
    (
        "skills-cognicode-core",
        include_str!("bundled/skills-cognicode-core.yaml"),
    ),
    (
        "sandbox-templates",
        include_str!("bundled/sandbox-templates.yaml"),
    ),
];

/// Install all bundled plugins into `~/.cognicode/plugins/<name>/`.
pub fn install_bundled_plugins(home: &std::path::Path) -> anyhow::Result<usize> {
    use anyhow::Context;
    let mut count = 0;
    for (name, yaml) in PLUGIN_MANIFESTS {
        let manifest = crate::manifest::PluginManifest::from_str(yaml)
            .with_context(|| format!("built-in manifest for {name}"))?;
        let plugin_dir = home.join("plugins").join(name);
        std::fs::create_dir_all(&plugin_dir)
            .with_context(|| format!("create {}", plugin_dir.display()))?;
        let target = plugin_dir.join("plugin.yaml");
        std::fs::write(&target, yaml)
            .with_context(|| format!("write {}", target.display()))?;
        eprintln!("  ✓ installed bundled plugin: {name}");
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_manifests_parse() {
        for (name, yaml) in PLUGIN_MANIFESTS {
            let m = crate::manifest::PluginManifest::from_str(yaml)
                .unwrap_or_else(|e| panic!("{name} failed: {e}"));
            assert_eq!(m.name, *name, "manifest name mismatch");
        }
    }
}
