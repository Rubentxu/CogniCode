//! Multi-language workspace inference engine
//!
//! Orchestrates C4 inference across multiple programming languages.

use std::path::Path;

use crate::inference::config_parsers::detect_and_parse;
use crate::inference::config_parsers::go_parser::GoParser;
use crate::inference::config_parsers::python::PythonParser;
use crate::inference::ts_inference::TsInference;
use crate::model::c4_types::{Container, ContainerType, ElementId, SoftwareSystem};
use crate::model::relationships::{C4Relationship, C4RelationshipKind};
use crate::model::workspace::C4Workspace;

/// A language detected in the workspace
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    Go,
    Unknown,
}

/// Multi-language workspace inference engine
#[derive(Debug, Clone)]
pub struct MultiLangEngine {
    /// Detected languages in the workspace
    languages: Vec<Language>,
}

impl MultiLangEngine {
    pub fn new() -> Self {
        Self {
            languages: Vec::new(),
        }
    }

    /// Detect languages in a workspace
    pub fn detect_languages(&mut self, project_dir: &Path) -> Vec<Language> {
        self.languages.clear();

        // Check for Rust
        if project_dir.join("Cargo.toml").exists() {
            self.languages.push(Language::Rust);
        }

        // Check for TypeScript/JavaScript
        let package_json = project_dir.join("package.json");
        if package_json.exists() {
            if let Ok(content) = std::fs::read_to_string(&package_json) {
                if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                    // Check if it's TypeScript or JavaScript
                    let deps = pkg.get("dependencies")
                        .or(pkg.get("devDependencies"))
                        .and_then(|d| d.as_object());

                    let has_ts = deps.map(|d| d.contains_key("typescript")).unwrap_or(false);
                    let has_ts_path = project_dir.join("tsconfig.json").exists();

                    if has_ts || has_ts_path {
                        self.languages.push(Language::TypeScript);
                    } else {
                        self.languages.push(Language::JavaScript);
                    }
                }
            }
        }

        // Check for Python
        if project_dir.join("pyproject.toml").exists()
            || project_dir.join("setup.py").exists()
            || project_dir.join("requirements.txt").exists()
        {
            self.languages.push(Language::Python);
        }

        // Check for Go
        if project_dir.join("go.mod").exists() {
            self.languages.push(Language::Go);
        }

