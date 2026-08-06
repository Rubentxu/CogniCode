//! MoldQL scaffold registry for the Moldable View Runtime.
//!
//! Scaffolds are declarative view recipes loaded from `moldql-scaffolds.yaml`
//! at startup. Each scaffold maps an [`InspectableObjectType`] to a MoldQL query
//! template and a recommended [`ViewKind`]/[`RendererKind`] pair, enabling the
//! Explorer to offer one-click contextual views for any inspected object.

use std::fmt;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::dto::{InspectableObjectType, RendererKind, ViewKind};

/// Maximum length for `intent`, `label`, and `description` fields.
/// Scaffolds exceeding this limit are logged and rejected during validation.
const MAX_TEXT_LEN: usize = 512;

/// Maximum length for `query_template` strings.
const MAX_QUERY_LEN: usize = 4096;

// ============================================================================
// Scaffold data structure
// ============================================================================

/// A single scaffold definition parsed from `moldql-scaffolds.yaml`.
#[derive(Debug, Clone, Deserialize)]
pub struct Scaffold {
    /// Unique identifier, e.g. `"callers_and_callees"`.
    pub id: String,

    /// Which inspectable object type this scaffold applies to.
    #[serde(rename = "object_type")]
    pub object_type: String,

    /// One-line semantic intent (imperative mood).
    pub intent: String,

    /// Short display label for UI pickers.
    pub label: String,

    /// Longer explanation shown in hover / help text.
    pub description: String,

    /// MoldQL query string with `{{object_id}}` placeholder.
    #[serde(rename = "query_template")]
    pub query_template: String,

    /// Recommended [`ViewKind`] variant name (snake_case string).
    #[serde(rename = "view_kind")]
    pub view_kind: String,

    /// Recommended [`RendererKind`] variant name (snake_case string).
    #[serde(rename = "renderer_kind")]
    pub renderer_kind: String,

    /// Conditional eligibility predicate (always `null` in Phase 1).
    #[serde(rename = "applies_when")]
    pub applies_when: Option<String>,

    /// Whether this scaffold produces relation-candidate edges (always `false` in Phase 1).
    #[serde(rename = "produces_relation_candidates")]
    pub produces_relation_candidates: bool,
}

impl Scaffold {
    /// Parse the `object_type` string into an [`InspectableObjectType`].
    ///
    /// Returns `None` if the string does not match any known variant.
    pub fn parse_object_type(s: &str) -> Option<InspectableObjectType> {
        match s {
            "workspace" => Some(InspectableObjectType::Workspace),
            "scope" => Some(InspectableObjectType::Scope),
            "symbol" => Some(InspectableObjectType::Symbol),
            "file" => Some(InspectableObjectType::File),
            "module" => Some(InspectableObjectType::Module),
            "evidence" => Some(InspectableObjectType::Evidence),
            "decision_artifact" => Some(InspectableObjectType::DecisionArtifact),
            "quality_issue" => Some(InspectableObjectType::QualityIssue),
            "rule" => Some(InspectableObjectType::Rule),
            "saved_exploration" => Some(InspectableObjectType::SavedExploration),
            "investigation" => Some(InspectableObjectType::Investigation),
            _ => None,
        }
    }

    /// Parse the `view_kind` string into a [`ViewKind`].
    ///
    /// Unknown strings are returned as `ViewKind::Custom(s)`.
    pub fn parse_view_kind(s: &str) -> ViewKind {
        // ViewKind implements Deserialize via its Serialize/Deserialize impl
        serde_yaml::from_str(&format!("---\n{}", s))
            .unwrap_or_else(|_| ViewKind::Custom(s.to_string()))
    }

    /// Parse the `renderer_kind` string into a [`RendererKind`].
    ///
    /// Unknown strings are returned as `RendererKind::Custom(s)`.
    pub fn parse_renderer_kind(s: &str) -> RendererKind {
        serde_yaml::from_str(&format!("---\n{}", s))
            .unwrap_or_else(|_| RendererKind::Custom(s.to_string()))
    }

