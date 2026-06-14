//! Lens trait, `LensContext`, and `LensRegistry`.
//!
//! A **lens** is a named, typed query over an inspectable object that
//! composes data from the existing ports into `DesignFinding` objects.
//! Lenses are **hypotheses, not verdicts** — they surface observations
//! the human reader can interpret, never declarative claims.
//!
//! Lenses are NOT new ports. They consume the existing
//! [`crate::ports::SymbolRepository`], optional
//! [`crate::ports::QualityRepository`], and
//! [`crate::ports::SourceReader`]. Adding a new lens is OCP-compliant:
//! implement the trait, register an instance, no service-dispatch changes.

use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::object_identity::ObjectIdentity;
use crate::dto::{DesignFinding, InspectableObjectType, LensDescriptor, LensResult};
use crate::error::ExplorerResult;
use crate::ports::quality_repository::QualityRepository;
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::SymbolRepository;
use cognicode_core::domain::traits::GraphQueryPort;

/// Bundles every port a lens is allowed to touch, plus the resolved
/// object the lens is being applied to.
///
/// Lenses accept a `LensContext` rather than a service reference so they
/// stay pure and trivial to test in isolation — every test can build a
/// `LensContext` from mocks and exercise the lens directly.
pub struct LensContext {
    pub object_id: ObjectIdentity,
    pub symbol_repo: Arc<dyn SymbolRepository>,
    /// Optional. `None` means "no quality backend wired" — lenses MUST
    /// degrade gracefully in that case (lower confidence, no quality-based
    /// findings, but no errors).
    pub quality_repo: Option<Arc<dyn QualityRepository>>,
    pub source_reader: Arc<dyn SourceReader>,
    /// Optional graph query port for traversal and navigation queries.
    /// `None` when no call graph is wired.
    pub graph_query: Option<Arc<dyn GraphQueryPort>>,
}

impl LensContext {
    /// Construct a context. Convenience constructor for tests and
    /// production call sites.
    pub fn new(
        object_id: ObjectIdentity,
        symbol_repo: Arc<dyn SymbolRepository>,
        quality_repo: Option<Arc<dyn QualityRepository>>,
        source_reader: Arc<dyn SourceReader>,
        graph_query: Option<Arc<dyn GraphQueryPort>>,
    ) -> Self {
        Self {
            object_id,
            symbol_repo,
            quality_repo,
            source_reader,
            graph_query,
        }
    }
}

/// A named, composable, hypothesis-producing query.
///
/// Implementors declare which `InspectableObjectType`s they apply to via
/// [`Self::descriptor`]; the service uses that to filter
/// `available_lenses`. The `apply` method receives a [`LensContext`] and
/// returns a [`LensResult`] — pure transformation of the data the
/// context exposes, no I/O.
pub trait Lens: Send + Sync {
    /// Stable, snake_case identifier. The registry keys on this value.
    fn id(&self) -> &str;

    /// Human-readable metadata. Surfaced verbatim through the API.
    fn descriptor(&self) -> LensDescriptor;

    /// `true` when the lens is meaningful for the given object type.
    /// The default delegates to `descriptor().applicable_types.contains(&t)`.
    fn applies_to(&self, object_type: &InspectableObjectType) -> bool {
        self.descriptor().applicable_types.contains(object_type)
    }

    /// Run the lens against `ctx`. Must not fail when the quality
    /// backend is absent — degrade confidence and skip quality-based
    /// findings instead.
    fn apply(&self, ctx: &LensContext) -> ExplorerResult<LensResult>;
}

/// `HashMap` of lens id → lens instance, plus an insertion-order vec of
/// descriptors so `list()` returns a stable, predictable order.
#[derive(Clone)]
pub struct LensRegistry {
    lenses: HashMap<String, Arc<dyn Lens>>,
    /// Preserves registration order so `list()` and `applicable_to()`
    /// return a deterministic view (important for tests and for the API
    /// surface — callers see lenses in the order they were registered).
    order: Vec<LensDescriptor>,
}

