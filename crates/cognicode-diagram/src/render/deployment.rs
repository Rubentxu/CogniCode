//! Deployment diagram renderer for Mermaid and D2 formats

use crate::model::deployment::{DeploymentModel, DeploymentNode, DeploymentRelationship};
use crate::render::d2::D2Options;

/// Escape text for safe inclusion in diagram syntax
fn escape_deployment(text: &str) -> String {
    text.replace('"', "'")
        .replace('[', "(")
        .replace(']', ")")
        .replace('{', "(")
        .replace('}', ")")
        .replace('<', "(")
        .replace('>', ")")
        .replace('&', "and")
        .replace('\n', " ")
        .replace('\r', "")
}

#[derive(Debug, Clone, Default)]
pub struct DeploymentMermaidOptions {
    /// Show port mappings in the diagram
    pub show_ports: bool,
    /// Show environment variables
    pub show_environment: bool,
    /// Show network labels
    pub show_networks: bool,
    /// Diagram direction: "TB", "BT", "LR", "RL"
    pub direction: String,
}

/// Renders a deployment model as a Mermaid deployment diagram.
///
/// Produces a Mermaid flowchart showing deployment nodes (containers, databases, etc.)
/// connected by relationships, with optional network groupings.
///
/// # Examples
///
/// ```
/// use cognicode_diagram::model::deployment::{DeploymentModel, DeploymentNode};
/// use cognicode_diagram::render::deployment::render_deployment_mermaid;
///
/// let model = DeploymentModel::empty();
/// let mermaid = render_deployment_mermaid(&model);
/// assert!(mermaid.contains("flowchart"));
/// ```
pub fn render_deployment_mermaid(model: &DeploymentModel) -> String {
    let mut lines = Vec::new();

    lines.push("flowchart TB".to_string());
    lines.push("    %% Deployment Diagram".to_string());

    // Group nodes by their network if networks are defined
    let has_networks = !model.networks.is_empty();

    if has_networks {
        // Render networks as subgraphs
        for network in &model.networks {
            let network_id = format!("network_{}", network.name.replace('-', "_").replace(' ', "_"));
            lines.push(format!("    subgraph {}", network_id));
            lines.push(format!("        direction TB"));
            lines.push(format!("        label[\"{}\"]", escape_deployment(&network.name)));

            // Find nodes in this network (for now, all nodes are in default network)
            for node in &model.nodes {
                lines.push(render_node_mermaid(node, &model.relationships));
            }

            lines.push("    end".to_string());
        }

        // Also render nodes not in any network
        let _network_node_ids: Vec<&str> = model.networks.iter()
            .map(|n| n.name.as_str())
            .collect();

        for node in &model.nodes {
            let _node_id = node.id.replace('-', "_");
            // Check if this node is already rendered
            let already_rendered = model.networks.iter().any(|_net| {
                // Simple heuristic: if there are networks, assume nodes belong to first network
                false // We'll render all nodes outside networks too
            });

            if !already_rendered {
                lines.push(render_node_mermaid(node, &model.relationships));
            }
        }
    } else {
        // No networks - just render nodes
        for node in &model.nodes {
            lines.push(render_node_mermaid(node, &model.relationships));
        }
    }

    // Render relationships
    for rel in &model.relationships {
        let source = rel.source.replace('-', "_");
        let target = rel.target.replace('-', "_");
        let label = if rel.label.is_empty() {
            String::new()
        } else {
            format!(" : {}", escape_deployment(&rel.label))
        };
        lines.push(format!("    {} --> {}{}", source, target, label));
    }

    lines.join("\n")
}

/// Render a single deployment node in Mermaid format
fn render_node_mermaid(node: &DeploymentNode, _relationships: &[DeploymentRelationship]) -> String {
    let node_id = node.id.replace('-', "_");
    let name = escape_deployment(&node.name);

    // Determine shape based on technology/image hints
    let technology_lower = node.technology.to_lowercase();

    // Build ports string if available
    let ports_str = if node.ports.is_empty() {
        String::new()
    } else {
        let port_strs: Vec<String> = node.ports.iter()
            .map(|p| format!("{}:{}", p.host, p.container))
            .collect();
        format!("\\nPorts: {}", port_strs.join(", "))
    };

    if technology_lower.contains("database") || technology_lower.contains("postgres") ||
       technology_lower.contains("mysql") || technology_lower.contains("mongodb") ||
       technology_lower.contains("redis") || technology_lower.contains("db") {
        // Cylinder for databases
        format!("    {}[(\"{}{}\")] {}", node_id, name, ports_str, node.technology)
    } else if technology_lower.contains("nginx") || technology_lower.contains("apache") ||
              technology_lower.contains("gateway") || technology_lower.contains("proxy") {
        // Hexagon for gateways
        format!("    {}{{ \"{}\" }}", node_id, name)
    } else if technology_lower.contains("queue") || technology_lower.contains("rabbitmq") ||
              technology_lower.contains("kafka") || technology_lower.contains("mq") {
        // Queue shape: hexagon
        format!("    {}{{ \"{}\" }}", node_id, name)
    } else {
        // Rectangle for services
        format!("    {}[\"{}{}\"]", node_id, name, ports_str)
    }
}

