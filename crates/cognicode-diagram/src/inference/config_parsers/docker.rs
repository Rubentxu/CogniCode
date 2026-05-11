//! Docker and Docker Compose parser for deployment inference
//!
//! Parses Dockerfile and docker-compose.yml files to extract deployment
//! metadata including services, networks, volumes, and relationships.

use std::path::{Path, PathBuf};
use indexmap::IndexMap;

use crate::model::deployment::{
    DeploymentModel, DeploymentNode, DeploymentRelationship, Network, PortMapping, Volume,
};

/// Result of parsing a Dockerfile
#[derive(Debug, Clone)]
pub struct DockerfileInfo {
    pub base_image: Option<String>,
    pub stage: Option<String>,
    pub expose_ports: Vec<u16>,
    pub environment: IndexMap<String, String>,
    pub command: Option<String>,
    pub entrypoint: Option<String>,
    pub is_multi_stage: bool,
}

/// Result of parsing a docker-compose.yml
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ComposeInfo {
    pub services: Vec<ComposeService>,
    pub networks: Vec<ComposeNetwork>,
    pub volumes: Vec<ComposeVolume>,
    pub dependencies: Vec<ComposeDependency>,
}

/// A service defined in docker-compose.yml
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ComposeService {
    pub name: String,
    pub image: Option<String>,
    pub ports: Option<Vec<String>>,
    pub environment: Option<IndexMap<String, String>>,
    pub command: Option<String>,
    pub depends_on: Option<Vec<String>>,
}

/// A network defined in docker-compose.yml
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ComposeNetwork {
    pub name: String,
    pub driver: Option<String>,
}

/// A volume defined in docker-compose.yml
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ComposeVolume {
    pub name: String,
    pub driver: Option<String>,
}

/// A dependency between services
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ComposeDependency {
    pub from: String,
    pub to: String,
}

/// Parser for Dockerfile files
#[derive(Debug, Clone)]
pub struct DockerfileParser;

impl DockerfileParser {
    /// Parse a Dockerfile and extract deployment information
    pub fn parse(dockerfile_path: &Path) -> anyhow::Result<Vec<DockerfileInfo>> {
        let content = std::fs::read_to_string(dockerfile_path)?;
        Self::parse_content(&content)
    }

    /// Parse Dockerfile content directly
    pub fn parse_content(content: &str) -> anyhow::Result<Vec<DockerfileInfo>> {
        let mut results = Vec::new();
        let mut current_stage: Option<String> = None;
        let mut current_info: Option<DockerfileInfo> = None;

        for line in content.lines() {
            let line = line.trim();

            // Handle FROM instruction (starts new stage)
            if let Some(stage_info) = Self::parse_from(line, &current_stage) {
                // Save previous stage if exists
                if let Some(info) = current_info.take() {
                    results.push(info);
                }

                current_stage = stage_info.1;
                current_info = Some(DockerfileInfo {
                    base_image: Some(stage_info.0),
                    stage: current_stage.clone(),
                    expose_ports: Vec::new(),
                    environment: IndexMap::new(),
                    command: None,
                    entrypoint: None,
                    is_multi_stage: current_stage.is_some(),
                });
            } else if let Some(ref mut info) = current_info {
                // Parse other instructions for current stage
                Self::parse_instruction(line, info);
            }
        }

        // Don't forget the last stage
        if let Some(info) = current_info {
            results.push(info);
        }

        // If no stages found, return empty
        if results.is_empty() {
            // Try to parse as single-stage Dockerfile
            let single = Self::parse_single_stage(content)?;
            if !single.expose_ports.is_empty() || single.environment.len() > 0 || single.command.is_some() {
                results.push(single);
            }
        }

        Ok(results)
    }

