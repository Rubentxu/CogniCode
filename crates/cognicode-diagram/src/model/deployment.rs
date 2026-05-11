//! Deployment model types for Docker/infrastructure diagrams

use serde::{Deserialize, Serialize};
use indexmap::IndexMap;

/// Deployment model representing infrastructure topology
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentModel {
    pub nodes: Vec<DeploymentNode>,
    pub networks: Vec<Network>,
    pub volumes: Vec<Volume>,
    pub relationships: Vec<DeploymentRelationship>,
}

impl DeploymentModel {
    pub fn empty() -> Self {
        Self { nodes: vec![], networks: vec![], volumes: vec![], relationships: vec![] }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentNode {
    pub id: String,
    pub name: String,
    pub technology: String,
    pub base_image: Option<String>,
    pub ports: Vec<PortMapping>,
    pub environment: IndexMap<String, String>,
    pub command: Option<String>,
    pub stage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    pub id: String,
    pub name: String,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub id: String,
    pub name: String,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortMapping {
    pub host: u16,
    pub container: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentRelationship {
    pub source: String,
    pub target: String,
    pub label: String,
}