/// Renders a deployment model as a D2 diagram.
///
/// Produces D2 diagram source showing deployment nodes (containers, databases, etc.)
/// with their technology stack, port mappings, and relationships.
///
/// # Examples
///
/// ```
/// use cognicode_diagram::model::deployment::DeploymentModel;
/// use cognicode_diagram::render::d2::D2Options;
/// use cognicode_diagram::render::deployment::render_deployment_d2;
///
/// let model = DeploymentModel::empty();
/// let options = D2Options::default();
/// let d2 = render_deployment_d2(&model, &options);
/// assert!(d2.contains("direction"));
/// ```
pub fn render_deployment_d2(model: &DeploymentModel, options: &D2Options) -> String {
    let mut lines = Vec::new();

    // D2 header with direction
    let direction = match options.direction {
        crate::render::d2::D2Direction::Down => "down",
        crate::render::d2::D2Direction::Right => "right",
    };
    lines.push(format!("direction: {}", direction));

    // Theme
    if options.sketch {
        lines.push("sketch: true".to_string());
    }

    lines.push(format!("pad: {}", options.pad));

    // Render networks as D2 groups
    for network in &model.networks {
        let network_id = format!("net_{}", network.name.replace('-', "_").replace(' ', "_"));
        lines.push(format!("{}: {{", network_id));
        lines.push(format!("    shape: rectangle"));
        lines.push(format!("    label: \"{}\"", escape_deployment(&network.name)));

        // Add nodes in this network as children
        for node in &model.nodes {
            let node_id = node.id.replace('-', "_");
            lines.push(format!("    {}: {{", node_id));
            lines.push(render_node_d2(node));
            lines.push(format!("    }}"));
        }

        lines.push("}".to_string());
    }

    // Render standalone nodes (not in any network)
    for node in &model.nodes {
        let node_id = node.id.replace('-', "_");
        lines.push(format!("{}: {{", node_id));
        lines.push(render_node_d2(node));
        lines.push("}".to_string());
    }

    // Render relationships as D2 edges
    for rel in &model.relationships {
        let source = rel.source.replace('-', "_");
        let target = rel.target.replace('-', "_");
        let label = if rel.label.is_empty() {
            String::new()
        } else {
            format!("label: \"{}\"", escape_deployment(&rel.label))
        };
        lines.push(format!("{} -> {} @{}", source, target, label));
    }

    lines.join("\n")
}

