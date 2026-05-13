//! TypeScript/JavaScript inference for C4 diagrams
//!
//! Provides L1-L3 inference for TypeScript and JavaScript projects.

use std::path::Path;

use cognicode_core::domain::aggregates::call_graph::CallGraph;

use crate::inference::config_parsers::{
    nodejs::NodeJsParser, tsconfig::TsConfigInfo,
    detect_nextjs, detect_react, JsProjectType,
};
use crate::model::c4_types::{Container, ElementId, Component, ComponentType, ElementLocation, Person};

/// TypeScript/JavaScript inference engine
#[derive(Debug, Clone)]
pub struct TsInference {
    /// Whether to include React components
    pub include_react: bool,
    /// Whether to include Next.js specific structures
    pub include_nextjs: bool,
}

impl TsInference {
    pub fn new() -> Self {
        Self {
            include_react: true,
            include_nextjs: true,
        }
    }

    /// Infer a TypeScript/JavaScript container from project
    pub fn infer_container(&self, project_dir: &Path) -> anyhow::Result<Option<Container>> {
        let package_json = project_dir.join("package.json");
        if !package_json.exists() {
            return Ok(None);
        }

        // Parse package.json
        let mut container = NodeJsParser::parse_package_json(&package_json)?
            .ok_or_else(|| anyhow::anyhow!("Failed to parse package.json"))?;

        // Enrich with TypeScript info
        let tsconfig = project_dir.join("tsconfig.json");
        if tsconfig.exists() {
            if let Ok(_info) = TsConfigInfo::parse(&tsconfig) {
                // Add TypeScript technology
                if !container.technology.contains("TypeScript") {
                    container.technology = format!("{}, TypeScript", container.technology);
                }

                // Detect project type
                let project_type = JsProjectType::detect(project_dir);
                container.description = format!(
                    "{} ({:?})",
                    container.description,
                    project_type
                );
            }
        }

        Ok(Some(container))
    }

    /// Infer components from a TypeScript/JavaScript project
    pub fn infer_components(&self, project_dir: &Path, _call_graph: &CallGraph) -> Vec<Component> {
        let mut components = Vec::new();

        // Detect project structure
        let is_nextjs = detect_nextjs(project_dir);
        let is_react = detect_react(project_dir);

        // Infer components based on project structure
        if is_nextjs {
            components.extend(self.infer_nextjs_components(project_dir));
        } else if is_react {
            components.extend(self.infer_react_components(project_dir));
        } else {
            // Generic JS/TS project - infer from directory structure
            components.extend(self.infer_generic_components(project_dir));
        }

        components
    }

    /// Infer actors (L1) from TypeScript/JavaScript project dependencies
    ///
    /// Detects actors based on package.json dependencies:
    /// - Frontend frameworks (react, vue, angular) → End User (web UI)
    /// - Backend frameworks (express, fastify, next) → End User (API)
    /// - Full-stack frameworks (next, nuxt) → End User
    pub fn infer_actors(&self, project_dir: &Path) -> Vec<Person> {
        let mut actors = Vec::new();
        let deps = self.parse_package_dependencies(project_dir);

        let dep_names: Vec<&str> = deps.iter().map(|(n, _)| n.as_str()).collect();

        // End User actor: frontend frameworks
        let frontend_frameworks = ["react", "react-dom", "vue", "@vue/core", "angular", "@angular/core", "svelte"];
        if dep_names.iter().any(|d| frontend_frameworks.contains(d)) {
            actors.push(Person {
                id: ElementId::new("actor_end_user"),
                name: "End User".to_string(),
                description: "Web application user interacting via browser".to_string(),
                location: ElementLocation::External,
            });
        }

        // End User actor: backend/API frameworks (express, fastify, etc.)
        let api_frameworks = ["express", "fastify", "koa", "hapi", "@hapi/hapi", "nestjs", "@nestjs/core"];
        if dep_names.iter().any(|d| api_frameworks.contains(d)) {
            // Only add if we don't already have end user
            if !actors.iter().any(|a| a.name == "End User") {
                actors.push(Person {
                    id: ElementId::new("actor_end_user"),
                    name: "End User".to_string(),
                    description: "API consumer or service user".to_string(),
                    location: ElementLocation::External,
                });
            }
        }

        // Next.js implies both frontend and potential API
        if dep_names.iter().any(|d| *d == "next") {
            // Next.js is full-stack, user is already detected as End User above
            // Add API consumer for Next.js API routes
            if !actors.iter().any(|a| a.id.as_str() == "actor_api_consumer") {
                actors.push(Person {
                    id: ElementId::new("actor_api_consumer"),
                    name: "API Consumer".to_string(),
                    description: "External service consuming Next.js API routes".to_string(),
                    location: ElementLocation::External,
                });
            }
        }

        // Database clients suggest data access patterns
        let db_clients = ["mongoose", "@prisma/client", "typeorm", "sequelize", "pg", "mysql", "mysql2", "better-sqlite3", "redis", "ioredis"];
        if dep_names.iter().any(|d| db_clients.contains(d)) {
            if !actors.iter().any(|a| a.id.as_str() == "actor_data_consumer") {
                actors.push(Person {
                    id: ElementId::new("actor_data_consumer"),
                    name: "Data Consumer".to_string(),
                    description: "Service reading/writing data via ORM".to_string(),
                    location: ElementLocation::External,
                });
            }
        }

        actors
    }

