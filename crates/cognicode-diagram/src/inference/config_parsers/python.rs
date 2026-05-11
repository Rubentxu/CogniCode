//! Python project parser for container inference
//!
//! Parses pyproject.toml and setup.py files to extract container metadata.

use std::path::Path;

use crate::model::c4_types::{Container, ContainerType, ElementId};

/// Parser for Python projects
#[derive(Debug, Clone)]
pub struct PythonParser;

impl PythonParser {
    /// Infer containers from pyproject.toml
    pub fn parse_pyproject(path: &Path) -> anyhow::Result<Option<Container>> {
        let content = std::fs::read_to_string(path)?;

        // Try to parse as TOML first (pyproject.toml format)
        if let Ok(toml) = content.parse::<toml::Value>() {
            // pyproject.toml [project] section (PEP 621)
            if let Some(project) = toml.get("project") {
                let name = project
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("python-project");

                let description = project
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");

                // Detect scripts (entry points) - indicates a service
                let is_service = project.get("scripts").is_some();

                // Check for dependencies to infer technology
                let dependencies = project
                    .get("dependencies")
                    .or_else(|| project.get("optional-dependencies"))
                    .map(|d| {
                        d.as_array()
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                    })
                    .unwrap_or_default();

                let container_type = if is_service {
                    ContainerType::Service
                } else if is_cli_tool(&project) {
                    ContainerType::Executable
                } else {
                    ContainerType::Library
                };

                let technology = detect_python_technology(&dependencies);

                return Ok(Some(Container {
                    id: ElementId::new(format!("container-{}", name.replace('-', "_"))),
                    name: name.to_string(),
                    container_type,
                    technology,
                    description: description.to_string(),
                    path: Some(path.parent().unwrap().to_path_buf()),
                    components: Vec::new(),
                }));
            }

            // Fallback: try to parse [tool.poetry] section (Poetry format)
            if let Some(poetry) = toml.get("tool").and_then(|t| t.get("poetry")) {
                let name = poetry
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("python-project");

                let description = poetry
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");

                let dependencies = poetry
                    .get("dependencies")
                    .and_then(|d| d.as_table())
                    .map(|table| table.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default();

                let container_type = if poetry.get("scripts").is_some() {
                    ContainerType::Service
                } else {
                    ContainerType::Library
                };

                let technology = detect_python_technology(&dependencies);

                return Ok(Some(Container {
                    id: ElementId::new(format!("container-{}", name.replace('-', "_"))),
                    name: name.to_string(),
                    container_type,
                    technology,
                    description: description.to_string(),
                    path: Some(path.parent().unwrap().to_path_buf()),
                    components: Vec::new(),
                }));
            }
        }

        Ok(None)
    }

    /// Parse setup.py for older Python projects
    pub fn parse_setup_py(project_dir: &Path) -> anyhow::Result<Option<Container>> {
        let setup_py = project_dir.join("setup.py");

        if !setup_py.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&setup_py)?;

        // Simple extraction of name and description from setup.py
        let name = extract_setup_value(&content, "name")
            .unwrap_or_else(|| "python-project".to_string());

        let description = extract_setup_value(&content, "description").unwrap_or_default();

        // Check for entry_points to determine type
        let has_entry_points = content.contains("entry_points");

        let container_type = if has_entry_points {
            ContainerType::Service
        } else {
            ContainerType::Library
        };

        // Extract dependencies from install_requires
        let dependencies = extract_list_value(&content, "install_requires");

        Ok(Some(Container {
            id: ElementId::new(format!("container-{}", name.replace('-', "_"))),
            name: name.clone(),
            container_type,
            technology: detect_python_technology(&dependencies),
            description,
            path: Some(project_dir.to_path_buf()),
            components: Vec::new(),
        }))
    }
}

/// Check if project is a CLI tool based on scripts/console_scripts
fn is_cli_tool(project: &toml::Value) -> bool {
    // Check for console_scripts in [project.scripts] (PEP 621)
    if let Some(scripts) = project.get("scripts").and_then(|s| s.as_table()) {
        if !scripts.is_empty() {
            return true;
        }
    }

    // Check for pyproject.toml [project.scripts] section
    if project.get("scripts").is_some() {
        return true;
    }

    false
}

