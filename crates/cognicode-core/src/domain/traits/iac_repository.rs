//! IaC Repository trait — Infrastructure-as-Code resource queries (Terraform, Ansible).
//! Stub implementation pending full IaC extraction pipeline.

use async_trait::async_trait;

/// Result of querying an IaC resource.
#[derive(Debug, Clone)]
pub struct IacResource {
    pub id: String,
    pub name: String,
    pub resource_type: String,
}

/// An edge between two IaC resources.
#[derive(Debug, Clone)]
pub struct IacEdge {
    pub target_id: String,
    pub target: Option<IacResource>,
    pub edge_type: String,
    pub confidence: Option<f32>,
}

/// Repository for IaC resource queries.
#[async_trait]
pub trait IacRepository: Send + Sync {
    async fn find_resource(&self, resource_id: &str) -> Result<Option<IacResource>, String>;
    async fn get_dependencies(&self, resource_id: &str, depth: Option<u32>) -> Result<Vec<IacEdge>, String>;
    async fn get_dependents(&self, resource_id: &str, depth: Option<u32>) -> Result<Vec<IacEdge>, String>;
}
