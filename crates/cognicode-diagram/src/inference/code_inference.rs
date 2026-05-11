//! Code-level (L4) inference — extract CodeElement structs from CallGraph symbols

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::aggregates::symbol::Symbol;
use cognicode_core::domain::value_objects::dependency_type::DependencyType;
use cognicode_core::domain::value_objects::symbol_kind::SymbolKind;

use crate::model::c4_types::{
    Attribute, CodeElement, CodeElementKind, ElementId, Method, Visibility,
};

/// L4 Code-level inference engine
#[derive(Debug, Clone)]
pub struct CodeInference {
    /// Whether to include visibility markers
    pub show_visibility: bool,
    /// Whether to include method parameters
    pub show_parameters: bool,
}

impl CodeInference {
    pub fn new() -> Self {
        Self {
            show_visibility: true,
            show_parameters: true,
        }
    }

    /// Map a SymbolKind from cognicode-core to our CodeElementKind
    pub fn map_symbol_kind(kind: SymbolKind) -> CodeElementKind {
        match kind {
            SymbolKind::Class => CodeElementKind::Class,
            SymbolKind::Struct => CodeElementKind::Struct,
            SymbolKind::Enum => CodeElementKind::Enum,
            SymbolKind::Interface | SymbolKind::Trait => CodeElementKind::Interface,
            SymbolKind::Function => CodeElementKind::Function,
            SymbolKind::Method => CodeElementKind::Method,
            SymbolKind::Constructor => CodeElementKind::Constructor,
            SymbolKind::Field | SymbolKind::Property => CodeElementKind::Field,
            SymbolKind::Constant => CodeElementKind::Constant,
            _ => CodeElementKind::Class, // Default fallback
        }
    }

    /// Map a visibility string to our Visibility enum
    pub fn map_visibility(vis: Option<&str>) -> Visibility {
        match vis {
            Some("public") => Visibility::Public,
            Some("private") => Visibility::Private,
            Some("protected") => Visibility::Protected,
            Some("crate" | "package") => Visibility::Package,
            _ => Visibility::Public, // Default to public
        }
    }

    /// Check if a symbol kind is a type definition worth showing at L4
    pub fn is_type_like(kind: SymbolKind) -> bool {
        matches!(
            kind,
            SymbolKind::Class
                | SymbolKind::Struct
                | SymbolKind::Enum
                | SymbolKind::Trait
                | SymbolKind::Interface
                | SymbolKind::Type
        )
    }

    /// Infer a CodeElement from a Symbol using CallGraph context
    pub fn infer_from_symbol(&self, symbol_id: &str, symbol: &Symbol, call_graph: &CallGraph) -> CodeElement {
        let id = ElementId::new(symbol_id);
        let kind = Self::map_symbol_kind(*symbol.kind());

        // Use FQN if available, otherwise just name
        let name = symbol.fully_qualified_name().to_string();

        // Get file path from Location
        let path = Some(symbol.location().file().to_string());

        let methods = self.infer_methods(symbol_id, call_graph);
        let attributes = self.infer_attributes(symbol_id, call_graph);

        CodeElement {
            id,
            name,
            kind,
            visibility: Visibility::Public, // Visibility not available from Symbol
            path,
            attributes,
            methods,
            relationships: Vec::new(), // Filled by UmlRuleEngine later
        }
    }

    /// Find methods belonging to a symbol via `Contains` dependencies
    pub fn infer_methods(&self, symbol_id: &str, call_graph: &CallGraph) -> Vec<Method> {
        let mut methods = Vec::new();

        let sym_id = cognicode_core::domain::aggregates::call_graph::SymbolId::new(symbol_id);

        for (dep_id, dep_type) in call_graph.dependencies(&sym_id) {
            if *dep_type != DependencyType::Contains {
                continue;
            }

            if let Some(child) = call_graph.get_symbol(dep_id) {
                if child.kind().is_callable() {
                    methods.push(Method {
                        name: child.name().to_string(),
                        parameters: Vec::new(),
                        return_type: None,
                        visibility: Visibility::Public,
                        is_async: false,
                    });
                }
            }
        }

        methods
    }

