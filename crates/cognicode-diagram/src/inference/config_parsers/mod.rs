//! Config parsers for different build systems
//!
//! Auto-detects the build system and delegates to the appropriate parser.

pub mod cargo;
pub mod docker;
pub mod nodejs;
pub mod python;

pub use cargo::{CargoParser, CrateInfo, WorkspaceInfo};

use std::path::Path;

/// Auto-detected build system type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildSystem {
    /// Cargo/Rust project
    Cargo,
    /// Cargo workspace
    CargoWorkspace,
    /// Unknown build system
    Unknown,
}

/// Detect which build system a project uses based on config files
pub fn detect_build_system(project_dir: &Path) -> BuildSystem {
    // Check for Cargo.toml
    let cargo_toml = project_dir.join("Cargo.toml");

    if !cargo_toml.exists() {
        return BuildSystem::Unknown;
    }

    // Try to parse as workspace or regular crate
    if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
        if let Ok(value) = content.parse::<toml::Value>() {
            // Check if it's a workspace
            if value.get("workspace").is_some() {
                return BuildSystem::CargoWorkspace;
            }
            // Check if it's a package
            if value.get("package").is_some() {
                return BuildSystem::Cargo;
            }
        }
    }

    BuildSystem::Unknown
}

/// Parse project configuration and return workspace info
pub fn parse_project(project_dir: &Path) -> anyhow::Result<Option<WorkspaceInfo>> {
    match detect_build_system(project_dir) {
        BuildSystem::CargoWorkspace => {
            let cargo_toml = project_dir.join("Cargo.toml");
            Ok(Some(CargoParser::parse_workspace(&cargo_toml)?))
        }
        BuildSystem::Cargo => {
            let cargo_toml = project_dir.join("Cargo.toml");
            let crate_info = CargoParser::parse_crate(&cargo_toml)?;

            let container = Container {
                id: ElementId::new(format!("container-{}", crate_info.name)),
                name: crate_info.name.clone(),
                container_type: crate_info.container_type,
                technology: crate_info.technology.join(", "),
                description: crate_info.description.clone(),
                path: Some(cargo_toml),
                components: Vec::new(),
            };

            Ok(Some(WorkspaceInfo {
                name: crate_info.name,
                containers: vec![container],
                relationships: Vec::new(),
            }))
        }
        BuildSystem::Unknown => Ok(None),
    }
}

// Re-export Container and ElementId for use in this module
use crate::model::c4_types::{Container, ElementId};

/// Auto-detect and parse containers from any supported project
///
/// Detects the build system (Cargo, Node.js, Python) and parses
/// the appropriate configuration to infer containers.
pub fn detect_and_parse(project_dir: &Path) -> anyhow::Result<Vec<Container>> {
    let mut containers = Vec::new();

    // Try Cargo workspace/crate
    if let Ok(Some(workspace)) = parse_project(project_dir) {
        containers.extend(workspace.containers);
    }

    // Try Node.js package.json
    let package_json = project_dir.join("package.json");
    if package_json.exists() {
        if let Some(c) = nodejs::NodeJsParser::parse_package_json(&package_json)? {
            containers.push(c);
        }
    }

    // Try Python pyproject.toml
    let pyproject = project_dir.join("pyproject.toml");
    if pyproject.exists() {
        if let Some(c) = python::PythonParser::parse_pyproject(&pyproject)? {
            containers.push(c);
        }
    }

    // Try Python setup.py (legacy)
    let setup_py = project_dir.join("setup.py");
    if setup_py.exists() {
        if let Some(c) = python::PythonParser::parse_setup_py(&setup_py)? {
            containers.push(c);
        }
    }

    Ok(containers)
}