    /// Parse a FROM instruction
    fn parse_from(line: &str, _current_stage: &Option<String>) -> Option<(String, Option<String>)> {
        let upper = line.to_uppercase();
        if !upper.starts_with("FROM ") {
            return None;
        }

        let rest = &line[5..].trim();
        let parts: Vec<&str> = rest.split_whitespace().collect();

        if parts.is_empty() {
            return None;
        }

        let image = parts[0].to_string();
        let stage = if parts.len() >= 3 && parts[1].to_uppercase() == "AS" {
            Some(parts[2].to_string())
        } else {
            None
        };

        Some((image, stage))
    }

    /// Parse EXPOSE, ENV, CMD, ENTRYPOINT instructions
    fn parse_instruction(line: &str, info: &mut DockerfileInfo) {
        let upper = line.to_uppercase();

        if upper.starts_with("EXPOSE ") {
            let port_str = &line[7..].trim();
            // Parse multiple ports (space-separated)
            for port_part in port_str.split_whitespace() {
                if let Ok(port) = port_part.parse::<u16>() {
                    info.expose_ports.push(port);
                }
            }
        } else if upper.starts_with("ENV ") {
            let rest = &line[4..].trim();
            if let Some((key, value)) = Self::parse_env_var(rest) {
                info.environment.insert(key, value);
            }
        } else if upper.starts_with("CMD ") {
            let rest = &line[4..].trim();
            // Remove surrounding quotes if present
            let cmd = Self::extract_cmd(rest);
            info.command = Some(cmd);
        } else if upper.starts_with("ENTRYPOINT ") {
            let rest = &line[11..].trim();
            let entrypoint = Self::extract_cmd(rest);
            info.entrypoint = Some(entrypoint);
            // ENTRYPOINT overrides CMD
            info.command = None;
        }
    }

    /// Parse ENV KEY=VALUE format
    fn parse_env_var(rest: &str) -> Option<(String, String)> {
        // Handle KEY=VALUE and KEY="VALUE" formats
        let rest = rest.trim();

        if let Some(eq_pos) = rest.find('=') {
            let key = rest[..eq_pos].trim().to_string();
            let value = rest[eq_pos + 1..].trim().to_string();

            // Remove surrounding quotes
            let value = Self::strip_quotes(&value).to_string();

            Some((key, value))
        } else {
            None
        }
    }

    /// Extract command from CMD/ENTRYPOINT
    fn extract_cmd(rest: &str) -> String {
        let rest = rest.trim();

        // Handle JSON array format: ["cmd", "arg1"]
        if rest.starts_with('[') {
            // Try to parse as JSON array
            if let Ok(arr) = serde_json::from_str::<Vec<String>>(rest) {
                return arr.join(" ");
            }
        }

        // Remove surrounding quotes
        Self::strip_quotes(rest).to_string()
    }

    /// Strip surrounding quotes from a string
    fn strip_quotes(s: &str) -> &str {
        let s = s.trim();
        if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
            &s[1..s.len() - 1]
        } else {
            s
        }
    }

    /// Parse a single-stage Dockerfile (no FROM detected)
    fn parse_single_stage(content: &str) -> anyhow::Result<DockerfileInfo> {
        let mut info = DockerfileInfo {
            base_image: None,
            stage: None,
            expose_ports: Vec::new(),
            environment: IndexMap::new(),
            command: None,
            entrypoint: None,
            is_multi_stage: false,
        };

        for line in content.lines() {
            let line = line.trim();
            Self::parse_instruction(line, &mut info);
        }

        Ok(info)
    }

    /// Convert DockerfileInfo to DeploymentNode
    pub fn to_deployment_node(info: &DockerfileInfo, name: &str) -> DeploymentNode {
        let ports = info.expose_ports
            .iter()
            .map(|p| PortMapping {
                host: *p,
                container: *p,
                protocol: "tcp".to_string(),
            })
            .collect();

        let command = info.entrypoint.clone().or(info.command.clone());

        DeploymentNode {
            id: format!("node-{}", name.replace(' ', "-").to_lowercase()),
            name: name.to_string(),
            technology: "Docker".to_string(),
            base_image: info.base_image.clone(),
            ports,
            environment: info.environment.clone(),
            command,
            stage: info.stage.clone(),
        }
    }
}