/// Detect Python technology stack from dependencies
fn detect_python_technology(deps: &[String]) -> String {
    let mut tech = vec!["Python".to_string()];

    for dep in deps {
        let dep_lower = dep.to_lowercase();

        // Web frameworks
        if dep_lower.contains("fastapi")
            || dep_lower.contains("starlette")
            || dep_lower.contains("sanic")
        {
            tech.push("FastAPI".to_string());
        }
        if dep_lower.contains("flask") {
            tech.push("Flask".to_string());
        }
        if dep_lower.contains("django") {
            tech.push("Django".to_string());
        }
        if dep_lower.contains("tornado") {
            tech.push("Tornado".to_string());
        }

        // Async
        if dep_lower.contains("asyncio") || dep_lower.contains("aiohttp") || dep_lower.contains("uvicorn") {
            tech.push("Async".to_string());
        }

        // ORMs / Database
        if dep_lower.contains("sqlalchemy") {
            tech.push("SQLAlchemy".to_string());
        }
        if dep_lower.contains("django-orm") || dep_lower.contains("djangorestframework") {
            tech.push("Django ORM".to_string());
        }
        if dep_lower.contains("psycopg") || dep_lower.contains("mysqlclient") || dep_lower.contains("pymysql") {
            tech.push("Database Driver".to_string());
        }
        if dep_lower.contains("redis") {
            tech.push("Redis".to_string());
        }
        if dep_lower.contains("pydantic") {
            tech.push("Pydantic".to_string());
        }

        // Message queues
        if dep_lower.contains("celery") {
            tech.push("Celery".to_string());
        }
        if dep_lower.contains("pika") || dep_lower.contains("aio-pika") {
            tech.push("RabbitMQ".to_string());
        }

        // Testing
        if dep_lower.contains("pytest") {
            tech.push("Pytest".to_string());
        }
        if dep_lower.contains("unittest") {
            tech.push("Unittest".to_string());
        }

        // GraphQL
        if dep_lower.contains("graphene") || dep_lower.contains("strawberry") {
            tech.push("GraphQL".to_string());
        }

        // ML/AI
        if dep_lower.contains("tensorflow") {
            tech.push("TensorFlow".to_string());
        }
        if dep_lower.contains("torch") || dep_lower.contains("pytorch") {
            tech.push("PyTorch".to_string());
        }
        if dep_lower.contains("numpy") || dep_lower.contains("pandas") {
            tech.push("Data Science".to_string());
        }

        // CLI
        if dep_lower.contains("click") || dep_lower.contains("typer") || dep_lower.contains("argparse") {
            tech.push("CLI".to_string());
        }

        // WebSocket
        if dep_lower.contains("websockets") {
            tech.push("WebSocket".to_string());
        }
    }

    tech.join(", ")
}

/// Extract a string value from setup.py content
fn extract_setup_value(content: &str, key: &str) -> Option<String> {
    // Match patterns like: name="value" or name = "value"
    let pattern = format!(r#"{}\s*=\s*["']([^"']+)["']"#, key);

    let re = regex::Regex::new(&pattern).ok()?;
    re.captures(content)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

/// Extract a list value (dependencies) from setup.py content
fn extract_list_value(content: &str, key: &str) -> Vec<String> {
    // Match patterns like: install_requires=["a", "b"] or install_requires = ["a", "b"]
    let pattern = format!(r#"{}\s*=\s*\[([^\]]+)\]"#, key);

    let re = match regex::Regex::new(&pattern) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    let deps_str = match re.captures(content) {
        Some(cap) => match cap.get(1) {
            Some(m) => m.as_str(),
            None => return Vec::new(),
        },
        None => return Vec::new(),
    };

    // Extract each quoted string
    let item_pattern = r#"["']([^"']+)["']"#;
    let item_re = match regex::Regex::new(item_pattern) {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };

    item_re
        .captures_iter(deps_str)
        .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_pyproject_service() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(
                r#"
[project]
name = "my-api"
description = "My FastAPI service"
dependencies = ["fastapi", "uvicorn", "sqlalchemy"]
scripts.my-api = "my_api.main:app"
"#
                .as_bytes(),
            )
            .unwrap();

        let container = PythonParser::parse_pyproject(temp_file.path()).unwrap().unwrap();

        assert_eq!(container.name, "my-api");
        assert_eq!(container.container_type, ContainerType::Service);
        assert!(container.technology.contains("FastAPI"));
        assert!(container.technology.contains("Python"));
    }

    #[test]
    fn test_parse_pyproject_library() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(
                r#"
[project]
name = "my-utils"
description = "Utility functions"
dependencies = ["typing"]
"#
                .as_bytes(),
            )
            .unwrap();

        let container = PythonParser::parse_pyproject(temp_file.path()).unwrap().unwrap();

        assert_eq!(container.name, "my-utils");
        assert_eq!(container.container_type, ContainerType::Library);
    }

    #[test]
    fn test_parse_poetry_project() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(
                r#"
[tool.poetry]
name = "my-poetry-lib"
description = "Poetry managed library"

[tool.poetry.dependencies]
python = "^3.9"
django = "^4.0"

[tool.poetry.scripts]
my-cmd = "my_package:main"
"#
                .as_bytes(),
            )
            .unwrap();

        let container = PythonParser::parse_pyproject(temp_file.path()).unwrap().unwrap();

        assert_eq!(container.name, "my-poetry-lib");
        assert_eq!(container.container_type, ContainerType::Service);
        assert!(container.technology.contains("Django"));
    }

    #[test]
    fn test_detect_python_technology() {
        let deps = vec![
            "fastapi".to_string(),
            "uvicorn".to_string(),
            "sqlalchemy".to_string(),
            "pydantic".to_string(),
        ];
        let tech = detect_python_technology(&deps);

        assert!(tech.contains("FastAPI"));
        assert!(tech.contains("SQLAlchemy"));
        assert!(tech.contains("Python"));
    }

    #[test]
    fn test_extract_setup_value() {
        let content = r#"
from setuptools import setup
setup(
    name="my-package",
    description="A sample package",
    version="1.0.0"
)
"#;

        assert_eq!(extract_setup_value(content, "name"), Some("my-package".to_string()));
        assert_eq!(
            extract_setup_value(content, "description"),
            Some("A sample package".to_string())
        );
    }

    #[test]
    fn test_extract_list_value() {
        let content = r#"
setup(
    name="my-package",
    install_requires=["fastapi", "uvicorn", "sqlalchemy"]
)
"#;

        let deps = extract_list_value(content, "install_requires");
        assert_eq!(deps.len(), 3);
        assert!(deps.contains(&"fastapi".to_string()));
        assert!(deps.contains(&"uvicorn".to_string()));
    }
}