    /// Validate this scaffold's text fields and structural invariants.
    ///
    /// Returns `Ok(())` if the scaffold is well-formed; otherwise returns a
    /// descriptive error message.
    pub fn validate(&self) -> Result<(), ScaffoldValidationError> {
        if self.id.is_empty() {
            return Err(ScaffoldValidationError::EmptyField("id"));
        }
        if Self::parse_object_type(&self.object_type).is_none() {
            return Err(ScaffoldValidationError::UnknownObjectType(
                self.object_type.clone(),
            ));
        }
        if self.intent.len() > MAX_TEXT_LEN {
            return Err(ScaffoldValidationError::FieldTooLong {
                field: "intent",
                len: self.intent.len(),
                max: MAX_TEXT_LEN,
            });
        }
        if self.label.is_empty() {
            return Err(ScaffoldValidationError::EmptyField("label"));
        }
        if self.label.len() > MAX_TEXT_LEN {
            return Err(ScaffoldValidationError::FieldTooLong {
                field: "label",
                len: self.label.len(),
                max: MAX_TEXT_LEN,
            });
        }
        if self.description.len() > MAX_TEXT_LEN {
            return Err(ScaffoldValidationError::FieldTooLong {
                field: "description",
                len: self.description.len(),
                max: MAX_TEXT_LEN,
            });
        }
        if self.query_template.is_empty() {
            return Err(ScaffoldValidationError::EmptyField("query_template"));
        }
        if self.query_template.len() > MAX_QUERY_LEN {
            return Err(ScaffoldValidationError::FieldTooLong {
                field: "query_template",
                len: self.query_template.len(),
                max: MAX_QUERY_LEN,
            });
        }
        // view_kind and renderer_kind are lenient — unknown variants become Custom(_)
        Ok(())
    }
}

/// Error returned when a scaffold fails validation.
#[derive(Debug, Clone)]
pub enum ScaffoldValidationError {
    EmptyField(&'static str),
    UnknownObjectType(String),
    FieldTooLong {
        field: &'static str,
        len: usize,
        max: usize,
    },
}

impl fmt::Display for ScaffoldValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScaffoldValidationError::EmptyField(field) => {
                write!(f, "scaffold has empty `{field}` field")
            }
            ScaffoldValidationError::UnknownObjectType(ot) => {
                write!(f, "scaffold has unknown `object_type`: `{ot}`")
            }
            ScaffoldValidationError::FieldTooLong { field, len, max } => {
                write!(f, "scaffold `{field}` length {len} exceeds maximum {max}")
            }
        }
    }
}

impl std::error::Error for ScaffoldValidationError {}

// ============================================================================
// YAML parsing helpers
// ============================================================================

/// Wrapper struct that matches the top-level `scaffolds:` sequence in the YAML.
#[derive(Debug, Deserialize)]
struct ScaffoldsYaml {
    scaffolds: Vec<Scaffold>,
}

// ============================================================================
// ScaffoldRegistry
// ============================================================================

/// Global registry of all loaded scaffolds.
///
/// Uses a `OnceLock` so the YAML is parsed exactly once — on the first call
/// to any accessor — and is then shared across all subsequent calls.
pub struct ScaffoldRegistry {
    scaffolds: Vec<Scaffold>,
    /// Scaffolds indexed by their `object_type` string for fast lookup.
    by_object_type: std::collections::HashMap<String, Vec<usize>>,
    /// Scaffolds indexed by their `id` for fast lookup.
    by_id: std::collections::HashMap<String, usize>,
}

impl ScaffoldRegistry {
    /// Load and validate all scaffolds from the embedded `moldql-scaffolds.yaml`.
    ///
    /// This function is only called once (via `OnceLock::get_or_init`) even
    /// when called multiple times.
    fn load() -> Result<Self, ScaffoldLoadError> {
        let yaml_content = include_str!("../assets/moldql-scaffolds.yaml");
        let parsed: ScaffoldsYaml = serde_yaml::from_str(yaml_content)
            .map_err(|e| ScaffoldLoadError::Parse(e.to_string()))?;

        if parsed.scaffolds.is_empty() {
            return Err(ScaffoldLoadError::EmptyScaffolds);
        }

        let mut scaffolds = Vec::with_capacity(parsed.scaffolds.len());
        let mut by_object_type: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        let mut by_id: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        for (idx, scaffold) in parsed.scaffolds.into_iter().enumerate() {
            if let Err(e) = scaffold.validate() {
                return Err(ScaffoldLoadError::Validation {
                    id: scaffold.id.clone(),
                    error: e.to_string(),
                });
            }

            // Check for duplicate ids
            if by_id.contains_key(&scaffold.id) {
                return Err(ScaffoldLoadError::DuplicateId(scaffold.id));
            }

            by_object_type
                .entry(scaffold.object_type.clone())
                .or_default()
                .push(idx);

            by_id.insert(scaffold.id.clone(), idx);
            scaffolds.push(scaffold);
        }

        Ok(Self {
            scaffolds,
            by_object_type,
            by_id,
        })
    }