/// Parser for docker-compose.yml files
#[derive(Debug, Clone)]
pub struct DockerComposeParser;

impl DockerComposeParser {
    /// Parse a docker-compose.yml file
    pub fn parse(compose_path: &Path) -> anyhow::Result<ComposeInfo> {
        let content = std::fs::read_to_string(compose_path)?;
        Self::parse_content(&content)
    }

    /// Parse docker-compose.yml content directly
    pub fn parse_content(content: &str) -> anyhow::Result<ComposeInfo> {
        // Try to parse as a composition of services at root level
        let parsed: serde_yaml::Value = serde_yaml::from_str(content)?;

        let mut services = Vec::new();
        let mut networks = Vec::new();
        let mut volumes = Vec::new();
        let mut dependencies = Vec::new();

        // Parse services
        if let Some(services_map) = parsed.get("services").and_then(|v| v.as_mapping()) {
            for (key, value) in services_map {
                let name = key.as_str().unwrap_or("unknown").to_string();
                if let Some(service) = Self::parse_service(&name, value) {
                    // Extract dependencies
                    if let Some(deps) = &service.depends_on {
                        for dep in deps {
                            dependencies.push(ComposeDependency {
                                from: name.clone(),
                                to: dep.clone(),
                            });
                        }
                    }
                    services.push(service);
                }
            }
        }

        // Parse networks
        if let Some(networks_map) = parsed.get("networks").and_then(|v| v.as_mapping()) {
            for (key, value) in networks_map {
                let name = key.as_str().unwrap_or("unknown").to_string();
                let driver = value
                    .as_mapping()
                    .and_then(|m| m.get("driver"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
                networks.push(ComposeNetwork { name, driver });
            }
        }

        // Parse volumes
        if let Some(volumes_map) = parsed.get("volumes").and_then(|v| v.as_mapping()) {
            for (key, value) in volumes_map {
                let name = key.as_str().unwrap_or("unknown").to_string();
                let driver = value
                    .as_mapping()
                    .and_then(|m| m.get("driver"))
                    .and_then(|v| v.as_str())
                    .map(String::from);
                volumes.push(ComposeVolume { name, driver });
            }
        }

        Ok(ComposeInfo {
            services,
            networks,
            volumes,
            dependencies,
        })
    }

    /// Parse a single service from YAML
    fn parse_service(name: &str, value: &serde_yaml::Value) -> Option<ComposeService> {
        let mapping = value.as_mapping()?;

        let image = mapping
            .get("image")
            .and_then(|v| v.as_str())
            .map(String::from);

        let ports = mapping
            .get("ports")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

        let environment = mapping
            .get("environment")
            .and_then(|v| {
                if v.is_mapping() {
                    Some(v.as_mapping().unwrap())
                } else if v.is_sequence() {
                    // Convert sequence to mapping
                    None // Skip sequence format for now
                } else {
                    None
                }
            })
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| {
                        let key = k.as_str()?.to_string();
                        let value = v.as_str().unwrap_or("").to_string();
                        Some((key, value))
                    })
                    .collect()
            });

