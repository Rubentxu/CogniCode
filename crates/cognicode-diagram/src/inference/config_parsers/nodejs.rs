//! Node.js package.json parser for container inference

use std::path::Path;

use crate::model::c4_types::{Container, ContainerType, ElementId};

/// Parser for Node.js package.json files
#[derive(Debug, Clone)]
pub struct NodeJsParser;

impl NodeJsParser {
    /// Parse package.json and infer containers
    pub fn parse_package_json(path: &Path) -> anyhow::Result<Option<Container>> {
        let content = std::fs::read_to_string(path)?;
        let pkg: serde_json::Value = serde_json::from_str(&content)?;

        let name = pkg["name"].as_str().unwrap_or("unknown");
        let description = pkg["description"].as_str().unwrap_or("");

        // Classify container type
        let scripts = &pkg["scripts"];
        let has_service_script = scripts.is_object()
            && (scripts.get("start").is_some()
                || scripts.get("dev").is_some()
                || scripts.get("serve").is_some()
                || scripts.get("build").is_some()
                || scripts.get("start:prod").is_some());

        let container_type = if has_service_script {
            ContainerType::Service
        } else if pkg["bin"].is_object() {
            ContainerType::Executable
        } else {
            ContainerType::Library
        };

        // Detect technology
        let deps = pkg["dependencies"]
            .as_object()
            .map(|d| d.keys().collect::<Vec<_>>())
            .unwrap_or_default();
        let dev_deps = pkg["devDependencies"]
            .as_object()
            .map(|d| d.keys().collect::<Vec<_>>())
            .unwrap_or_default();

        let mut all_deps: Vec<&String> = deps;
        all_deps.extend(dev_deps);
        let technology = detect_nodejs_technology(&all_deps);

        Ok(Some(Container {
            id: ElementId::new(format!("container-{}", name.replace('/', "_"))),
            name: name.to_string(),
            container_type,
            technology,
            description: description.to_string(),
            path: Some(path.parent().unwrap().to_path_buf()),
            components: Vec::new(),
        }))
    }
}

/// Detect Node.js technology stack from dependencies
fn detect_nodejs_technology(deps: &[&String]) -> String {
    let mut tech = vec!["Node.js".to_string()];

    for dep in deps {
        match dep.as_str() {
            "express" => tech.push("Express".to_string()),
            "fastify" => tech.push("Fastify".to_string()),
            "koa" => tech.push("Koa".to_string()),
            "hapi" => tech.push("Hapi".to_string()),
            "mongoose" => tech.push("Mongoose".to_string()),
            "sequelize" => tech.push("Sequelize".to_string()),
            "pg" | "mysql" | "mysql2" | "better-sqlite3" => tech.push("Database ORM".to_string()),
            "ioredis" | "node-redis" | "redis" => tech.push("Redis Client".to_string()),
            "axios" | "node-fetch" | "got" | "undici" => tech.push("HTTP Client".to_string()),
            "ws" | "socket.io" | "socket.io-client" => tech.push("WebSocket".to_string()),
            "next" => tech.push("Next.js".to_string()),
            "nuxt" => tech.push("Nuxt".to_string()),
            "react" => tech.push("React".to_string()),
            "vue" => tech.push("Vue".to_string()),
            "angular" | "@angular/core" => tech.push("Angular".to_string()),
            "svelte" => tech.push("Svelte".to_string()),
            "nestjs" | "@nestjs/core" => tech.push("NestJS".to_string()),
            "prisma" | "@prisma/client" => tech.push("Prisma".to_string()),
            "typeorm" => tech.push("TypeORM".to_string()),
            "graphql" | "@apollo/client" => tech.push("GraphQL".to_string()),
            "jest" | "vitest" | "mocha" | "jasmine" => tech.push("Testing".to_string()),
            "webpack" | "vite" | "rollup" | "esbuild" => tech.push("Bundler".to_string()),
            "typescript" => {}
            _ => {}
        }
    }

    tech.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_package_json_service() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(
                r#"{
                "name": "my-api",
                "description": "My REST API",
                "scripts": { "start": "node index.js" },
                "dependencies": { "express": "^4.0.0", "mongoose": "^6.0.0" }
            }"#
                .as_bytes(),
            )
            .unwrap();

        let container = NodeJsParser::parse_package_json(temp_file.path()).unwrap().unwrap();

        assert_eq!(container.name, "my-api");
        assert_eq!(container.container_type, ContainerType::Service);
        assert!(container.technology.contains("Express"));
        assert!(container.technology.contains("Node.js"));
    }

    #[test]
    fn test_parse_package_json_library() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(
                r#"{
                "name": "my-utils",
                "description": "Utility functions"
            }"#
                .as_bytes(),
            )
            .unwrap();

        let container = NodeJsParser::parse_package_json(temp_file.path()).unwrap().unwrap();

        assert_eq!(container.name, "my-utils");
        assert_eq!(container.container_type, ContainerType::Library);
    }

    #[test]
    fn test_parse_package_json_executable() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file
            .write_all(
                r#"{
                "name": "my-cli",
                "description": "CLI tool",
                "bin": { "my-cli": "./bin/cli.js" }
            }"#
                .as_bytes(),
            )
            .unwrap();

        let container = NodeJsParser::parse_package_json(temp_file.path()).unwrap().unwrap();

        assert_eq!(container.name, "my-cli");
        assert_eq!(container.container_type, ContainerType::Executable);
    }

    #[test]
    fn test_detect_nodejs_technology() {
        let express = "express".to_string();
        let mongoose = "mongoose".to_string();
        let typescript = "typescript".to_string();
        let deps = vec![&express, &mongoose, &typescript];
        let tech = detect_nodejs_technology(&deps);

        assert!(tech.contains("Express"));
        assert!(tech.contains("Mongoose"));
        assert!(tech.contains("Node.js"));
    }
}