    /// Parse dependencies from package.json
    fn parse_package_dependencies(&self, project_dir: &Path) -> Vec<(String, String)> {
        let mut deps = Vec::new();
        let package_json = project_dir.join("package.json");

        if let Ok(content) = std::fs::read_to_string(&package_json) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                // Extract dependencies
                if let Some(obj) = value.get("dependencies").and_then(|d| d.as_object()) {
                    for (name, val) in obj {
                        let version = val.as_str().unwrap_or("*").to_string();
                        deps.push((name.clone(), version));
                    }
                }

                // Extract devDependencies
                if let Some(obj) = value.get("devDependencies").and_then(|d| d.as_object()) {
                    for (name, val) in obj {
                        let version = val.as_str().unwrap_or("*").to_string();
                        deps.push((name.clone(), version));
                    }
                }
            }
        }

        deps
    }

    /// Create a component with the correct fields
    fn make_component(&self, id: &str, name: &str, comp_type: ComponentType, tech: &str, desc: &str, path: &Path) -> Component {
        Component {
            id: ElementId::new(id.to_string()),
            name: name.to_string(),
            component_type: comp_type,
            technology: tech.to_string(),
            description: desc.to_string(),
            path: Some(path.to_path_buf()),
            code_elements: Vec::new(),
        }
    }

    /// Infer Next.js specific components
    fn infer_nextjs_components(&self, project_dir: &Path) -> Vec<Component> {
        let mut components = Vec::new();

        // Next.js App Router components
        let app_dir = project_dir.join("app");
        if app_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&app_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && !path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false) {
                        let name = path.file_name().unwrap().to_string_lossy().to_string();

                        // Skip route groups (parentheses) and layout files
                        if name.starts_with('(') && name.ends_with(')') {
                            continue;
                        }

                        components.push(self.make_component(
                            &format!("component-{}", name.to_lowercase()),
                            &format!("Route: /{}", name),
                            ComponentType::Module,
                            "Next.js Route",
                            &format!("Next.js app router page for /{}", name),
                            &path,
                        ));
                    }
                }
            }
        }

        // Next.js Pages Router components
        let pages_dir = project_dir.join("pages");
        if pages_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&pages_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "tsx" || e == "ts" || e == "jsx" || e == "js").unwrap_or(false) {
                        let name = path.file_stem().unwrap().to_string_lossy().to_string();

                        // Skip _app, _document, _error
                        if name.starts_with('_') {
                            continue;
                        }

                        components.push(self.make_component(
                            &format!("component-page-{}", name.to_lowercase()),
                            &format!("Page: /{}", name),
                            ComponentType::Module,
                            "Next.js Page",
                            &format!("Next.js page component for /{}", name),
                            &path,
                        ));
                    }
                }
            }
        }

        // API routes
        let api_dir = project_dir.join("app/api").join("pages/api");
        if api_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&api_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() || path.extension().map(|e| e == "ts" || e == "js").unwrap_or(false) {
                        let name = path.file_stem().unwrap().to_string_lossy().to_string();
                        components.push(self.make_component(
                            &format!("component-api-{}", name.to_lowercase()),
                            &format!("API: /api/{}", name),
                            ComponentType::Service,
                            "Next.js API Route",
                            &format!("API endpoint at /api/{}", name),
                            &path,
                        ));
                    }
                }
            }
        }

        // Components directory
        let components_dir = project_dir.join("components");
        if components_dir.is_dir() {
            components.extend(self.infer_react_component_dir(&components_dir, "shared"));
        }

        // Shared components (app/components or components)
        let shared_components = project_dir.join("app/components");
        if shared_components.is_dir() {
            components.extend(self.infer_react_component_dir(&shared_components, "app"));
        }

        components
    }

    /// Infer React components
    fn infer_react_components(&self, project_dir: &Path) -> Vec<Component> {
        let mut components = Vec::new();

        // Common React component directories
        let component_dirs = vec![
            ("src/components", "components"),
            ("components", "components"),
            ("src/pages", "pages"),
            ("pages", "pages"),
            ("src/containers", "containers"),
            ("containers", "containers"),
        ];

        for (dir, prefix) in component_dirs {
            let path = project_dir.join(dir);
            if path.is_dir() {
                components.extend(self.infer_react_component_dir(&path, prefix));
            }
        }

        components
    }

    /// Infer React components from a directory
    fn infer_react_component_dir(&self, dir: &Path, prefix: &str) -> Vec<Component> {
        let mut components = Vec::new();

        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Check if it's a component file
                if path.extension().map(|e| e == "tsx" || e == "jsx").unwrap_or(false) {
                    let name = path.file_stem().unwrap().to_string_lossy().to_string();

                    // Skip index files, test files
                    if name == "index" || name.ends_with(".test") || name.ends_with(".spec") {
                        continue;
                    }

                    components.push(self.make_component(
                        &format!("component-{}-{}", prefix, name.to_lowercase()),
                        &name,
                        ComponentType::Module,
                        "React Component",
                        &format!("React component: {}", name),
                        &path,
                    ));
                }

                // Check if it's a subdirectory with index file
                if path.is_dir() {
                    let index_tsx = path.join("index.tsx");
                    let index_jsx = path.join("index.jsx");
                    let index_ts = path.join("index.ts");
                    let index_js = path.join("index.js");

                    let actual_path = if index_tsx.exists() {
                        Some(index_tsx)
                    } else if index_jsx.exists() {
                        Some(index_jsx)
                    } else if index_ts.exists() {
                        Some(index_ts)
                    } else if index_js.exists() {
                        Some(index_js)
                    } else {
                        None
                    };

                    if let Some(actual_path) = actual_path {
                        let name = path.file_name().unwrap().to_string_lossy().to_string();

                        components.push(self.make_component(
                            &format!("component-{}-{}", prefix, name.to_lowercase()),
                            &name,
                            ComponentType::Module,
                            "React Component",
                            &format!("React component: {}", name),
                            &actual_path,
                        ));
                    }
                }
            }
        }

        components
    }

    /// Infer components from a generic JS/TS project
    fn infer_generic_components(&self, project_dir: &Path) -> Vec<Component> {
        let mut components = Vec::new();

        // Look for src directory
        let src_dir = project_dir.join("src");
        if !src_dir.is_dir() {
            return components;
        }

        // Infer from directory structure
        if let Ok(entries) = std::fs::read_dir(&src_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && !path.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(false) {
                    let name = path.file_name().unwrap().to_string_lossy().to_string();

                    components.push(self.make_component(
                        &format!("component-{}", name.to_lowercase()),
                        &name,
                        infer_dir_component_type(&name),
                        "TypeScript",
                        &format!("Module: {}", name),
                        &path,
                    ));
                }
            }
        }

        components
    }
}