        let command = mapping
            .get("command")
            .and_then(|v| {
                if v.is_sequence() {
                    v.as_sequence().map(|s| {
                        s.iter()
                            .filter_map(|v| v.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                } else {
                    v.as_str().map(String::from)
                }
            });

        let depends_on = mapping
            .get("depends_on")
            .and_then(|v| {
                if v.is_sequence() {
                    Some(v.as_sequence().unwrap())
                } else if v.is_mapping() {
                    // { service: { condition: service_healthy } } - can't extract simple list
                    None
                } else {
                    None
                }
            })
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

        Some(ComposeService {
            name: name.to_string(),
            image,
            ports,
            environment,
            command,
            depends_on,
        })
    }

    /// Convert ComposeInfo to DeploymentModel
    pub fn to_deployment_model(info: &ComposeInfo) -> DeploymentModel {
        let mut nodes = Vec::new();
        let mut relationships = Vec::new();

        // Convert services to deployment nodes
        for service in &info.services {
            let ports = service
                .ports
                .as_ref()
                .map(|ps| {
                    ps.iter()
                        .filter_map(|p| Self::parse_port_mapping(p))
                        .collect()
                })
                .unwrap_or_default();

            let environment = service.environment.clone().unwrap_or_default();

            nodes.push(DeploymentNode {
                id: format!("node-{}", service.name.replace(' ', "-").to_lowercase()),
                name: service.name.clone(),
                technology: service.image.clone().unwrap_or_else(|| "docker".to_string()),
                base_image: service.image.clone(),
                ports,
                environment,
                command: service.command.clone(),
                stage: None,
            });
        }

        // Convert networks to Network model
        let networks: Vec<Network> = info
            .networks
            .iter()
            .map(|n| Network {
                id: format!("network-{}", n.name.replace(' ', "-").to_lowercase()),
                name: n.name.clone(),
                driver: n.driver.clone(),
            })
            .collect();

        // Convert volumes to Volume model
        let volumes: Vec<Volume> = info
            .volumes
            .iter()
            .map(|v| Volume {
                id: format!("volume-{}", v.name.replace(' ', "-").to_lowercase()),
                name: v.name.clone(),
                driver: v.driver.clone(),
            })
            .collect();

        // Convert dependencies to relationships
        for dep in &info.dependencies {
            relationships.push(DeploymentRelationship {
                source: format!("node-{}", dep.from.replace(' ', "-").to_lowercase()),
                target: format!("node-{}", dep.to.replace(' ', "-").to_lowercase()),
                label: "depends_on".to_string(),
            });
        }

        DeploymentModel {
            nodes,
            networks,
            volumes,
            relationships,
        }
    }

    /// Parse port mapping string like "8080:80" or "8080"
    fn parse_port_mapping(s: &str) -> Option<PortMapping> {
        let parts: Vec<&str> = s.split(':').collect();

        match parts.len() {
            1 => {
                // Single port - assume same host and container
                let port: u16 = parts[0].parse().ok()?;
                Some(PortMapping {
                    host: port,
                    container: port,
                    protocol: "tcp".to_string(),
                })
            }
            2 => {
                // host:container format
                let host: u16 = parts[0].parse().ok()?;
                let container: u16 = parts[1].parse().ok()?;
                Some(PortMapping {
                    host,
                    container,
                    protocol: "tcp".to_string(),
                })
            }
            3 => {
                // host:protocol:container or host:container:protocol
                let host: u16 = parts[0].parse().ok()?;
                let (container, protocol) = if parts[2].parse::<u16>().is_ok() {
                    (parts[1].parse().ok()?, parts[2].to_string())
                } else {
                    (parts[1].parse().ok()?, parts[2].to_string())
                };
                Some(PortMapping {
                    host,
                    container,
                    protocol,
                })
            }
            _ => None,
        }
    }
}

/// Detect docker files in a project directory
pub fn detect_docker_files(project_dir: &Path) -> Option<PathBuf> {
    // Check for docker-compose.yml (most common)
    let compose_paths = [
        "docker-compose.yml",
        "docker-compose.yaml",
        "compose.yml",
        "compose.yaml",
    ];

    for name in &compose_paths {
        let path = project_dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }

    // Check for Dockerfile
    let dockerfile_paths = [
        "Dockerfile",
        "Dockerfile.dev",
        "Dockerfile.prod",
        "Dockerfile.staging",
    ];

    for name in &dockerfile_paths {
        let path = project_dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dockerfile_multi_stage() {
        let dockerfile = r#"
FROM golang:1.21 AS builder
WORKDIR /app
COPY . .
RUN go build -o main

FROM alpine:3.18 AS production
COPY --from=builder /app/main /usr/local/bin/
EXPOSE 8080
ENV NODE_ENV=production
CMD ["main"]
"#;

        let results = DockerfileParser::parse_content(dockerfile).unwrap();
        assert!(results.len() >= 2, "Expected multi-stage build with 2+ stages");
    }

    #[test]
    fn test_parse_dockerfile_expose() {
        let dockerfile = r#"
FROM nginx:latest
EXPOSE 80
EXPOSE 443
EXPOSE 8080
"#;

        let results = DockerfileParser::parse_content(dockerfile).unwrap();
        assert_eq!(results.len(), 1);
        let info = &results[0];
        assert_eq!(info.expose_ports, vec![80, 443, 8080]);
    }

    #[test]
    fn test_parse_dockerfile_env() {
        let dockerfile = r#"
FROM node:18
ENV NODE_ENV=development
ENV DATABASE_URL=postgres://localhost:5432/mydb
ENV API_KEY="secret-key"
"#;

        let results = DockerfileParser::parse_content(dockerfile).unwrap();
        assert_eq!(results.len(), 1);
        let info = &results[0];
        assert_eq!(info.environment.get("NODE_ENV").map(|s| s.as_str()), Some("development"));
        assert_eq!(info.environment.get("DATABASE_URL").map(|s| s.as_str()), Some("postgres://localhost:5432/mydb"));
        assert_eq!(info.environment.get("API_KEY").map(|s| s.as_str()), Some("secret-key"));
    }

    #[test]
    fn test_parse_compose_services() {
        let compose = r#"
services:
  api:
    image: myapp/api:latest
    ports:
      - "3000:3000"
    environment:
      DATABASE_URL: postgres://db:5432/api

  db:
    image: postgres:15
    ports:
      - "5432:5432"
    volumes:
      - db-data:/var/lib/postgresql/data

  cache:
    image: redis:7-alpine
    ports:
      - "6379:6379"

networks:
  default:
    driver: bridge

volumes:
  db-data:
    driver: local
"#;

        let info = DockerComposeParser::parse_content(compose).unwrap();
        assert_eq!(info.services.len(), 3, "Expected 3 services");
        assert_eq!(info.networks.len(), 1);
        assert_eq!(info.volumes.len(), 1);
    }

    #[test]
    fn test_parse_compose_networks() {
        let compose = r#"
services:
  web:
    image: nginx

networks:
  frontend:
    driver: bridge
  backend:
    driver: overlay
"#;

        let info = DockerComposeParser::parse_content(compose).unwrap();
        assert_eq!(info.networks.len(), 2);
        assert_eq!(info.networks[0].name, "frontend");
        assert_eq!(info.networks[0].driver.as_deref(), Some("bridge"));
        assert_eq!(info.networks[1].name, "backend");
        assert_eq!(info.networks[1].driver.as_deref(), Some("overlay"));
    }

    #[test]
    fn test_parse_compose_depends() {
        let compose = r#"
services:
  web:
    image: nginx
    depends_on:
      - api
      - db

  api:
    image: myapp/api

  db:
    image: postgres:15
"#;

        let info = DockerComposeParser::parse_content(compose).unwrap();
        assert_eq!(info.dependencies.len(), 2);
        // web depends on api
        let web_deps: Vec<_> = info.dependencies.iter().filter(|d| d.from == "web").collect();
        assert_eq!(web_deps.len(), 2);
    }

    #[test]
    fn test_port_mapping_formats() {
        assert_eq!(
            DockerComposeParser::parse_port_mapping("8080"),
            Some(PortMapping { host: 8080, container: 8080, protocol: "tcp".to_string() })
        );
        assert_eq!(
            DockerComposeParser::parse_port_mapping("8080:80"),
            Some(PortMapping { host: 8080, container: 80, protocol: "tcp".to_string() })
        );
    }

    #[test]
    fn test_detect_docker_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = detect_docker_files(temp_dir.path());
        assert!(path.is_none());

        // Create docker-compose.yml
        std::fs::write(temp_dir.path().join("docker-compose.yml"), "services:\n  web:\n    image: nginx").unwrap();
        let path = detect_docker_files(temp_dir.path()).unwrap();
        assert!(path.file_name().unwrap() == "docker-compose.yml");
    }
}
