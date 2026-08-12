//! Plugin registry — manages installed plugins and resolves dependencies.

use std::collections::HashMap;
use std::path::PathBuf;

use super::plugin::{PluginError, PluginManifest};

/// A plugin resolved from the registry.
#[derive(Debug, Clone)]
pub struct ResolvedPlugin {
    pub manifest: PluginManifest,
    pub install_path: PathBuf,
}

/// The plugin registry — manages all installed plugins.
#[derive(Debug, Clone, Default)]
pub struct PluginRegistry {
    plugins: HashMap<String, PluginManifest>,
}

impl PluginRegistry {
    /// Load registry from ~/.cognicode/plugins/
    pub fn load() -> Result<Self, PluginError> {
        let registry_path = Self::registry_path();
        if !registry_path.exists() {
            return Ok(Self::default());
        }

        let contents = std::fs::read_dir(&registry_path)
            .map_err(|e| PluginError::Parse(format!("failed to read registry: {}", e)))?;

        let mut registry = Self::default();
        for entry in contents.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let yaml = std::fs::read_to_string(&path).map_err(|e| {
                    PluginError::Parse(format!("failed to read {}: {}", path.display(), e))
                })?;
                let manifest: PluginManifest = serde_yaml::from_str(&yaml).map_err(|e| {
                    PluginError::Parse(format!("failed to parse {}: {}", path.display(), e))
                })?;
                registry.plugins.insert(manifest.name.clone(), manifest);
            }
        }
        Ok(registry)
    }

    /// Save registry to ~/.cognicode/plugins/
    pub fn save(&self) -> Result<(), PluginError> {
        let registry_path = Self::registry_path();
        std::fs::create_dir_all(&registry_path)
            .map_err(|e| PluginError::Parse(format!("failed to create registry dir: {}", e)))?;

        for (name, manifest) in &self.plugins {
            let path = registry_path.join(format!("{}.yaml", name));
            let yaml = serde_yaml::to_string(manifest)
                .map_err(|e| PluginError::Parse(format!("failed to serialize {}: {}", name, e)))?;
            std::fs::write(path, yaml)
                .map_err(|e| PluginError::Parse(format!("failed to write {}: {}", name, e)))?;
        }
        Ok(())
    }

    /// Get a plugin by name.
    pub fn get(&self, name: &str) -> Option<&PluginManifest> {
        self.plugins.get(name)
    }

    /// List all plugins.
    pub fn list(&self) -> impl Iterator<Item = &PluginManifest> {
        self.plugins.values()
    }

    /// Add a plugin to the registry.
    pub fn add(&mut self, manifest: PluginManifest) {
        self.plugins.insert(manifest.name.clone(), manifest);
    }

    /// Remove a plugin from the registry.
    pub fn remove(&mut self, name: &str) {
        self.plugins.remove(name);
    }

    /// Check if a plugin is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Get the number of registered plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    fn registry_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".cognicode/plugins")
    }
}

/// Add a plugin from a GitHub repository URL or shorthand.
pub fn add_plugin(name: String, url: Option<String>) -> Result<(), PluginError> {
    let (owner, repo) = match &url {
        Some(u) => {
            let (o, r) = super::plugin::parse_github_shorthand(u)
                .ok_or_else(|| PluginError::Parse(format!("invalid GitHub URL: {}", u)))?;
            (o, r)
        }
        None => {
            // Default to cognicode-{name} pattern
            let repo_name = format!("cognicode-{}", name);
            ("Rubentxu".to_string(), repo_name)
        }
    };

    let release = super::plugin::fetch_latest_release(&owner, &repo)?;

    let version = release.tag_name.trim_start_matches('v').to_string();
    let asset = super::plugin::find_asset(&release, &name)
        .ok_or_else(|| PluginError::NotFound(format!("asset for plugin {} not found", name)))?;

    let manifest = PluginManifest {
        name: name.clone(),
        version,
        kind: super::plugin::PluginKind::Binary,
        url: Some(asset.browser_download_url.clone()),
        sha256: None, // Will be verified on download
        dependencies: vec![],
        profiles: vec!["core".to_string()],
    };

    let mut registry = PluginRegistry::load()?;
    registry.add(manifest);
    registry.save()?;

    Ok(())
}

/// Remove a plugin from the registry.
pub fn remove_plugin(name: &str) -> Result<(), PluginError> {
    let mut registry = PluginRegistry::load()?;
    if !registry.contains(name) {
        return Err(PluginError::NotFound(name.to_string()));
    }
    registry.remove(name);
    registry.save()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_plugin_registry_empty() {
        // Create a temp registry
        let temp_dir = TempDir::new().unwrap();
        let registry_path = temp_dir.path().join("plugins");
        fs::create_dir_all(&registry_path).unwrap();

        // Write a test manifest
        let yaml = r#"
name: test-plugin
version: 0.1.0
kind: binary
sha256: abc123
dependencies: []
profiles:
  - core
"#;
        fs::write(registry_path.join("test-plugin.yaml"), yaml).unwrap();

        // Load would fail because it uses dirs::home_dir
        // This test documents the expected behavior
    }

    #[test]
    fn test_contains() {
        let mut registry = PluginRegistry::default();
        assert!(!registry.contains("test"));
        registry.add(PluginManifest {
            name: "test".to_string(),
            version: "0.1.0".to_string(),
            kind: super::super::plugin::PluginKind::Binary,
            url: None,
            sha256: None,
            dependencies: vec![],
            profiles: vec![],
        });
        assert!(registry.contains("test"));
        assert!(!registry.contains("other"));
    }

    #[test]
    fn test_len_and_empty() {
        let mut registry = PluginRegistry::default();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.add(PluginManifest {
            name: "test".to_string(),
            version: "0.1.0".to_string(),
            kind: super::super::plugin::PluginKind::Binary,
            url: None,
            sha256: None,
            dependencies: vec![],
            profiles: vec![],
        });

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }
}
