//! C4 Views — diagram view definitions

use serde::{Deserialize, Serialize};

/// System Context view configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemContextView {
    pub software_system_id: String,
    pub title: String,
    pub description: String,
    pub auto_layout: bool,
    pub enterprise_boundary_visible: bool,
}

/// Container view configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerView {
    pub software_system_id: String,
    pub title: String,
    pub description: String,
    pub auto_layout: bool,
}

/// Component view configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentView {
    pub container_id: String,
    pub title: String,
    pub description: String,
    pub auto_layout: bool,
}

/// Code view configuration (class diagram)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeView {
    pub path: String,
    pub title: String,
    pub max_depth: u8,
    pub visibility_filter: Option<String>,
}

/// Dynamic view configuration (sequence diagram)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicView {
    pub element_id: String,
    pub title: String,
    pub max_depth: u8,
}