impl Default for LensRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LensRegistry {
    /// Empty registry. Production service constructors build a registry
    /// with the default lens set; tests can build an empty one and
    /// register only the lenses they care about.
    pub fn new() -> Self {
        Self {
            lenses: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Register a lens. Re-registering with the same id replaces the
    /// previous instance AND the previous descriptor — useful for tests
    /// that want to swap in a mock.
    pub fn register(&mut self, lens: Arc<dyn Lens>) {
        let id = lens.id().to_string();
        if !self.lenses.contains_key(&id) {
            self.order.push(lens.descriptor());
        } else {
            // Replace the existing descriptor in place to keep
            // registration order stable across re-registrations.
            if let Some(slot) = self.order.iter_mut().find(|d| d.id == id) {
                *slot = lens.descriptor();
            }
        }
        self.lenses.insert(id, lens);
    }

    /// Look up a lens by id. Returns `None` when the id is unknown.
    pub fn get(&self, id: &str) -> Option<&Arc<dyn Lens>> {
        self.lenses.get(id)
    }

    /// Every registered descriptor, in registration order.
    pub fn list(&self) -> Vec<LensDescriptor> {
        self.order.clone()
    }

    /// Subset of `list()` whose `applicable_types` contains
    /// `object_type`. Order is preserved.
    pub fn applicable_to(&self, object_type: &InspectableObjectType) -> Vec<LensDescriptor> {
        self.order
            .iter()
            .filter(|d| d.applicable_types.contains(object_type))
            .cloned()
            .collect()
    }
}

/// Build the default lens set used by the production service
/// constructors. The three lenses live in
/// `crate::domain::lenses::{hotspots, dependencies, architecture}`.
pub fn default_registry() -> LensRegistry {
    let mut registry = LensRegistry::new();
    registry.register(Arc::new(crate::domain::lenses::hotspots::HotspotsLens));
    registry.register(Arc::new(
        crate::domain::lenses::dependencies::DependenciesLens,
    ));
    registry.register(Arc::new(
        crate::domain::lenses::architecture::ArchitectureLens,
    ));
    registry
}

// ============================================================================
// Helpers shared by the lens implementations
// ============================================================================

/// Stable id for a `DesignFinding` produced by a lens. Combines the
/// lens id with a domain-specific key so the same finding is
/// reproducible across runs (helpful for tests + diffs).
pub fn finding_id(lens_id: &str, key: &str) -> String {
    format!("finding:{lens_id}:{key}")
}

/// Clamp a confidence score to `[0.0, 1.0]`. Lenses compute it from
/// data availability; this helper guards against accidental
/// out-of-range values.
pub fn clamp_confidence(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else if value < 0.0 {
        0.0
    } else if value > 1.0 {
        1.0
    } else {
        value
    }
}

/// Order `findings` Critical → Warning → Info, breaking ties by
/// confidence descending, and truncate to `cap` entries. The default
/// cap is 20 per the spec.
pub fn cap_and_order(findings: Vec<DesignFinding>, cap: usize) -> Vec<DesignFinding> {
    let mut sorted = findings;
    sorted.sort_by(|a, b| {
        a.severity
            .rank()
            .cmp(&b.severity.rank())
            .then_with(|| {
                // Higher confidence first → reverse partial_cmp.
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.id.cmp(&b.id))
    });
    sorted.truncate(cap);
    sorted
}

/// Numeric weight for a quality-severity string. `Blocker=3`,
/// `Critical=2`, `Major=1.5`, `Minor=1`, `Info=0.5`, anything else `1.0`.
/// Lenses multiply issue counts by this when computing risk.
pub fn severity_weight(severity: &str) -> f32 {
    match severity.to_ascii_lowercase().as_str() {
        "blocker" => 3.0,
        "critical" => 2.0,
        "major" => 1.5,
        "minor" => 1.0,
        "info" => 0.5,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{FindingSeverity, InspectableObjectType};

    fn desc(id: &str, types: Vec<InspectableObjectType>) -> LensDescriptor {
        LensDescriptor {
            id: id.into(),
            name: id.into(),
            description: format!("test lens {id}"),
            applicable_types: types,
        }
    }

    struct TestLens {
        id: String,
        types: Vec<InspectableObjectType>,
    }
    impl Lens for TestLens {
        fn id(&self) -> &str {
            &self.id
        }
        fn descriptor(&self) -> LensDescriptor {
            desc(&self.id, self.types.clone())
        }
        fn apply(&self, _ctx: &LensContext) -> ExplorerResult<LensResult> {
            Ok(LensResult {
                lens_id: self.id.clone(),
                findings: Vec::new(),
                summary: String::new(),
            })
        }
    }

    #[test]
    fn register_and_get() {
        let mut r = LensRegistry::new();
        r.register(Arc::new(TestLens {
            id: "a".into(),
            types: vec![InspectableObjectType::Symbol],
        }));
        assert!(r.get("a").is_some());
        assert!(r.get("missing").is_none());
    }

    #[test]
    fn list_returns_in_registration_order() {
        let mut r = LensRegistry::new();
        r.register(Arc::new(TestLens {
            id: "a".into(),
            types: vec![InspectableObjectType::Symbol],
        }));
        r.register(Arc::new(TestLens {
            id: "b".into(),
            types: vec![InspectableObjectType::File],
        }));
        let ids: Vec<String> = r.list().into_iter().map(|d| d.id).collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn applicable_to_filters() {
        let mut r = LensRegistry::new();
        r.register(Arc::new(TestLens {
            id: "sym_only".into(),
            types: vec![InspectableObjectType::Symbol],
        }));
        r.register(Arc::new(TestLens {
            id: "file_only".into(),
            types: vec![InspectableObjectType::File],
        }));
        r.register(Arc::new(TestLens {
            id: "both".into(),
            types: vec![InspectableObjectType::Symbol, InspectableObjectType::File],
        }));
        let sym: Vec<String> = r
            .applicable_to(&InspectableObjectType::Symbol)
            .into_iter()
            .map(|d| d.id)
            .collect();
        assert_eq!(sym, vec!["sym_only", "both"]);
        let file: Vec<String> = r
            .applicable_to(&InspectableObjectType::File)
            .into_iter()
            .map(|d| d.id)
            .collect();
        assert_eq!(file, vec!["file_only", "both"]);
    }

    #[test]
    fn re_register_replaces_in_place() {
        let mut r = LensRegistry::new();
        r.register(Arc::new(TestLens {
            id: "a".into(),
            types: vec![InspectableObjectType::Symbol],
        }));
        r.register(Arc::new(TestLens {
            id: "b".into(),
            types: vec![InspectableObjectType::File],
        }));
        r.register(Arc::new(TestLens {
            id: "a".into(),
            types: vec![InspectableObjectType::Scope],
        }));
        // "a" should now be applicable to Scope but not Symbol; order preserved.
        assert!(
            !r.applicable_to(&InspectableObjectType::Symbol)
                .iter()
                .any(|d| d.id == "a")
        );
        assert!(
            r.applicable_to(&InspectableObjectType::Scope)
                .iter()
                .any(|d| d.id == "a")
        );
        let ids: Vec<String> = r.list().into_iter().map(|d| d.id).collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn default_registry_has_three_lenses() {
        let r = default_registry();
        assert_eq!(r.list().len(), 3);
        let ids: Vec<String> = r.list().into_iter().map(|d| d.id).collect();
        assert!(ids.contains(&"hotspots".to_string()));
        assert!(ids.contains(&"dependencies".to_string()));
        assert!(ids.contains(&"architecture".to_string()));
    }

    #[test]
    fn finding_id_is_stable() {
        assert_eq!(
            finding_id("hotspots", "src/foo.rs:alpha:1"),
            "finding:hotspots:src/foo.rs:alpha:1"
        );
    }

    #[test]
    fn clamp_confidence_bounds_values() {
        assert_eq!(clamp_confidence(0.5), 0.5);
        assert_eq!(clamp_confidence(-0.1), 0.0);
        assert_eq!(clamp_confidence(1.5), 1.0);
        assert_eq!(clamp_confidence(f32::NAN), 0.0);
    }

    #[test]
    fn cap_and_order_sorts_critical_first() {
        let mk = |id: &str, sev: FindingSeverity, conf: f32| DesignFinding {
            id: id.into(),
            lens_id: "t".into(),
            title: "t".into(),
            hypothesis: "h".into(),
            severity: sev,
            confidence: conf,
            object_ids: Vec::new(),
            evidence_ids: Vec::new(),
        };
        let f = vec![
            mk("a-info", FindingSeverity::Info, 0.9),
            mk("b-crit", FindingSeverity::Critical, 0.5),
            mk("c-warn", FindingSeverity::Warning, 0.7),
            mk("d-crit-high", FindingSeverity::Critical, 0.95),
        ];
        let ordered = cap_and_order(f, 20);
        let ids: Vec<&str> = ordered.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, vec!["d-crit-high", "b-crit", "c-warn", "a-info"]);
    }

    #[test]
    fn cap_and_order_truncates() {
        let mk = |i: usize| DesignFinding {
            id: format!("f{i}"),
            lens_id: "t".into(),
            title: "t".into(),
            hypothesis: "h".into(),
            severity: FindingSeverity::Info,
            confidence: 0.5,
            object_ids: Vec::new(),
            evidence_ids: Vec::new(),
        };
        let f: Vec<DesignFinding> = (0..30).map(mk).collect();
        let out = cap_and_order(f, 20);
        assert_eq!(out.len(), 20);
    }

    #[test]
    fn severity_weight_known_values() {
        assert_eq!(severity_weight("Blocker"), 3.0);
        assert_eq!(severity_weight("critical"), 2.0);
        assert_eq!(severity_weight("Major"), 1.5);
        assert_eq!(severity_weight("minor"), 1.0);
        assert_eq!(severity_weight("Info"), 0.5);
        assert_eq!(severity_weight("Unknown"), 1.0);
    }
}