    /// Find attributes (fields/properties) belonging to a symbol via `Contains` dependencies
    pub fn infer_attributes(&self, symbol_id: &str, call_graph: &CallGraph) -> Vec<Attribute> {
        let mut attributes = Vec::new();

        let sym_id = cognicode_core::domain::aggregates::call_graph::SymbolId::new(symbol_id);

        for (dep_id, dep_type) in call_graph.dependencies(&sym_id) {
            if *dep_type != DependencyType::Contains {
                continue;
            }

            if let Some(child) = call_graph.get_symbol(dep_id) {
                if matches!(child.kind(), SymbolKind::Field | SymbolKind::Property) {
                    attributes.push(Attribute {
                        name: child.name().to_string(),
                        type_annotation: None,
                        visibility: Visibility::Public,
                    });
                }
            }
        }

        attributes
    }

    /// Infer all CodeElements within a module scope
    ///
    /// `scope` is a module path prefix (e.g. "src/domain")
    /// `max_depth` limits dependency traversal depth
    pub fn infer_scope(
        &self,
        scope: &str,
        call_graph: &CallGraph,
        max_depth: usize,
    ) -> Vec<CodeElement> {
        let _ = max_depth; // Used for relationship traversal, not element collection

        let mut elements = Vec::new();

        for (id, symbol) in call_graph.symbol_ids() {
            let file = symbol.location().file();

            // Filter by scope (match against file path or FQN)
            if !file.contains(scope) && !symbol.fully_qualified_name().contains(scope) {
                continue;
            }

            // Only include type definitions at L4
            if Self::is_type_like(*symbol.kind()) {
                elements.push(self.infer_from_symbol(id.as_str(), symbol, call_graph));
            }
        }

        elements
    }
}

impl Default for CodeInference {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_symbol_kind() {
        assert_eq!(CodeInference::map_symbol_kind(SymbolKind::Class), CodeElementKind::Class);
        assert_eq!(CodeInference::map_symbol_kind(SymbolKind::Struct), CodeElementKind::Struct);
        assert_eq!(CodeInference::map_symbol_kind(SymbolKind::Enum), CodeElementKind::Enum);
        assert_eq!(CodeInference::map_symbol_kind(SymbolKind::Trait), CodeElementKind::Interface);
        assert_eq!(CodeInference::map_symbol_kind(SymbolKind::Function), CodeElementKind::Function);
        assert_eq!(CodeInference::map_symbol_kind(SymbolKind::Method), CodeElementKind::Method);
        assert_eq!(CodeInference::map_symbol_kind(SymbolKind::Constructor), CodeElementKind::Constructor);
        assert_eq!(CodeInference::map_symbol_kind(SymbolKind::Field), CodeElementKind::Field);
        assert_eq!(CodeInference::map_symbol_kind(SymbolKind::Constant), CodeElementKind::Constant);
    }

    #[test]
    fn test_map_visibility() {
        assert_eq!(CodeInference::map_visibility(Some("public")), Visibility::Public);
        assert_eq!(CodeInference::map_visibility(Some("private")), Visibility::Private);
        assert_eq!(CodeInference::map_visibility(Some("protected")), Visibility::Protected);
        assert_eq!(CodeInference::map_visibility(None), Visibility::Public);
    }

    #[test]
    fn test_is_type_like() {
        assert!(CodeInference::is_type_like(SymbolKind::Class));
        assert!(CodeInference::is_type_like(SymbolKind::Struct));
        assert!(CodeInference::is_type_like(SymbolKind::Enum));
        assert!(CodeInference::is_type_like(SymbolKind::Trait));
        assert!(!CodeInference::is_type_like(SymbolKind::Function));
        assert!(!CodeInference::is_type_like(SymbolKind::Method));
    }
}
