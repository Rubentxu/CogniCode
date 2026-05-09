//! Cargo.toml parser for container inference
//!
//! Parses Cargo.toml files to extract container metadata including
//! crate name, description, technology stack, and dependencies.

use std::path::Path;

use crate::model::c4_types::{Container, ContainerType, ElementId};
use crate::model::relationships::{C4Relationship, C4RelationshipKind};

/// Information extracted from a workspace Cargo.toml
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    /// Workspace name
    pub name: String,
    /// Containers (crates) in the workspace
    pub containers: Vec<Container>,
    /// Relationships between containers
    pub relationships: Vec<C4Relationship>,
}

/// Information extracted from a single crate's Cargo.toml
#[derive(Debug, Clone)]
pub struct CrateInfo {
    /// Crate name
    pub name: String,
    /// Crate description
    pub description: String,
    /// Type of container
    pub container_type: ContainerType,
    /// Technology stack detected from dependencies
    pub technology: Vec<String>,
    /// List of dependency crate names
    pub dependencies: Vec<String>,
}

/// Parser for Cargo.toml files
#[derive(Debug, Clone)]
pub struct CargoParser;

impl CargoParser {
    /// Parse workspace Cargo.toml and infer containers
    pub fn parse_workspace(cargo_toml_path: &Path) -> anyhow::Result<WorkspaceInfo> {
        let content = std::fs::read_to_string(cargo_toml_path)?;
        let value: toml::Value = toml::from_str(&content)?;

        let name = value
            .get("workspace")
            .and_then(|w| w.get("package"))
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("Unknown Workspace")
            .to_string();

        let mut containers = Vec::new();
        let mut relationships = Vec::new();

        // Parse workspace members
        if let Some(members) = value
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(|m| m.as_array())
        {
            // Get workspace directory for resolving member paths
            let workspace_dir = cargo_toml_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Invalid Cargo.toml path"))?;

            for member_path in members {
                if let Some(path_str) = member_path.as_str() {
                    let member_toml = workspace_dir.join(path_str).join("Cargo.toml");

                    if member_toml.exists() {
                        match Self::parse_crate(&member_toml) {
                            Ok(crate_info) => {
                                let container = Self::crate_info_to_container(&crate_info, Some(member_toml.clone()));
                                containers.push(container);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse crate at {:?}: {}", member_toml, e);
                            }
                        }
                    }
                }
            }
        }

        // Infer relationships from dependencies
        relationships = Self::infer_workspace_relationships(&containers);

        Ok(WorkspaceInfo {
            name,
            containers,
            relationships,
        })
    }

    /// Parse a single crate's Cargo.toml
    pub fn parse_crate(cargo_toml_path: &Path) -> anyhow::Result<CrateInfo> {
        let content = std::fs::read_to_string(cargo_toml_path)?;
        let value: toml::Value = toml::from_str(&content)?;

        let package = value
            .get("package")
            .ok_or_else(|| anyhow::anyhow!("Missing [package] section"))?;

        let name = package
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("unknown")
            .to_string();

        let description = package
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();

        // Detect container type from targets
        let container_type = Self::classify_container_type(&value);

        // Detect technology from dependencies
        let dependencies = value.get("dependencies").map(|d| {
            Self::extract_dependencies(d)
        }).unwrap_or_default();

        let technology = Self::detect_technology(value.get("dependencies").unwrap_or(&toml::Value::Array(Vec::new())));

        Ok(CrateInfo {
            name,
            description,
            container_type,
            technology,
            dependencies,
        })
    }