    /// Returns all scaffolds.
    pub fn all(&self) -> &[Scaffold] {
        &self.scaffolds
    }

    /// Returns all scaffolds matching the given `object_type` string.
    ///
    /// The lookup is O(1) via the index map.
    pub fn get_for_object_type(&self, object_type: &str) -> Vec<&Scaffold> {
        self.by_object_type
            .get(object_type)
            .map(|indices| indices.iter().map(|&i| &self.scaffolds[i]).collect())
            .unwrap_or_default()
    }

    /// Returns the scaffold with the given `id`, if one exists.
    ///
    /// The lookup is O(1) via the id index map.
    pub fn get_by_id(&self, id: &str) -> Option<&Scaffold> {
        self.by_id.get(id).map(|&idx| &self.scaffolds[idx])
    }
}

/// Error returned when the registry fails to load.
#[derive(Debug, Clone)]
pub enum ScaffoldLoadError {
    Parse(String),
    EmptyScaffolds,
    Validation { id: String, error: String },
    DuplicateId(String),
}

impl fmt::Display for ScaffoldLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScaffoldLoadError::Parse(msg) => write!(f, "failed to parse YAML: {msg}"),
            ScaffoldLoadError::EmptyScaffolds => {
                write!(f, "moldql-scaffolds.yaml contains no scaffolds")
            }
            ScaffoldLoadError::Validation { id, error } => {
                write!(f, "scaffold `{id}` failed validation: {error}")
            }
            ScaffoldLoadError::DuplicateId(id) => {
                write!(f, "duplicate scaffold id: `{id}`")
            }
        }
    }
}

impl std::error::Error for ScaffoldLoadError {}

/// Global singleton — loaded once on first access.
static REGISTRY: OnceLock<Result<ScaffoldRegistry, ScaffoldLoadError>> = OnceLock::new();