/// Render a single deployment node in D2 format
fn render_node_d2(node: &DeploymentNode) -> String {
    let mut parts = Vec::new();

    // Add label
    parts.push(format!("label: \"{}\"", escape_deployment(&node.name)));

    // Add shape based on technology
    let tech_lower = node.technology.to_lowercase();
    let shape = if tech_lower.contains("database") || tech_lower.contains("postgres") ||
                  tech_lower.contains("mysql") || tech_lower.contains("mongodb") ||
                  tech_lower.contains("redis") {
        "cylinder"
    } else if tech_lower.contains("queue") || tech_lower.contains("rabbitmq") ||
              tech_lower.contains("kafka") {
        "queue"
    } else {
        "rectangle"
    };
    parts.push(format!("shape: {}", shape));

    // Add ports if available
    if !node.ports.is_empty() {
        let port_strs: Vec<String> = node.ports.iter()
            .map(|p| format!("{}/{}", p.container, p.protocol))
            .collect();
        parts.push(format!("ports: [{}]", port_strs.join(", ")));
    }

    // Add environment info (just keys, not values for brevity)
    if !node.environment.is_empty() {
        let env_keys: Vec<String> = node.environment.keys().map(|s| s.clone()).collect();
        parts.push(format!("environment: [{}]", env_keys.join(", ")));
    }

    parts.join("\n    ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::deployment::{DeploymentModel, DeploymentNode, DeploymentRelationship, Network, Volume};
    use indexmap::IndexMap;

    #[test]
    fn test_render_deployment_mermaid_empty() {
        let model = DeploymentModel::empty();
        let result = render_deployment_mermaid(&model);
        assert!(result.contains("flowchart TB"));
        assert!(result.contains("Deployment Diagram"));
    }

    #[test]
    fn test_render_deployment_mermaid_single_node() {
        let mut env = IndexMap::new();
        env.insert("NODE_ENV".to_string(), "production".to_string());

        let model = DeploymentModel {
            nodes: vec![DeploymentNode {
                id: "node-api".to_string(),
                name: "API Service".to_string(),
                technology: "nginx".to_string(),
                base_image: Some("nginx:latest".to_string()),
                ports: vec![crate::model::deployment::PortMapping {
                    host: 80,
                    container: 80,
                    protocol: "tcp".to_string(),
                }],
                environment: env,
                command: None,
                stage: None,
            }],
            networks: vec![],
            volumes: vec![],
            relationships: vec![],
        };

        let result = render_deployment_mermaid(&model);
        assert!(result.contains("node_api"));
        assert!(result.contains("API Service"));
    }

    #[test]
    fn test_render_deployment_mermaid_with_relationships() {
        let model = DeploymentModel {
            nodes: vec![
                DeploymentNode {
                    id: "node-web".to_string(),
                    name: "Web".to_string(),
                    technology: "nginx".to_string(),
                    base_image: None,
                    ports: vec![],
                    environment: IndexMap::new(),
                    command: None,
                    stage: None,
                },
                DeploymentNode {
                    id: "node-api".to_string(),
                    name: "API".to_string(),
                    technology: "golang".to_string(),
                    base_image: None,
                    ports: vec![],
                    environment: IndexMap::new(),
                    command: None,
                    stage: None,
                },
            ],
            networks: vec![],
            volumes: vec![],
            relationships: vec![
                DeploymentRelationship {
                    source: "node-web".to_string(),
                    target: "node-api".to_string(),
                    label: "calls".to_string(),
                },
            ],
        };

        let result = render_deployment_mermaid(&model);
        assert!(result.contains("node_web --> node_api : calls"));
    }

    #[test]
    fn test_render_deployment_d2() {
        let model = DeploymentModel {
            nodes: vec![DeploymentNode {
                id: "node-db".to_string(),
                name: "Database".to_string(),
                technology: "postgres".to_string(),
                base_image: Some("postgres:15".to_string()),
                ports: vec![crate::model::deployment::PortMapping {
                    host: 5432,
                    container: 5432,
                    protocol: "tcp".to_string(),
                }],
                environment: IndexMap::new(),
                command: None,
                stage: None,
            }],
            networks: vec![Network {
                id: "network-default".to_string(),
                name: "default".to_string(),
                driver: Some("bridge".to_string()),
            }],
            volumes: vec![],
            relationships: vec![],
        };

        let options = D2Options::default();
        let result = render_deployment_d2(&model, &options);
        assert!(result.contains("direction: down"));
        assert!(result.contains("node_db"));
        assert!(result.contains("Database"));
    }

    #[test]
    fn test_render_deployment_d2_with_relationships() {
        let model = DeploymentModel {
            nodes: vec![
                DeploymentNode {
                    id: "node-api".to_string(),
                    name: "API".to_string(),
                    technology: "golang".to_string(),
                    base_image: None,
                    ports: vec![],
                    environment: IndexMap::new(),
                    command: None,
                    stage: None,
                },
                DeploymentNode {
                    id: "node-db".to_string(),
                    name: "DB".to_string(),
                    technology: "postgres".to_string(),
                    base_image: None,
                    ports: vec![],
                    environment: IndexMap::new(),
                    command: None,
                    stage: None,
                },
            ],
            networks: vec![],
            volumes: vec![],
            relationships: vec![
                DeploymentRelationship {
                    source: "node-api".to_string(),
                    target: "node-db".to_string(),
                    label: "connects_to".to_string(),
                },
            ],
        };

        let options = D2Options::default();
        let result = render_deployment_d2(&model, &options);
        assert!(result.contains("node_api -> node_db"));
        assert!(result.contains("connects_to"));
    }
}
