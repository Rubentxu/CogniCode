//! C4 relationships between elements

use serde::{Deserialize, Serialize};
use super::c4_types::ElementId;

/// Kind of relationship between C4 elements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum C4RelationshipKind {
    Uses,
    Calls,
    DependsOn,
    SendsTo,
    ReadsFrom,
    WritesTo,
    Inherits,
    Implements,
    Composes,
    Aggregates,
}

impl std::fmt::Display for C4RelationshipKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            C4RelationshipKind::Uses => "uses",
            C4RelationshipKind::Calls => "calls",
            C4RelationshipKind::DependsOn => "depends on",
            C4RelationshipKind::SendsTo => "sends to",
            C4RelationshipKind::ReadsFrom => "reads from",
            C4RelationshipKind::WritesTo => "writes to",
            C4RelationshipKind::Inherits => "inherits from",
            C4RelationshipKind::Implements => "implements",
            C4RelationshipKind::Composes => "composes",
            C4RelationshipKind::Aggregates => "aggregates",
        };
        write!(f, "{}", s)
    }
}

/// A relationship between two C4 elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C4Relationship {
    pub source_id: ElementId,
    pub target_id: ElementId,
    pub kind: C4RelationshipKind,
    pub label: Option<String>,
    pub technology: Option<String>,
    pub confidence: f64,
}

impl C4Relationship {
    pub fn new(source: ElementId, target: ElementId, kind: C4RelationshipKind) -> Self {
        Self {
            source_id: source,
            target_id: target,
            kind,
            label: None,
            technology: None,
            confidence: 1.0,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_technology(mut self, tech: impl Into<String>) -> Self {
        self.technology = Some(tech.into());
        self
    }

    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }
}