/// Infer component type from directory name
fn infer_dir_component_type(name: &str) -> ComponentType {
    let name_lower = name.to_lowercase();
    match name_lower.as_str() {
        "components" | "ui" | "shared" | "common" => ComponentType::Module,
        "services" | "api" | "endpoints" => ComponentType::Service,
        "hooks" | "use" => ComponentType::Module,
        "utils" | "helpers" | "lib" => ComponentType::Module,
        "pages" | "views" | "screens" => ComponentType::Module,
        "models" | "types" | "interfaces" => ComponentType::Interface,
        "store" | "state" | "context" => ComponentType::Service,
        _ => ComponentType::Module,
    }
}

impl Default for TsInference {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_project_type_detection_react() {
        let temp_dir = TempDir::new().unwrap();
        let pkg_path = temp_dir.path().join("package.json");

        std::fs::write(
            &pkg_path,
            r#"{
                "name": "my-react-app",
                "dependencies": {
                    "react": "^18.0.0",
                    "react-dom": "^18.0.0"
                }
            }"#,
        )
        .unwrap();

        let components_dir = temp_dir.path().join("src/components");
        std::fs::create_dir_all(&components_dir).unwrap();
        std::fs::write(components_dir.join("Button.tsx"), "export const Button = () => {}").unwrap();

