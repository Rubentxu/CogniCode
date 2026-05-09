//! C4 Workspace — container for model + views + styles

use serde::{Deserialize, Serialize};
use super::c4_types::{C4Element, Person, SoftwareSystem};
use super::relationships::C4Relationship;

/// C4 Workspace containing the full model and views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C4Workspace {
    pub name: String,
    pub description: String,
    pub model: C4Model,
    pub views: Vec<C4View>,
}

/// The C4 model (elements + relationships)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C4Model {
    pub people: Vec<Person>,
    pub systems: Vec<SoftwareSystem>,
    pub relationships: Vec<C4Relationship>,
}

impl C4Model {
    pub fn new() -> Self {
        Self {
            people: Vec::new(),
            systems: Vec::new(),
            relationships: Vec::new(),
        }
    }
}

impl Default for C4Model {
    fn default() -> Self {
        Self::new()
    }
}

/// A view (diagram) of the C4 model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C4View {
    pub key: String,
    pub title: String,
    pub description: String,
    pub view_type: C4ViewType,
}

/// Type of C4 view
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum C4ViewType {
    SystemContext,
    Container,
    Component,
    Code,
    Dynamic,
    Deployment,
}

impl C4Workspace {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            model: C4Model::new(),
            views: Vec::new(),
        }
    }
}
