//! C4 Model element types

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unique identifier for a C4 element
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElementId(String);

impl ElementId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ElementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Location of an element relative to the system boundary
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElementLocation {
    Internal,
    External,
}

/// Visibility of a code element
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Package,
}

/// A person (actor) in the C4 model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub id: ElementId,
    pub name: String,
    pub description: String,
    pub location: ElementLocation,
}

/// A software system in the C4 model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareSystem {
    pub id: ElementId,
    pub name: String,
    pub description: String,
    pub location: ElementLocation,
    pub containers: Vec<Container>,
}

/// Type of container (deployment unit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerType {
    Service,
    Library,
    DataStore,
    Executable,
    Queue,
}

/// A container within a software system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: ElementId,
    pub name: String,
    pub container_type: ContainerType,
    pub technology: String,
    pub description: String,
    pub path: Option<PathBuf>,
    pub components: Vec<Component>,
}

/// Type of component within a container
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentType {
    Module,
    Interface,
    Controller,
    Repository,
    Service,
}

/// A component within a container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub id: ElementId,
    pub name: String,
    pub component_type: ComponentType,
    pub technology: String,
    pub description: String,
    pub path: Option<PathBuf>,
    pub code_elements: Vec<CodeElement>,
}

/// Kind of code element (maps from SymbolKind)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodeElementKind {
    Class,
    Struct,
    Enum,
    Interface,
    Function,
    Method,
    Constructor,
    Field,
    Constant,
}

/// An attribute (field) of a code element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attribute {
    pub name: String,
    pub type_annotation: Option<String>,
    pub visibility: Visibility,
}

/// A method of a code element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Method {
    pub name: String,
    pub parameters: Vec<(String, Option<String>)>,
    pub return_type: Option<String>,
    pub visibility: Visibility,
    pub is_async: bool,
}

/// A code-level element (class, struct, enum, trait, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeElement {
    pub id: ElementId,
    pub name: String,
    pub kind: CodeElementKind,
    pub visibility: Visibility,
    pub path: Option<String>,
    pub attributes: Vec<Attribute>,
    pub methods: Vec<Method>,
    pub relationships: Vec<UmlRelationship>,
}

/// UML relationship between code elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UmlRelationship {
    pub target_id: ElementId,
    pub kind: UmlRelationKind,
    pub label: Option<String>,
    pub confidence: f64,
}

/// Kind of UML relationship
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UmlRelationKind {
    Inheritance,
    Realization,
    Composition,
    Aggregation,
    Association,
    Dependency,
}

impl std::fmt::Display for UmlRelationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UmlRelationKind::Inheritance => write!(f, "inheritance"),
            UmlRelationKind::Realization => write!(f, "realization"),
            UmlRelationKind::Composition => write!(f, "composition"),
            UmlRelationKind::Aggregation => write!(f, "aggregation"),
            UmlRelationKind::Association => write!(f, "association"),
            UmlRelationKind::Dependency => write!(f, "dependency"),
        }
    }
}

/// Top-level C4 element enum for heterogeneous collections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum C4Element {
    Person(Person),
    SoftwareSystem(SoftwareSystem),
    Container(Container),
    Component(Component),
    CodeElement(CodeElement),
}

impl C4Element {
    pub fn id(&self) -> &ElementId {
        match self {
            C4Element::Person(p) => &p.id,
            C4Element::SoftwareSystem(s) => &s.id,
            C4Element::Container(c) => &c.id,
            C4Element::Component(c) => &c.id,
            C4Element::CodeElement(c) => &c.id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            C4Element::Person(p) => &p.name,
            C4Element::SoftwareSystem(s) => &s.name,
            C4Element::Container(c) => &c.name,
            C4Element::Component(c) => &c.name,
            C4Element::CodeElement(c) => &c.name,
        }
    }
}
