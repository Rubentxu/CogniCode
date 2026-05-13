//! Go module parser for container inference
//!
//! Parses go.mod files to extract container metadata including
//! module name, go version, and dependencies.

use std::path::Path;

use crate::model::c4_types::{Container, ContainerType, ElementId};

/// Information extracted from a go.mod file
#[derive(Debug, Clone)]
pub struct GoModuleInfo {
    /// Module name (import path)
    pub name: String,
    /// Go version
    pub go_version: Option<String>,
    /// Dependencies (direct)
    pub dependencies: Vec<String>,
    /// Whether this is a module or legacy GOPATH project
    pub is_module: bool,
}

/// Parser for Go go.mod files
#[derive(Debug, Clone)]
pub struct GoParser;

impl GoParser {
    /// Parse go.mod and infer container
    pub fn parse_go_mod(path: &Path) -> anyhow::Result<Option<Container>> {
        let content = std::fs::read_to_string(path)?;
        let module_info = Self::parse_go_mod_content(&content)?;

        let container_type = Self::classify_container_type(&module_info);

        let technology = Self::detect_technology(&module_info.dependencies);

        Ok(Some(Container {
            id: ElementId::new(format!("container-{}", module_info.name.replace('/', "_"))),
            name: module_info.name.clone(),
            container_type,
            technology,
            description: format!(
                "Go module: {} (go {})",
                module_info.name,
                module_info.go_version.unwrap_or_else(|| "unknown".to_string())
            ),
            path: Some(path.parent().unwrap().to_path_buf()),
            components: Vec::new(),
        }))
    }

    /// Parse go.mod content and extract module info
    pub fn parse_go_mod_content(content: &str) -> anyhow::Result<GoModuleInfo> {
        let mut name = String::new();
        let mut go_version = None;
        let mut dependencies = Vec::new();
        let is_module = true;

        for line in content.lines() {
            let line = line.trim();

            if line.starts_with("module ") {
                name = line.trim_start_matches("module ").to_string();
            } else if line.starts_with("go ") {
                go_version = Some(line.trim_start_matches("go ").to_string());
            } else if line.starts_with("require (") || line.starts_with("require(") {
                // Multi-line require block
                continue;
            } else if line.starts_with(")") && !dependencies.is_empty() {
                // End of require block
                continue;
            } else if line.starts_with("require ") {
                // Single-line require
                let dep = line.trim_start_matches("require ").split_whitespace().next();
                if let Some(dep) = dep {
                    dependencies.push(dep.to_string());
                }
            } else if !line.is_empty() && !line.starts_with("//") && !line.starts_with("module") && !line.starts_with("go") && !line.starts_with("require") && !line.starts_with("replace") && !line.starts_with("exclude") {
                // Likely a dependency in require block (no leading whitespace check for robustness)
                if !line.starts_with('[') && !line.ends_with(')') {
                    let dep = line.split_whitespace().next().unwrap_or(line);
                    if !dep.is_empty() && dep != "(" && dep != ")" {
                        dependencies.push(dep.to_string());
                    }
                }
            }
        }

        if name.is_empty() {
            name = "go-module".to_string();
        }

        Ok(GoModuleInfo {
            name,
            go_version,
            dependencies,
            is_module,
        })
    }

    /// Classify container type based on module info
    fn classify_container_type(info: &GoModuleInfo) -> ContainerType {
        let name_lower = info.name.to_lowercase();

        // Heuristics based on module name
        if name_lower.contains("service")
            || name_lower.contains("api")
            || name_lower.contains("server")
            || name_lower.contains("gateway")
        {
            return ContainerType::Service;
        }

        if name_lower.contains("cli")
            || name_lower.contains("cmd")
            || name_lower.contains("tool")
        {
            return ContainerType::Executable;
        }

        if name_lower.contains("db")
            || name_lower.contains("store")
            || name_lower.contains("data")
            || name_lower.contains("repository")
        {
            return ContainerType::DataStore;
        }

        if name_lower.contains("queue")
            || name_lower.contains("broker")
            || name_lower.contains("mq")
        {
            return ContainerType::Queue;
        }

        // Check dependencies for web frameworks
        for dep in &info.dependencies {
            let dep_lower = dep.to_lowercase();
            if dep_lower.contains("gin")
                || dep_lower.contains("echo")
                || dep_lower.contains("fiber")
                || dep_lower.contains("chi")
                || dep_lower.contains("gorilla")
            {
                return ContainerType::Service;
            }

            if dep_lower.contains("grpc") || dep_lower.contains("protobuf") {
                return ContainerType::Service;
            }
        }

        // Default based on module path patterns
        if info.name.starts_with("github.com/")
            || info.name.starts_with("gitlab.com/")
            || info.name.starts_with("go.uber.org/")
        {
            // Likely a library
            return ContainerType::Library;
        }

        ContainerType::Library
    }