        self.languages.clone()
    }

    /// Infer containers from all languages in the workspace
    pub fn infer_containers(&self, project_dir: &Path) -> Vec<Container> {
        let mut all_containers = Vec::new();

        for lang in &self.languages {
            let containers = match lang {
                Language::Rust => {
                    // Use existing detect_and_parse for Rust
                    detect_and_parse(project_dir).unwrap_or_default()
                }
                Language::TypeScript | Language::JavaScript => {
                    // Use TsInference
                    let ts_inference = TsInference::new();
                    if let Some(container) = ts_inference.infer_container(project_dir).ok().flatten() {
                        vec![container]
                    } else {
                        Vec::new()
                    }
                }
                Language::Python => {
                    // Use PythonParser
                    let pyproject = project_dir.join("pyproject.toml");
                    if pyproject.exists() {
                        PythonParser::parse_pyproject(&pyproject).ok().flatten().into_iter().collect()
                    } else {
                        Vec::new()
                    }
                }
                Language::Go => {
                    // Use GoParser from config_parsers
                    let go_mod = project_dir.join("go.mod");
                    if go_mod.exists() {
                        GoParser::parse_go_mod(&go_mod).ok().flatten().into_iter().collect()
                    } else {
                        Vec::new()
                    }
                }
                Language::Unknown => Vec::new(),
            };
            all_containers.extend(containers);
        }

        // If no containers found, create a default one
        if all_containers.is_empty() {
            all_containers.push(Container {
                id: ElementId::new("container-default"),
                name: project_dir.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Project")
                    .to_string(),
                container_type: ContainerType::Service,
                technology: "Mixed".to_string(),
                description: "Multi-language project".to_string(),
                path: Some(project_dir.to_path_buf()),
                components: Vec::new(),
            });
        }

        all_containers
    }

    /// Infer relationships between containers across languages
    pub fn infer_cross_language_relationships(
        &self,
        containers: &[Container],
    ) -> Vec<C4Relationship> {
        let mut relationships = Vec::new();

        for i in 0..containers.len() {
            for j in (i + 1)..containers.len() {
                let c1 = &containers[i];
                let c2 = &containers[j];

                // Check for FFI relationships
                if let Some(r) = self.detect_ffi_relationship(c1, c2) {
                    relationships.push(r);
                    continue;
                }

                // Check for HTTP/API relationships based on technology
                if let Some(r) = self.detect_http_relationship(c1, c2) {
                    relationships.push(r);
                }
            }
        }

        relationships
    }

    /// Detect FFI relationships between containers
    fn detect_ffi_relationship(
        &self,
        c1: &Container,
        c2: &Container,
    ) -> Option<C4Relationship> {
        let tech1 = c1.technology.to_lowercase();
        let tech2 = c2.technology.to_lowercase();

        // Rust + Python (pyo3 FFI)
        if (tech1.contains("rust") && tech2.contains("python"))
            || (tech1.contains("python") && tech2.contains("rust"))
        {
            return Some(C4Relationship::new(c1.id.clone(), c2.id.clone(), C4RelationshipKind::Calls)
                .with_label("FFI via pyo3")
                .with_technology("pyo3 FFI"));
        }

        // Rust + Go (cgo)
        if (tech1.contains("rust") && tech2.contains("go"))
            || (tech1.contains("go") && tech2.contains("rust"))
        {
            return Some(C4Relationship::new(c1.id.clone(), c2.id.clone(), C4RelationshipKind::Calls)
                .with_label("FFI via cgo")
                .with_technology("cgo FFI"));
        }

        None
    }

    /// Detect HTTP/API relationships between containers
    fn detect_http_relationship(
        &self,
        c1: &Container,
        c2: &Container,
    ) -> Option<C4Relationship> {
        let tech1 = c1.technology.to_lowercase();
        let tech2 = c2.technology.to_lowercase();

        // API Server → Database
        let is_api = |tech: &str| -> bool {
            tech.contains("express")
                || tech.contains("fastify")
                || tech.contains("nestjs")
                || tech.contains("next.js")
                || tech.contains("flask")
                || tech.contains("django")
                || tech.contains("axum")
                || tech.contains("actix")
        };

        let is_db = |tech: &str| -> bool {
            tech.contains("postgres")
                || tech.contains("mysql")
                || tech.contains("sqlite")
                || tech.contains("mongodb")
                || tech.contains("redis")
        };

        if (is_api(&tech1) && is_db(&tech2)) || (is_api(&tech2) && is_db(&tech1)) {
            let (api, db) = if is_api(&tech1) {
                (&c1.id, &c2.id)
            } else {
                (&c2.id, &c1.id)
            };

            return Some(C4Relationship::new(api.clone(), db.clone(), C4RelationshipKind::ReadsFrom)
                .with_label("Reads/Writes data")
                .with_technology("SQL"));
        }

        // Frontend → Backend API
        let is_frontend = |tech: &str| -> bool {
            tech.contains("react")
                || tech.contains("vue")
                || tech.contains("angular")
                || tech.contains("svelte")
                || tech.contains("next.js")
        };

        if is_frontend(&tech1) && is_api(&tech2) {
            return Some(C4Relationship::new(c1.id.clone(), c2.id.clone(), C4RelationshipKind::Calls)
                .with_label("HTTP/REST API calls")
                .with_technology("HTTP"));
        }

        if is_frontend(&tech2) && is_api(&tech1) {
            return Some(C4Relationship::new(c2.id.clone(), c1.id.clone(), C4RelationshipKind::Calls)
                .with_label("HTTP/REST API calls")
                .with_technology("HTTP"));
        }

        None
    }

    /// Build a complete C4 workspace from a multi-language project
    pub fn build_workspace(&self, project_dir: &Path) -> C4Workspace {
        let project_name = project_dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Project")
            .to_string();

        let mut workspace = C4Workspace::new(&project_name);
        workspace.description = format!(
            "Multi-language system ({} languages)",
            self.languages.len()
        );

        // Get containers
        let containers = self.infer_containers(project_dir);

        // Create a system with all containers
        let system = SoftwareSystem {
            id: ElementId::new("sys-main"),
            name: project_name.clone(),
            description: workspace.description.clone(),
            location: crate::model::c4_types::ElementLocation::Internal,
            containers: containers.clone(),
        };
        workspace.model.systems.push(system);

        // Infer cross-language relationships
        let relationships = self.infer_cross_language_relationships(&containers);
        workspace.model.relationships.extend(relationships);

        workspace
    }
}