        let inference = TsInference::new();
        let components = inference.infer_components(temp_dir.path(), &CallGraph::new());

        assert!(!components.is_empty());
        assert!(components.iter().any(|c| c.name == "Button"));
    }

    #[test]
    fn test_infer_dir_component_type() {
        assert_eq!(infer_dir_component_type("components"), ComponentType::Module);
        assert_eq!(infer_dir_component_type("services"), ComponentType::Service);
        assert_eq!(infer_dir_component_type("interfaces"), ComponentType::Interface);
    }

    #[test]
    fn test_infer_actors_react() {
        let temp_dir = TempDir::new().unwrap();
        let pkg_path = temp_dir.path().join("package.json");

        std::fs::write(
            &pkg_path,
            r#"{
                "name": "my-react-app",
                "dependencies": {
                    "react": "^18.0.0",
                    "react-dom": "^18.0.0"
                }
            }"#,
        )
        .unwrap();

        let inference = TsInference::new();
        let actors = inference.infer_actors(temp_dir.path());

        assert!(!actors.is_empty());
        assert!(actors.iter().any(|a| a.name == "End User"));
    }

    #[test]
    fn test_infer_actors_express() {
        let temp_dir = TempDir::new().unwrap();
        let pkg_path = temp_dir.path().join("package.json");

        std::fs::write(
            &pkg_path,
            r#"{
                "name": "my-api",
                "dependencies": {
                    "express": "^4.0.0",
                    "mongoose": "^6.0.0"
                }
            }"#,
        )
        .unwrap();

        let inference = TsInference::new();
        let actors = inference.infer_actors(temp_dir.path());

        assert!(!actors.is_empty());
        assert!(actors.iter().any(|a| a.name == "End User"));
        assert!(actors.iter().any(|a| a.name == "Data Consumer"));
    }

    #[test]
    fn test_infer_actors_nextjs() {
        let temp_dir = TempDir::new().unwrap();
        let pkg_path = temp_dir.path().join("package.json");

        std::fs::write(
            &pkg_path,
            r#"{
                "name": "my-nextjs-app",
                "dependencies": {
                    "next": "^13.0.0",
                    "react": "^18.0.0",
                    "@prisma/client": "^5.0.0"
                }
            }"#,
        )
        .unwrap();

        let inference = TsInference::new();
        let actors = inference.infer_actors(temp_dir.path());

        assert!(actors.iter().any(|a| a.name == "End User"));
        assert!(actors.iter().any(|a| a.name == "API Consumer"));
        assert!(actors.iter().any(|a| a.name == "Data Consumer"));
    }

    #[test]
    fn test_infer_actors_empty_project() {
        let temp_dir = TempDir::new().unwrap();
        // No package.json

        let inference = TsInference::new();
        let actors = inference.infer_actors(temp_dir.path());

        assert!(actors.is_empty());
    }

    #[test]
    fn test_infer_actors_vue() {
        let temp_dir = TempDir::new().unwrap();
        let pkg_path = temp_dir.path().join("package.json");

        std::fs::write(
            &pkg_path,
            r#"{
                "name": "my-vue-app",
                "dependencies": {
                    "vue": "^3.0.0"
                }
            }"#,
        )
        .unwrap();

        let inference = TsInference::new();
        let actors = inference.infer_actors(temp_dir.path());

        assert!(!actors.is_empty());
        assert!(actors.iter().any(|a| a.name == "End User"));
    }

    #[test]
    fn test_infer_actors_nestjs() {
        let temp_dir = TempDir::new().unwrap();
        let pkg_path = temp_dir.path().join("package.json");

        std::fs::write(
            &pkg_path,
            r#"{
                "name": "my-nestjs-api",
                "dependencies": {
                    "@nestjs/core": "^10.0.0",
                    "@nestjs/platform-express": "^10.0.0"
                }
            }"#,
        )
        .unwrap();

        let inference = TsInference::new();
        let actors = inference.infer_actors(temp_dir.path());

        assert!(!actors.is_empty());
        assert!(actors.iter().any(|a| a.name == "End User"));
    }
}