    /// Detect technology stack from dependencies
    fn detect_technology(dependencies: &[String]) -> String {
        let mut tech = vec!["Go".to_string()];

        for dep in dependencies {
            let dep_lower = dep.to_lowercase();

            // Web frameworks
            if dep_lower.contains("gin-gonic/gin") || dep_lower == "gin" {
                tech.push("Gin".to_string());
            }
            if dep_lower.contains("labstack/echo") || dep_lower == "echo" {
                tech.push("Echo".to_string());
            }
            if dep_lower.contains("gofiber/fiber") || dep_lower == "fiber" {
                tech.push("Fiber".to_string());
            }
            if dep_lower.contains("go-chi/chi") || dep_lower == "chi" {
                tech.push("Chi".to_string());
            }
            if dep_lower.contains("gorilla/mux") || dep_lower == "mux" {
                tech.push("Gorilla Mux".to_string());
            }

            // gRPC and API
            if dep_lower.contains("grpc") {
                tech.push("gRPC".to_string());
            }
            if dep_lower.contains("protobuf") {
                tech.push("Protocol Buffers".to_string());
            }

            // Database
            if dep_lower.contains("gorm") {
                tech.push("GORM".to_string());
            }
            if dep_lower.contains("sqlx") || dep_lower.contains("database/sql") {
                tech.push("Go SQL".to_string());
            }
            if dep_lower.contains("mongo") || dep_lower.contains("mongodb") {
                tech.push("MongoDB".to_string());
            }
            if dep_lower.contains("redis") {
                tech.push("Redis".to_string());
            }
            if dep_lower.contains("elastic") {
                tech.push("Elasticsearch".to_string());
            }

            // Message queues
            if dep_lower.contains("rabbitmq") || dep_lower.contains("amqp") {
                tech.push("RabbitMQ".to_string());
            }
            if dep_lower.contains("kafka") || dep_lower.contains("sarama") {
                tech.push("Kafka".to_string());
            }

            // Microservices
            if dep_lower.contains("go-kit") || dep_lower.contains("gokit") {
                tech.push("Go Kit".to_string());
            }
            if dep_lower.contains("micro") {
                tech.push("Micro".to_string());
            }

            // Authentication
            if dep_lower.contains("jwt") || dep_lower.contains("golang-jwt") {
                tech.push("JWT".to_string());
            }
            if dep_lower.contains("oauth") {
                tech.push("OAuth".to_string());
            }

            // CLI
            if dep_lower.contains("cobra") {
                tech.push("Cobra CLI".to_string());
            }
            if dep_lower.contains("urfave/cli") || dep_lower == "cli" {
                tech.push("Urfave CLI".to_string());
            }
            if dep_lower.contains("spf13/cobra") {
                tech.push("Cobra CLI".to_string());
            }

            // Testing
            if dep_lower.contains("testify") || dep_lower.contains("assert") {
                tech.push("Testify".to_string());
            }
            if dep_lower.contains("ginkgo") || dep_lower.contains("gomega") {
                tech.push("Ginkgo".to_string());
            }

            // Utilities
            if dep_lower.contains("zap") {
                tech.push("Zap Logger".to_string());
            }
            if dep_lower.contains("logrus") {
                tech.push("Logrus".to_string());
            }
            if dep_lower.contains("viper") {
                tech.push("Viper Config".to_string());
            }
        }

        tech.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_go_mod_basic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(
                b"module github.com/example/myapi\n\ngo 1.21\n\nrequire (\n\tgithub.com/gin-gonic/gin v1.9.1\n\tgithub.com/sqlx drivers v1.5.0\n)\n",
            )
            .unwrap();

        let container = GoParser::parse_go_mod(temp_file.path()).unwrap().unwrap();

        assert_eq!(container.name, "github.com/example/myapi");
        assert_eq!(container.container_type, ContainerType::Service);
        assert!(container.technology.contains("Gin"));
        assert!(container.technology.contains("Go"));
    }

    #[test]
    fn test_parse_go_mod_cli() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(
                b"module github.com/example/mycli\n\ngo 1.21\n\nrequire (\n\tgithub.com/spf13/cobra v1.7.0\n\tgithub.com/spf13/viper v1.15.0\n)\n",
            )
            .unwrap();

        let container = GoParser::parse_go_mod(temp_file.path()).unwrap().unwrap();

        assert_eq!(container.name, "github.com/example/mycli");
        assert_eq!(container.container_type, ContainerType::Executable);
        assert!(container.technology.contains("Cobra CLI"));
    }

    #[test]
    fn test_parse_go_mod_library() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(
                b"module github.com/example/utils\n\ngo 1.21\n",
            )
            .unwrap();

        let container = GoParser::parse_go_mod(temp_file.path()).unwrap().unwrap();

        assert_eq!(container.name, "github.com/example/utils");
        assert_eq!(container.container_type, ContainerType::Library);
    }

    #[test]
    fn test_parse_go_mod_single_require() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(
                b"module github.com/example/server\n\ngo 1.21\n\nrequire github.com/labstack/echo/v4 v4.11.0\n",
            )
            .unwrap();

        let container = GoParser::parse_go_mod(temp_file.path()).unwrap().unwrap();

        assert_eq!(container.name, "github.com/example/server");
        assert!(container.technology.contains("Echo"));
    }

    #[test]
    fn test_detect_technology() {
        let deps = vec![
            "github.com/gin-gonic/gin".to_string(),
            "github.com/redis/go-redis/v9".to_string(),
            "github.com/grpc/grpc-go".to_string(),
        ];

        let tech = GoParser::detect_technology(&deps);

        assert!(tech.contains("Gin"));
        assert!(tech.contains("Redis"));
        assert!(tech.contains("gRPC"));
        assert!(tech.contains("Go"));
    }
}