/// Access the global scaffold registry, loading it on first call.
///
/// Returns a reference to the registry, or a reference to the error that
/// occurred during loading.
pub fn registry() -> Result<&'static ScaffoldRegistry, &'static ScaffoldLoadError> {
    REGISTRY.get_or_init(ScaffoldRegistry::load).as_ref()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid scaffold YAML used as a test fixture.
    const FIXTURE_YAML: &str = r#"
scaffolds:
  - id: test-scaffold
    object_type: symbol
    intent: "Find callers of this symbol"
    label: "Test Scaffold"
    description: "A test scaffold for unit testing"
    query_template: "calls from '{{object_id}}' depth 1"
    view_kind: call_graph
    renderer_kind: graph
    applies_when: null
    produces_relation_candidates: false
"#;

    #[test]
    fn scaffold_validate_ok() {
        let scaffold = Scaffold {
            id: "callers_and_callees".into(),
            object_type: "symbol".into(),
            intent: "Show callers".into(),
            label: "Callers".into(),
            description: "Shows callers".into(),
            query_template: "calls from '{{object_id}}'".into(),
            view_kind: "call_graph".into(),
            renderer_kind: "graph".into(),
            applies_when: None,
            produces_relation_candidates: false,
        };
        assert!(scaffold.validate().is_ok());
    }

    #[test]
    fn scaffold_validate_empty_id() {
        let scaffold = Scaffold {
            id: "".into(),
            object_type: "symbol".into(),
            intent: "Show callers".into(),
            label: "Callers".into(),
            description: "Shows callers".into(),
            query_template: "calls from '{{object_id}}'".into(),
            view_kind: "call_graph".into(),
            renderer_kind: "graph".into(),
            applies_when: None,
            produces_relation_candidates: false,
        };
        assert!(matches!(
            scaffold.validate(),
            Err(ScaffoldValidationError::EmptyField("id"))
        ));
    }

    #[test]
    fn scaffold_validate_unknown_object_type() {
        let scaffold = Scaffold {
            id: "test".into(),
            object_type: "not_a_type".into(),
            intent: "Show callers".into(),
            label: "Callers".into(),
            description: "Shows callers".into(),
            query_template: "calls from '{{object_id}}'".into(),
            view_kind: "call_graph".into(),
            renderer_kind: "graph".into(),
            applies_when: None,
            produces_relation_candidates: false,
        };
        assert!(matches!(
            scaffold.validate(),
            Err(ScaffoldValidationError::UnknownObjectType(ot)) if ot == "not_a_type"
        ));
    }

    #[test]
    fn scaffold_validate_intent_too_long() {
        let scaffold = Scaffold {
            id: "test".into(),
            object_type: "symbol".into(),
            intent: "x".repeat(600),
            label: "Callers".into(),
            description: "Shows callers".into(),
            query_template: "calls from '{{object_id}}'".into(),
            view_kind: "call_graph".into(),
            renderer_kind: "graph".into(),
            applies_when: None,
            produces_relation_candidates: false,
        };
        assert!(matches!(
            scaffold.validate(),
            Err(ScaffoldValidationError::FieldTooLong {
                field: "intent",
                ..
            })
        ));
    }

    #[test]
    fn scaffold_parse_object_type() {
        assert_eq!(
            Scaffold::parse_object_type("symbol"),
            Some(InspectableObjectType::Symbol)
        );
        assert_eq!(
            Scaffold::parse_object_type("file"),
            Some(InspectableObjectType::File)
        );
        assert_eq!(
            Scaffold::parse_object_type("scope"),
            Some(InspectableObjectType::Scope)
        );
        assert_eq!(
            Scaffold::parse_object_type("investigation"),
            Some(InspectableObjectType::Investigation)
        );
        assert_eq!(Scaffold::parse_object_type("unknown"), None);
    }

    #[test]
    fn scaffold_parse_view_kind() {
        assert_eq!(Scaffold::parse_view_kind("call_graph"), ViewKind::CallGraph);
        assert_eq!(
            Scaffold::parse_view_kind("vertical_slice"),
            ViewKind::VerticalSlice
        );
        assert_eq!(
            Scaffold::parse_view_kind("unknown_kind"),
            ViewKind::Custom("unknown_kind".to_string())
        );
    }

    #[test]
    fn scaffold_parse_renderer_kind() {
        assert_eq!(Scaffold::parse_renderer_kind("graph"), RendererKind::Graph);
        assert_eq!(Scaffold::parse_renderer_kind("table"), RendererKind::Table);
        assert_eq!(
            Scaffold::parse_renderer_kind("unknown_renderer"),
            RendererKind::Custom("unknown_renderer".to_string())
        );
    }

    #[test]
    fn parse_fixture_yaml() {
        let parsed: ScaffoldsYaml =
            serde_yaml::from_str(FIXTURE_YAML).expect("fixture should parse");
        assert_eq!(parsed.scaffolds.len(), 1);
        let s = &parsed.scaffolds[0];
        assert_eq!(s.id, "test-scaffold");
        assert_eq!(s.object_type, "symbol");
        assert_eq!(s.intent, "Find callers of this symbol");
        assert!(s.validate().is_ok());
    }

    #[test]
    fn registry_loads_all_scaffolds() {
        let reg = registry().expect("registry should load");
        // The real YAML has 9 scaffolds across 4 object types
        assert!(!reg.all().is_empty());

        // Verify we can look up by object type
        let symbol_scaffolds = reg.get_for_object_type("symbol");
        assert!(!symbol_scaffolds.is_empty());
        assert!(symbol_scaffolds.iter().all(|s| s.object_type == "symbol"));

        // Verify we can look up by id
        let callers = reg.get_by_id("callers_and_callees");
        assert!(callers.is_some());
        assert_eq!(callers.unwrap().id, "callers_and_callees");

        // Unknown id returns None
        assert!(reg.get_by_id("nonexistent").is_none());
        // Unknown object type returns empty vec
        assert!(reg.get_for_object_type("not_a_type").is_empty());
    }

    #[test]
    fn registry_unknown_object_type_returns_empty() {
        let reg = registry().expect("registry should load");
        let result = reg.get_for_object_type("this_type_does_not_exist");
        assert!(result.is_empty());
    }
}
