//! UML rule engine — maps DependencyType to UML relationships with confidence scores

use std::collections::HashMap;

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::value_objects::dependency_type::DependencyType;

use crate::model::c4_types::{ElementId, UmlRelationKind, UmlRelationship};

/// Result of applying an inference rule
#[derive(Debug, Clone)]
pub struct RuleResult {
    pub target_id: ElementId,
    pub kind: UmlRelationKind,
    pub label: Option<String>,
    pub confidence: f64,
}

/// Rule-based UML relationship inference engine
#[derive(Debug, Clone)]
pub struct UmlRuleEngine {
    /// Minimum confidence threshold (0.0–1.0) for including a relationship
    min_confidence: f64,
}

impl UmlRuleEngine {
    pub fn new() -> Self {
        Self {
            min_confidence: 0.3,
        }
    }

    pub fn with_min_confidence(mut self, confidence: f64) -> Self {
        self.min_confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Map a DependencyType to a UML relationship kind with confidence score
    pub fn infer_from_dependency(&self, dep_type: DependencyType) -> Option<RuleResult> {
        let (kind, confidence, label) = match dep_type {
            DependencyType::Inherits => (
                UmlRelationKind::Inheritance,
                1.0,
                Some("inherits from".to_string()),
            ),
            DependencyType::Contains => (
                UmlRelationKind::Composition,
                0.9,
                Some("contains".to_string()),
            ),
            DependencyType::References => (
                UmlRelationKind::Association,
                0.7,
                Some("references".to_string()),
            ),
            DependencyType::UsesGeneric => (
                UmlRelationKind::Dependency,
                0.8,
                Some("uses generic".to_string()),
            ),
            DependencyType::Calls => (
                UmlRelationKind::Dependency,
                0.6,
                Some("calls".to_string()),
            ),
            DependencyType::Imports => (
                UmlRelationKind::Dependency,
                0.5,
                Some("imports".to_string()),
            ),
            DependencyType::AnnotatedBy => (
                UmlRelationKind::Dependency,
                0.4,
                Some("annotated by".to_string()),
            ),
            DependencyType::Defines => return None, // Skip — structural, not a relationship
        };

        Some(RuleResult {
            target_id: ElementId::new(""),
            kind,
            label,
            confidence,
        })
    }

    /// Map a DependencyType to UML relationship kind (returns kind + confidence only)
    pub fn map_dependency(dep_type: DependencyType) -> Option<(UmlRelationKind, f64)> {
        match dep_type {
            DependencyType::Inherits => Some((UmlRelationKind::Inheritance, 1.0)),
            DependencyType::Contains => Some((UmlRelationKind::Composition, 0.9)),
            DependencyType::References => Some((UmlRelationKind::Association, 0.7)),
            DependencyType::UsesGeneric => Some((UmlRelationKind::Dependency, 0.8)),
            DependencyType::Calls => Some((UmlRelationKind::Dependency, 0.6)),
            DependencyType::Imports => Some((UmlRelationKind::Dependency, 0.5)),
            DependencyType::AnnotatedBy => Some((UmlRelationKind::Dependency, 0.4)),
            DependencyType::Defines => None,
        }
    }

    /// Infer UML relationships from CallGraph dependencies
    ///
    /// `element_ids` maps SymbolId (as string) → ElementId for resolved code elements
    pub fn infer_uml_relationships(
        &self,
        call_graph: &CallGraph,
        element_ids: &HashMap<String, ElementId>,
    ) -> Vec<UmlRelationship> {
        let mut relationships = Vec::new();

        // Iterate all dependency edges (3-tuple: source, target, dep_type)
        for (source_sym_id, target_sym_id, dep_type) in call_graph.all_dependencies() {
            let source_element_id = match element_ids.get(source_sym_id.as_str()) {
                Some(id) => id.clone(),
                None => continue,
            };

            let target_element_id = match element_ids.get(target_sym_id.as_str()) {
                Some(id) => id.clone(),
                None => continue,
            };

            if let Some((kind, confidence)) = Self::map_dependency(*dep_type) {
                if confidence >= self.min_confidence && source_element_id != target_element_id {
                    relationships.push(UmlRelationship {
                        target_id: target_element_id,
                        kind,
                        label: None,
                        confidence,
                    });
                }
            }

            let _ = source_element_id; // Used above
        }

        relationships
    }
}

impl Default for UmlRuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_from_dependency_inherits() {
        let engine = UmlRuleEngine::new();
        let result = engine.infer_from_dependency(DependencyType::Inherits).unwrap();
        assert_eq!(result.kind, UmlRelationKind::Inheritance);
        assert!((result.confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_infer_from_dependency_defines_skipped() {
        let engine = UmlRuleEngine::new();
        assert!(engine.infer_from_dependency(DependencyType::Defines).is_none());
    }

    #[test]
    fn test_map_dependency_all_variants() {
        assert_eq!(
            UmlRuleEngine::map_dependency(DependencyType::Inherits).unwrap().0,
            UmlRelationKind::Inheritance
        );
        assert_eq!(
            UmlRuleEngine::map_dependency(DependencyType::Contains).unwrap().0,
            UmlRelationKind::Composition
        );
        assert_eq!(
            UmlRuleEngine::map_dependency(DependencyType::References).unwrap().0,
            UmlRelationKind::Association
        );
        assert!(UmlRuleEngine::map_dependency(DependencyType::Defines).is_none());
    }

    #[test]
    fn test_min_confidence_filter() {
        let engine = UmlRuleEngine::new().with_min_confidence(0.9);
        let result = engine.infer_from_dependency(DependencyType::Calls).unwrap();
        // Calls has confidence 0.6, but the rule itself returns the value;
        // filtering happens in infer_uml_relationships
        assert!((result.confidence - 0.6).abs() < f64::EPSILON);
    }
}
