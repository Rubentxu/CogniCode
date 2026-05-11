//! Deployment Inference — extracts deployment topology from Docker configurations
//!
//! Scans for Dockerfile and docker-compose.yml files, parses them,
//! and merges results into a DeploymentModel.

use std::path::Path;

use crate::inference::config_parsers::docker::{
    detect_docker_files, DockerfileParser, DockerComposeParser,
};
use crate::model::deployment::{DeploymentModel, DeploymentNode, DeploymentRelationship};

/// Infers deployment topology from Docker configurations.
///
/// Scans the project directory for `Dockerfile` and `docker-compose.yml` files,
/// parses them, and produces a `DeploymentModel` representing the infrastructure.
///
/// For Dockerfile: Extracts base image, exposed ports, environment variables,
/// and creates nodes for multi-stage builds.
///
/// For docker-compose.yml: Extracts services, their images, ports, environment
/// variables, volumes, and networks, plus relationships from `depends_on`.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use cognicode_diagram::inference::deployment_inference::infer_deployment;
/// use cognicode_diagram::render::deployment::render_deployment_mermaid;
///
/// # let project_dir = Path::new("/path/to/project");
/// let model = infer_deployment(project_dir)?;
/// let mermaid = render_deployment_mermaid(&model);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn infer_deployment(project_dir: &Path) -> anyhow::Result<DeploymentModel> {
    let Some(docker_path) = detect_docker_files(project_dir) else {
        // No docker files found - return empty model
        return Ok(DeploymentModel::empty());
    };

    let filename = docker_path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // Determine file type and parse accordingly
    if filename.contains("compose") || filename.contains("Compose") {
        // docker-compose.yml
        let compose_info = DockerComposeParser::parse(&docker_path)?;
        let model = DockerComposeParser::to_deployment_model(&compose_info);

        // Add relationships from depends_on
        return Ok(model);
    } else {
        // Dockerfile
        let dockerfiles = DockerfileParser::parse(&docker_path)?;

        if dockerfiles.is_empty() {
            return Ok(DeploymentModel::empty());
        }

        // If multiple stages, create a node for each
        let nodes: Vec<DeploymentNode> = dockerfiles
            .iter()
            .enumerate()
            .map(|(idx, info)| {
                let name = info.stage.clone()
                    .unwrap_or_else(|| {
                        if dockerfiles.len() == 1 {
                            "app".to_string()
                        } else {
                            format!("stage{}", idx + 1)
                        }
                    });
                DockerfileParser::to_deployment_node(info, &name)
            })
            .collect();

        // Create relationships between multi-stage build stages
        let relationships = if dockerfiles.len() > 1 {
            dockerfiles.iter()
                .enumerate()
                .skip(1)
                .map(|(idx, _)| {
                    DeploymentRelationship {
                        source: format!("node-stage{}", idx),
                        target: format!("node-stage{}", idx + 1),
                        label: "copies_to".to_string(),
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        return Ok(DeploymentModel {
            nodes,
            networks: Vec::new(),
            volumes: Vec::new(),
            relationships,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_deployment_no_docker() {
        let temp_dir = tempfile::tempdir().unwrap();
        let model = infer_deployment(temp_dir.path()).unwrap();
        assert!(model.nodes.is_empty());
        assert!(model.networks.is_empty());
        assert!(model.volumes.is_empty());
        assert!(model.relationships.is_empty());
    }

    #[test]
    fn test_infer_deployment_single_dockerfile() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_dir.path().join("Dockerfile"),
            r#"
FROM nginx:latest
EXPOSE 80 443
ENV NODE_ENV=production
"#,
        )
        .unwrap();

        let model = infer_deployment(temp_dir.path()).unwrap();
        assert_eq!(model.nodes.len(), 1);
        assert_eq!(model.nodes[0].ports.len(), 2);
        assert_eq!(model.nodes[0].environment.get("NODE_ENV").map(|s| s.as_str()), Some("production"));
    }

    #[test]
    fn test_infer_deployment_docker_compose() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            temp_dir.path().join("docker-compose.yml"),
            r#"
services:
  api:
    image: myapp/api
    ports:
      - "3000:3000"
    depends_on:
      - db

  db:
    image: postgres:15
    ports:
      - "5432:5432"

networks:
  default:
    driver: bridge
"#,
        )
        .unwrap();

        let model = infer_deployment(temp_dir.path()).unwrap();
        assert_eq!(model.nodes.len(), 2);
        assert_eq!(model.networks.len(), 1);
        assert_eq!(model.relationships.len(), 1);
    }
}