    /// Extract dependency names from a toml value
    fn extract_dependencies(deps: &toml::Value) -> Vec<String> {
        match deps {
            toml::Value::Table(table) => {
                table.keys().cloned().collect()
            }
            toml::Value::Array(arr) => {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// Detect technology stack from dependencies
    fn detect_technology(deps: &toml::Value) -> Vec<String> {
        let mut tech = vec!["Rust".to_string()];

        if let Some(table) = deps.as_table() {
            for key in table.keys() {
                let key_lower = key.to_lowercase();

                // Web frameworks
                if key_lower.contains("actix") || key_lower.contains("axum") || key_lower.contains("rocket") {
                    tech.push("Actix/Axum".to_string());
                }

                // Async runtime
                if key_lower.contains("tokio") || key_lower.contains("async-std") {
                    tech.push("Tokio".to_string());
                }

                // WebAssembly
                if key_lower.contains("wasm") {
                    tech.push("WebAssembly".to_string());
                }

                // Serialization
                if key_lower.contains("serde") {
                    tech.push("Serde".to_string());
                }

                // Database
                if key_lower.contains("sqlx") || key_lower.contains("diesel") || key_lower.contains("rusqlite") {
                    tech.push("Database".to_string());
                }

                // Logging
                if key_lower.contains("tracing") || key_lower.contains("log") {
                    tech.push("Tracing".to_string());
                }
            }
        }

        tech
    }

    /// Classify container type from targets
    fn classify_container_type(crate_info: &toml::Value) -> ContainerType {
        // Check for binary target
        if let Some(bin) = crate_info.get("bin").and_then(|b| b.as_array()) {
            if !bin.is_empty() {
                return ContainerType::Executable;
            }
        }

        // Check for lib target
        if let Some(lib) = crate_info.get("lib") {
            if lib.as_table().map(|t| !t.is_empty()).unwrap_or(false) {
                return ContainerType::Library;
            }
        }

        // Check name patterns
        if let Some(name) = crate_info
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        {
            let name_lower = name.to_lowercase();

            if name_lower.contains("server") || name_lower.contains("service") || name_lower.contains("api") {
                return ContainerType::Service;
            }
            if name_lower.contains("db") || name_lower.contains("store") || name_lower.contains("data") {
                return ContainerType::DataStore;
            }
            if name_lower.contains("cli") || name_lower.contains("cmd") {
                return ContainerType::Executable;
            }
            if name_lower.contains("queue") || name_lower.contains("mq") || name_lower.contains("broker") {
                return ContainerType::Queue;
            }
        }

        // Default to library
        ContainerType::Library
    }

    /// Convert CrateInfo to a Container
    fn crate_info_to_container(crate_info: &CrateInfo, path: Option<std::path::PathBuf>) -> Container {
        Container {
            id: ElementId::new(format!("container-{}", crate_info.name)),
            name: crate_info.name.clone(),
            container_type: crate_info.container_type,
            technology: crate_info.technology.join(", "),
            description: crate_info.description.clone(),
            path,
            components: Vec::new(), // Components filled by L3 inference
        }
    }

    /// Infer relationships between containers based on Cargo.toml dependencies
    fn infer_workspace_relationships(containers: &[Container]) -> Vec<C4Relationship> {
        let mut relationships = Vec::new();

        // Build index of containers by name
        let container_by_name: std::collections::HashMap<&str, &Container> = containers
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();

        // For each container, check if its name matches any dependency of other containers
        // This is a heuristic based on naming conventions
        for container in containers {
            for (dep_name, dep_container) in &container_by_name {
                // Skip self-references
                if container.name.as_str() == *dep_name {
                    continue;
                }

                // Check if container name suggests a dependency relationship
                // e.g., "user-service" depends on "user-repository"
                let container_parts: Vec<&str> = container.name.split('-').collect();
                let dep_parts: Vec<&str> = dep_name.split('-').collect();

                // Check if dep_name is a suffix of container_name
                // e.g., "user-service" -> "user-repository" (repository suffix)
                if container_parts.len() > 1 && dep_parts.len() > 1 {
                    let container_last = container_parts.last().unwrap();
                    let dep_last = dep_parts.last().unwrap();

                    // Pattern: service depends on repository, controller depends on service
                    if *container_last == "service" && *dep_last == "repository" {
                        if container_parts[0] == dep_parts[0] {
                            relationships.push(C4Relationship::new(
                                container.id.clone(),
                                dep_container.id.clone(),
                                C4RelationshipKind::DependsOn,
                            ));
                        }
                    }
                    if *container_last == "controller" && *dep_last == "service" {
                        if container_parts[0] == dep_parts[0] {
                            relationships.push(C4Relationship::new(
                                container.id.clone(),
                                dep_container.id.clone(),
                                C4RelationshipKind::Calls,
                            ));
                        }
                    }
                }
            }
        }

        relationships
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_technology() {
        // Create a dependencies table directly (as Cargo.toml [dependencies] section)
        let mut deps_table = toml::map::Map::new();
        deps_table.insert("tokio".to_string(), toml::Value::String("1.0".to_string()));
        deps_table.insert("serde".to_string(), toml::Value::String("1.0".to_string()));
        let deps = toml::Value::Table(deps_table);

        let tech = CargoParser::detect_technology(&deps);
        assert!(tech.contains(&"Rust".to_string()));
        assert!(tech.contains(&"Tokio".to_string()));
    }

    #[test]
    fn test_classify_container_type() {
        let lib_crate = toml::Value::Table(toml::toml! {
            [package]
            name = "my-library"

            [lib]
            path = "src/lib.rs"
        });

        assert_eq!(
            CargoParser::classify_container_type(&lib_crate),
            ContainerType::Library
        );

        let bin_crate = toml::Value::Table(toml::toml! {
            [package]
            name = "my-binary"

            [[bin]]
            name = "main"
            path = "src/main.rs"
        });

        assert_eq!(
            CargoParser::classify_container_type(&bin_crate),
            ContainerType::Executable
        );

        let service_crate = toml::Value::Table(toml::toml! {
            [package]
            name = "user-service"
        });

        assert_eq!(
            CargoParser::classify_container_type(&service_crate),
            ContainerType::Service
        );
    }
}