impl Default for MultiLangEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_languages_single() {
        let temp_dir = TempDir::new().unwrap();

        // Add Cargo.toml (Rust)
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        let mut engine = MultiLangEngine::new();
        let langs = engine.detect_languages(temp_dir.path());

        assert!(langs.contains(&Language::Rust));
    }

    #[test]
    fn test_detect_languages_multi() {
        let temp_dir = TempDir::new().unwrap();

        // Add Cargo.toml (Rust)
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        // Add package.json (JavaScript)
        std::fs::write(
            temp_dir.path().join("package.json"),
            r#"{"name": "frontend", "dependencies": {"react": "^18.0.0"}}"#,
        )
        .unwrap();

        // Add pyproject.toml (Python)
        std::fs::write(temp_dir.path().join("pyproject.toml"), "[project]\nname = \"backend\"").unwrap();

        let mut engine = MultiLangEngine::new();
        let langs = engine.detect_languages(temp_dir.path());

        assert!(langs.contains(&Language::Rust));
        assert!(langs.contains(&Language::JavaScript));
        assert!(langs.contains(&Language::Python));
    }

    #[test]
    fn test_infer_containers_multi_lang() {
        let temp_dir = TempDir::new().unwrap();

        // Add Cargo.toml (Rust)
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"backend\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        // Add package.json (TypeScript)
        std::fs::write(
            temp_dir.path().join("package.json"),
            r#"{
                "name": "frontend",
                "dependencies": {"react": "^18.0.0"},
                "devDependencies": {"typescript": "^5.0.0"}
            }"#,
        )
        .unwrap();

        // Add tsconfig.json
        std::fs::write(
            temp_dir.path().join("tsconfig.json"),
            r#"{"compilerOptions": {"baseUrl": "."}}"#,
        )
        .unwrap();

        let mut engine = MultiLangEngine::new();
        engine.detect_languages(temp_dir.path());
        let containers = engine.infer_containers(temp_dir.path());

        // Should have at least 2 containers
        assert!(containers.len() >= 2);

        // Check technologies
        let techs: Vec<_> = containers.iter().map(|c| c.technology.as_str()).collect();
        assert!(techs.iter().any(|t| t.contains("Rust")));
        assert!(techs.iter().any(|t| t.contains("React") || t.contains("Node")));
    }

    #[test]
    fn test_build_workspace() {
        let temp_dir = TempDir::new().unwrap();

        // Add Cargo.toml (Rust)
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"backend\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        // Add package.json (JavaScript)
        std::fs::write(
            temp_dir.path().join("package.json"),
            r#"{"name": "frontend", "dependencies": {"react": "^18.0.0"}}"#,
        )
        .unwrap();

        let mut engine = MultiLangEngine::new();
        engine.detect_languages(temp_dir.path());
        let workspace = engine.build_workspace(temp_dir.path());

        assert!(!workspace.model.systems.is_empty());
        assert_eq!(workspace.model.systems.len(), 1); // One system with multiple containers
    }
}