//! ViewRegistry — backend discovery surface for built-in and runtime views.
//!
//! # Architecture
//!
//! The registry follows the same trait-object pattern as [`LensRegistry`].
//! Built-in views register through `inventory::submit!` which provides
//! compile-time collection on stable Rust (linkme/distributed-slice
//! deferred to v1.1 per the design).
//!
//! # Phase 1 Scope
//!
//! - `ViewDescriptorProvider` trait — metadata only (no `ContextualView` payload)
//! - Static registration of the 4 built-in providers
//! - `ViewRegistry::list_for` / `get` / `known_view_kinds`
//! - `spec_store: None` path is a no-op; Phase 2+ wires the store handle
//!
//! # Phase 2 Scope
//!
//! - `ViewSpecStore` trait with full CRUD methods
//! - `PostgresViewSpecStore` implementation backed by the `view_specs` table
//! - `ViewRegistry` wires the store to serve runtime view specs
//!
//! # Out of Scope
//!
//! - Runtime view execution — Phase 4
//! - `linkme` registration — v1.1

use std::sync::OnceLock;

use async_trait::async_trait;

use crate::dto::{InspectableObjectType, RendererKind, ViewDescriptorDto, ViewKind, ViewSpec};

/// Error returned by [`ViewSpecStore`] operations.
#[derive(Debug, Clone)]
pub enum ViewSpecStoreError {
    /// The operation failed due to a storage error.
    Store(String),
    /// A row with the same `(workspace_id, owner, title)` already exists.
    Conflict(String),
    /// The requested view spec was not found.
    NotFound(String),
}

impl std::fmt::Display for ViewSpecStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Store(msg) => write!(f, "view_spec store error: {msg}"),
            Self::Conflict(msg) => write!(f, "view_spec conflict: {msg}"),
            Self::NotFound(msg) => write!(f, "view_spec not found: {msg}"),
        }
    }
}

impl std::error::Error for ViewSpecStoreError {}

/// Phase 2+ view spec store trait.
///
/// Abstracts the persistence layer for [`ViewSpec`] objects.
/// Implementations must be `Send + Sync` and `Arc`-friendly.
#[async_trait]
pub trait ViewSpecStore: Send + Sync + 'static {
    /// Persist a view spec. The `id` is client-provided; the store
    /// must return [`ViewSpecStoreError::Conflict`] when a row with the
    /// same `(workspace_id, owner, title)` already exists (idempotent
    /// save is the caller's responsibility).
    async fn save(
        &self,
        spec: &ViewSpec,
        workspace_id: &str,
        owner: &str,
    ) -> Result<(), ViewSpecStoreError>;

    /// Load a single view spec by id, scoped to `(workspace_id, owner)`.
    /// Returns `Ok(None)` when no matching row exists.
    async fn load(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Option<ViewSpec>, ViewSpecStoreError>;

    /// List every view spec for `(workspace_id, owner)`, ordered by
    /// `created_at DESC` (newest first). Returns `Ok(vec![])` for an
    /// empty scope — NOT an error.
    async fn list(
        &self,
        workspace_id: &str,
        owner: &str,
    ) -> Result<Vec<ViewSpec>, ViewSpecStoreError>;

    /// Delete a view spec by id, scoped to `(workspace_id, owner)`.
    /// Returns `Ok(true)` if a row was deleted, `Ok(false)` if no
    /// matching row existed.
    async fn delete(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Result<bool, ViewSpecStoreError>;

    /// List every view spec for `workspace_id` with the given `applies_to`,
    /// across ALL owners. Used by the "all owners visible" Spotter model.
    /// Returns `Ok(vec![])` for an empty scope — NOT an error.
    async fn list_for_workspace(
        &self,
        workspace_id: &str,
        applies_to: InspectableObjectType,
    ) -> Result<Vec<ViewSpec>, ViewSpecStoreError>;

    /// Update a view spec's provenance fields (seed_object_id, seed_view_id,
    /// applies_when) in-place without touching other columns.
    /// Returns `Ok(true)` if a row was updated, `Ok(false)` if no matching
    /// row existed.
    async fn update(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
        seed_object_id: Option<&str>,
        seed_view_id: Option<&str>,
        applies_when: Option<&str>,
    ) -> Result<bool, ViewSpecStoreError>;
}

/// A provider of one view's metadata.
///
/// The trait carries descriptor metadata only — the registry does NOT build
/// the [`ContextualView`][crate::dto::ContextualView] payload. Existing
/// service-layer dispatch (`build_overview`, etc.) is unchanged; the registry
/// is an *additional* discovery surface for `available_views`.
pub trait ViewDescriptorProvider: Send + Sync {
    /// Stable id, e.g. `"overview"`, `"call-graph"`.
    fn id(&self) -> &'static str;

    /// Human-readable title, e.g. `"Overview"`, `"Call Graph"`.
    fn title(&self) -> &'static str;

    /// Object kinds the view applies to.
    fn applies_to(&self) -> &'static [InspectableObjectType];

    /// Semantic view intent. For built-ins this is a well-known constant;
    /// for future runtime providers it may be `ViewKind::Custom(_)`.
    fn view_kind(&self) -> ViewKind;

    /// Whether this provider is shipped compiled-in (`true`) or user-defined
    /// (`false`). Phase 1 always returns `true`.
    fn is_builtin(&self) -> bool {
        true
    }

    /// Default renderer for this view. Phase 1 uses a service-layer mapping;
    /// providers may override this in future phases.
    fn renderer_kind(&self) -> RendererKind {
        // Default to Json; the service layer maps known view ids to renderers.
        RendererKind::Json
    }
}

// ============================================================================
// Registration system (inventory for stable Rust, linkme deferred to v1.1)
// ============================================================================

/// Wrapper type that holds a `&'static dyn ViewDescriptorProvider` and
/// implements `inventory::Collect`. This allows us to use `inventory`
/// to register trait objects by wrapping them in a concrete type.
pub struct ProviderWrapper {
    pub provider: &'static dyn ViewDescriptorProvider,
}

// inventory::collect expects impl Collect, and Collect is implemented for
// types that have a fn() returning &'static O (a factory).
inventory::collect!(ProviderWrapper);

/// Returns all registered built-in providers, sorted alphabetically by id.
fn builtin_providers() -> &'static [&'static dyn ViewDescriptorProvider] {
    static SORTED: OnceLock<Vec<&'static dyn ViewDescriptorProvider>> = OnceLock::new();
    SORTED.get_or_init(|| {
        let mut v: Vec<&'static dyn ViewDescriptorProvider> = Vec::new();
        for wrapper in inventory::iter::<ProviderWrapper> {
            v.push(wrapper.provider);
        }
        v.sort_by_key(|p| p.id());
        v
    });
    SORTED.get().unwrap().as_slice()
}

/// `ViewDescriptorDto` extracted from a provider.
impl From<&dyn ViewDescriptorProvider> for ViewDescriptorDto {
    fn from(provider: &dyn ViewDescriptorProvider) -> Self {
        Self {
            id: provider.id().to_string(),
            title: provider.title().to_string(),
            view_kind: provider.view_kind(),
            is_builtin: true,
            source: None,
        }
    }
}

/// Adapter that presents a `ViewDescriptorProvider` as a `ViewExecutor`.
/// Phase 1 uses this to expose existing registrations via the new `get_executor`
/// API without requiring separate registrations. Phase 2+ registers `ViewExecutor`
/// implementations directly.
struct ProviderExecutorAdapter {
    provider: &'static dyn ViewDescriptorProvider,
}

impl ProviderExecutorAdapter {
    fn new(provider: &'static dyn ViewDescriptorProvider) -> Self {
        Self { provider }
    }
}

impl crate::domain::views::ViewDescriptor for ProviderExecutorAdapter {
    fn id(&self) -> &'static str {
        self.provider.id()
    }
    fn title(&self) -> &'static str {
        self.provider.title()
    }
    fn applies_to(&self) -> &'static [crate::dto::InspectableObjectType] {
        self.provider.applies_to()
    }
    fn view_kind(&self) -> crate::dto::ViewKind {
        self.provider.view_kind()
    }
    fn renderer_kind(&self) -> crate::dto::RendererKind {
        self.provider.renderer_kind()
    }
}

#[async_trait::async_trait]
impl crate::domain::views::ViewExecutor for ProviderExecutorAdapter {
    async fn build(
        &self,
        _ctx: &crate::domain::views::ViewContext<'_>,
    ) -> crate::error::ExplorerResult<crate::dto::ContextualView> {
        // Phase 1: no-op. Real implementations come in PR 2+.
        Err(crate::error::ExplorerError::ViewNotAvailable {
            object_id: "provider-adapter".to_string(),
            view_id: self.provider.id().to_string(),
        })
    }
}

// ============================================================================
// REAL_EXECUTORS — single module-level OnceLock shared by list_for and get_executor
// ============================================================================

use std::sync::Arc;

/// Assert that no two executors share the same `ViewKind`.
///
/// Panics with a diagnostic message naming both executor ids and the
/// duplicate `ViewKind` variant when a conflict is detected.
///
/// This guard fires once at `real_executors()` initialization — it does NOT
/// run on every call, only at process startup when the `OnceLock` is first
/// populated.
fn assert_view_kind_uniqueness(pairs: &[(&'static str, crate::dto::ViewKind)]) {
    use std::collections::HashSet;
    // S4: Use a simple HashSet to detect duplicates without Vec allocation.
    // Track (kind, first_id) for the error message; duplicate ids are detected
    // by insertion failure rather than building a full Vec map.
    let mut seen: HashSet<&crate::dto::ViewKind> = HashSet::new();
    for (id, kind) in pairs {
        if !seen.insert(kind) {
            panic!(
                "duplicate ViewKind registration: kind={kind:?} already registered by id={id}",
            );
        }
    }
}

type ViewExecutorMap = std::collections::HashMap<
    &'static str,
    &'static dyn crate::domain::views::ViewExecutor,
>;

static REAL_EXECUTORS: OnceLock<ViewExecutorMap> = OnceLock::new();

fn real_executors() -> &'static ViewExecutorMap {
    REAL_EXECUTORS.get_or_init(|| {
        // Declare all (id, ViewExecutor) pairs first, then derive ViewKind via the
        // live trait method to guarantee synchronization with the trait definition.
        struct ExecutorEntry {
            id: &'static str,
            executor: &'static dyn crate::domain::views::ViewExecutor,
        }

        let entries: &[ExecutorEntry] = &[
            ExecutorEntry {
                id: "overview",
                executor: &crate::domain::views::OVERVIEW_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "call-graph",
                executor: &crate::domain::views::CALLGRAPH_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "source",
                executor: &crate::domain::views::SOURCE_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "quality",
                executor: &crate::domain::views::QUALITY_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "evidence",
                executor: &crate::domain::views::EVIDENCE_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "symbols",
                executor: &crate::domain::views::SYMBOLS_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "dependencies",
                executor: &crate::domain::views::DEPENDENCIES_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "hotspots",
                executor: &crate::domain::views::HOTSPOTS_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "architecture-drift",
                executor: &crate::domain::views::ARCHITECTURE_DRIFT_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "usage-examples",
                executor: &crate::domain::views::USAGE_EXAMPLES_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "api-surface",
                executor: &crate::domain::views::API_SURFACE_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "test-slice",
                executor: &crate::domain::views::TEST_SLICE_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "debug-slice",
                executor: &crate::domain::views::DEBUG_SLICE_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "change-impact-story",
                executor: &crate::domain::views::CHANGE_IMPACT_STORY_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "ownership-map",
                executor: &crate::domain::views::OWNERSHIP_MAP_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "composed-narrative",
                executor: &crate::domain::views::COMPOSED_NARRATIVE_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "risk_map",
                executor: &crate::domain::views::RISK_MAP_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "decision-graph",
                executor: &crate::domain::views::DECISION_GRAPH_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "decision-support-pack",
                executor: &crate::domain::views::DECISION_SUPPORT_PACK_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "architecture_rationale",
                executor: &crate::domain::views::ARCHITECTURE_RATIONALE_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "doc-source",
                executor: &crate::domain::views::DOC_SOURCE_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "evidence-overview",
                executor: &crate::domain::views::EVIDENCE_OVERVIEW_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "doc_code_alignment",
                executor: &crate::domain::views::DOC_CODE_ALIGNMENT_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
            ExecutorEntry {
                id: "concept_map",
                executor: &crate::domain::views::CONCEPT_MAP_EXECUTOR
                    as &dyn crate::domain::views::ViewExecutor,
            },
        ];

        // Build map first, then derive pairs from the live trait method.
        let map: ViewExecutorMap =
            std::collections::HashMap::from_iter(entries.iter().map(|e| (e.id, e.executor)));

        // E23 uniqueness guard — derive ViewKind via the live trait method to catch
        // any drift between the ExecutorEntry copy and the actual trait definition.
        let pairs: Vec<(&str, crate::dto::ViewKind)> =
            entries.iter().map(|e| (e.id, e.executor.view_kind())).collect();
        assert_view_kind_uniqueness(&pairs);

        map
    })
}

// ============================================================================
// ViewRegistry service
// ============================================================================

/// Service-level registry for discovering built-in and (Phase 2+) runtime views.
///
/// Phase 1: `spec_store` is `None` and the runtime path returns `[]`.
pub struct ViewRegistry {
    spec_store: Option<Arc<dyn ViewSpecStore>>,
}

impl ViewRegistry {
    /// Construct a registry.
    ///
    /// `spec_store` is `None` in Phase 1. Phase 2+ passes a handle to the
    /// `PostgresViewSpecStore` so runtime view specs are included in listings.
    pub fn new(spec_store: Option<Arc<dyn ViewSpecStore>>) -> Self {
        Self { spec_store }
    }

    /// Every view that applies to `object_type`, in stable order:
    /// built-ins first (sorted alphabetically by id), then runtime specs
    /// (Phase 2+ — currently always empty).
    pub fn list_for(&self, object_type: InspectableObjectType) -> Vec<ViewDescriptorDto> {
        // Collect from both builtin providers (inventory-based) and REAL_EXECUTORS (Phase 3).
        // REAL_EXECUTORS includes all executors registered at runtime, some of which may not
        // have provider wrappers (Phase 3: evidence, symbols, dependencies, hotspots, etc.).
        let mut descriptors: Vec<ViewDescriptorDto> = Vec::new();

        // Add from builtin providers
        for provider in builtin_providers() {
            if provider.applies_to().contains(&object_type) {
                descriptors.push(ViewDescriptorDto::from(*provider));
            }
        }

        // Access REAL_EXECUTORS to get all registered executors
        let real_map = real_executors();

        // Collect ids already provided so we skip duplicates
        // Clone to owned Strings so the borrow chain is broken before we mutate descriptors
        let provider_ids: std::collections::HashSet<std::borrow::Cow<str>> = descriptors
            .iter()
            .map(|d| std::borrow::Cow::Owned(d.id.clone()))
            .collect();

        for (id, executor) in real_map.iter() {
            if provider_ids.contains(*id) {
                continue; // Already added from providers
            }
            if executor.applies_to().contains(&object_type) {
                descriptors.push(ViewDescriptorDto {
                    id: id.to_string(),
                    title: executor.title().to_string(),
                    view_kind: executor.view_kind(),
                    is_builtin: true,
                    source: None,
                });
            }
        }

        // Sort alphabetically by id for stable ordering
        descriptors.sort_by_key(|d| d.id.clone());
        descriptors
    }

    /// Look up a single view executor by id.
    ///
    /// Returns `None` when no capability with that id is registered.
    /// Phase 2+ will also check runtime-registered executors.
    ///
    /// Phase 2 implementation: first checks real ViewExecutor implementations
    /// (OverviewExecutor, CallGraphExecutor, SourceExecutor, QualityExecutor),
    /// then falls back to ProviderExecutorAdapter for unregistered ids.
    pub fn get_executor(
        &self,
        id: &str,
    ) -> Option<&'static dyn crate::domain::views::ViewExecutor> {
        // Phase 3: all 8 real executors take priority over provider adapters.
        let real = real_executors();
        real.get(id).copied().or_else(|| {
            // Fall back to provider adapters for any ids not covered by Phase 2 executors.
            static EXECUTORS: OnceLock<
                std::collections::HashMap<
                    &'static str,
                    Box<dyn crate::domain::views::ViewExecutor>,
                >,
            > = OnceLock::new();
            let executors = EXECUTORS.get_or_init(|| {
                let mut map = std::collections::HashMap::new();
                for wrapper in inventory::iter::<ProviderWrapper> {
                    let provider = wrapper.provider;
                    let id = provider.id();
                    // Skip if already provided by a Phase 2 real executor.
                    if real.contains_key(id) {
                        continue;
                    }
                    let executor: Box<dyn crate::domain::views::ViewExecutor> =
                        Box::new(ProviderExecutorAdapter { provider });
                    map.insert(id, executor);
                }
                map
            });
            executors
                .get(id)
                .map(|b| b.as_ref() as &dyn crate::domain::views::ViewExecutor)
        })
    }

    /// Async version of `list_for` that merges built-in descriptors with
    /// runtime specs from the store (Phase 2+).
    ///
    /// Built-ins are listed first (sorted alphabetically by id), then
    /// runtime specs from the store (sorted by title, stable).
    /// Uses `list_for_workspace` to show all owners' specs (all-owners-visible model).
    pub async fn list_for_with_store(
        &self,
        object_type: InspectableObjectType,
        workspace_id: &str,
    ) -> Vec<ViewDescriptorDto> {
        let mut descriptors: Vec<ViewDescriptorDto> = self.list_for(object_type);

        if let Some(store) = &self.spec_store {
            let existing_ids: std::collections::HashSet<_> =
                descriptors.iter().map(|d| d.id.clone()).collect();

            // Fetch ALL runtime specs for this workspace + object type (all owners visible)
            if let Ok(specs) = store.list_for_workspace(workspace_id, object_type).await {
                for spec in specs {
                    // Skip if already in built-in list
                    if !existing_ids.contains(&spec.id) {
                        descriptors.push(ViewDescriptorDto {
                            id: spec.id.clone(),
                            title: spec.title.clone(),
                            view_kind: spec.view_kind,
                            is_builtin: false,
                            source: Some("runtime".to_string()),
                        });
                    }
                }
            }
        }

        descriptors
    }

    /// Look up a single view by id across built-ins (Phase 1) and runtime
    /// (Phase 2+). Returns `None` when the id is unknown.
    ///
    /// Note: this is a sync version that only checks built-ins.
    /// For runtime lookup with workspace context, use `get_with_store`.
    pub fn get(&self, id: &str) -> Option<ViewDescriptorDto> {
        builtin_providers()
            .iter()
            .find(|provider| provider.id() == id)
            .map(|provider| ViewDescriptorDto::from(*provider))
    }

    /// Async version of `get` that also looks up runtime specs by id
    /// scoped to `(workspace_id, owner)`.
    pub async fn get_with_store(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> Option<ViewDescriptorDto> {
        // Check built-ins first
        if let Some(descriptor) = self.get(id) {
            return Some(descriptor);
        }
        // Check runtime specs
        if let Some(store) = &self.spec_store {
            if let Ok(Some(spec)) = store.load(id, workspace_id, owner).await {
                return Some(ViewDescriptorDto {
                    id: spec.id.clone(),
                    title: spec.title.clone(),
                    view_kind: spec.view_kind,
                    is_builtin: false,
                    source: Some("runtime".to_string()),
                });
            }
        }
        None
    }

    /// Stable catalog of all known [`ViewKind`] values.
    ///
    /// Phase 1 returns the Rust enum's variants. Phase 2+ may extend this
    /// with runtime-registered kinds.
    pub fn known_view_kinds(&self) -> &'static [ViewKind] {
        static KNOWN_KINDS: OnceLock<Vec<ViewKind>> = OnceLock::new();
        KNOWN_KINDS.get_or_init(|| {
            vec![
                ViewKind::VerticalSlice,
                ViewKind::CallGraph,
                ViewKind::SeamMap,
                ViewKind::DependencyGraph,
                ViewKind::SourceView,
                ViewKind::DataFlow,
                ViewKind::ImpactRadius,
                ViewKind::DiffView,
                ViewKind::C4Context,
                ViewKind::C4Container,
                ViewKind::C4Component,
                ViewKind::C4Code,
                ViewKind::QualityHotspots,
                ViewKind::EvidenceView,
                ViewKind::DecisionGraph,
                ViewKind::DecisionSupportPack,
                ViewKind::ArchitectureRationale,
                ViewKind::ArchitectureDrift,
                ViewKind::BoundaryMap,
                ViewKind::DependencyPressure,
                ViewKind::ChangeImpactStory,
                ViewKind::OwnershipMap,
                ViewKind::RiskMap,
                ViewKind::DecisionTrace,
                ViewKind::TestSlice,
                ViewKind::DebugSlice,
                ViewKind::RefactorPlan,
                ViewKind::CallersAndImplementors,
                ViewKind::UsageExamples,
                ViewKind::ApiSurface,
                ViewKind::DeadCodeCandidates,
                ViewKind::SemanticSearchResults,
                ViewKind::DocCodeAlignment,
                ViewKind::ExampleObject,
                ViewKind::ComposedNarrative,
                ViewKind::ProjectDiary,
                ViewKind::ConceptMap,
                ViewKind::EvidencePack,
            ]
        });
        KNOWN_KINDS.get().unwrap().as_slice()
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Trait object safety ---

    #[test]
    fn view_descriptor_provider_is_object_safe() {
        fn _check(_: &dyn ViewDescriptorProvider) {}
        // If this compiles, the trait is object-safe (no methods that prevent it).
    }

    // --- ViewDescriptor::from ---

    #[test]
    fn view_descriptor_from_provider_extracts_metadata() {
        struct MockProvider;
        impl ViewDescriptorProvider for MockProvider {
            fn id(&self) -> &'static str {
                "test-view"
            }
            fn title(&self) -> &'static str {
                "Test View"
            }
            fn applies_to(&self) -> &'static [InspectableObjectType] {
                &[InspectableObjectType::Symbol]
            }
            fn view_kind(&self) -> ViewKind {
                ViewKind::CallGraph
            }
        }
        let provider = MockProvider;
        let descriptor = ViewDescriptorDto::from(&provider as &dyn ViewDescriptorProvider);
        assert_eq!(descriptor.id, "test-view");
        assert_eq!(descriptor.title, "Test View");
    }

    // --- get returns None for unknown id ---

    #[test]
    fn get_returns_none_for_unknown_id() {
        let registry = ViewRegistry::new(None);
        let result = registry.get("this-does-not-exist");
        assert!(result.is_none());
    }

    // --- known_view_kinds returns all variants ---

    #[test]
    fn known_view_kinds_returns_all_view_kind_variants() {
        let registry = ViewRegistry::new(None);
        let kinds = registry.known_view_kinds();
        assert!(!kinds.is_empty());
        assert!(kinds.contains(&ViewKind::CallGraph));
        assert!(kinds.contains(&ViewKind::SourceView));
        assert!(kinds.contains(&ViewKind::QualityHotspots));
        assert!(kinds.contains(&ViewKind::DecisionSupportPack));
    }

    // --- known_view_kinds is stable (same slice on multiple calls) ---

    #[test]
    fn known_view_kinds_is_stable() {
        let registry = ViewRegistry::new(None);
        let first = registry.known_view_kinds();
        let second = registry.known_view_kinds();
        assert!(std::ptr::eq(first, second));
    }

    // --- list_for returns descriptors for any type that has matching executors ---

    #[test]
    fn list_for_returns_descriptors_for_matching_types() {
        let registry = ViewRegistry::new(None);
        // list_for now derives descriptors from both builtin providers AND REAL_EXECUTORS.
        // Doc now returns doc-source (D4). Evidence returns evidence-overview (D4).
        let doc_views = registry.list_for(InspectableObjectType::Doc);
        assert!(
            doc_views.iter().any(|v| v.id == "doc-source"),
            "expected doc-source for Doc, got {doc_views:?}"
        );
        let evidence_views = registry.list_for(InspectableObjectType::Evidence);
        assert!(
            evidence_views.iter().any(|v| v.id == "evidence-overview"),
            "expected evidence-overview for Evidence, got {evidence_views:?}"
        );
    }

    // --- DecisionArtifact list includes decision-support-pack with correct view_kind ---

    #[test]
    fn list_for_decision_artifact_includes_decision_support_pack() {
        let registry = ViewRegistry::new(None);
        let views = registry.list_for(InspectableObjectType::DecisionArtifact);
        let pack_view = views
            .iter()
            .find(|v| v.id == "decision-support-pack");
        assert!(
            pack_view.is_some(),
            "expected decision-support-pack for DecisionArtifact, got {views:?}"
        );
        let pack_view = pack_view.unwrap();
        assert_eq!(pack_view.view_kind, ViewKind::DecisionSupportPack);
        assert_eq!(pack_view.title, "Decision Support Pack");
        assert!(pack_view.is_builtin);
    }

    // --- Built-in providers are registered and accessible ---

    #[test]
    fn built_in_providers_are_accessible() {
        let registry = ViewRegistry::new(None);
        // The 4 built-in providers (overview, call-graph, source, quality)
        // are registered via inventory::submit! in domain/views.rs at compile time.
        // Verify they are accessible through the registry.
        let overview = registry.get("overview");
        assert!(overview.is_some(), "expected overview to be registered");
        assert_eq!(overview.unwrap().title, "Overview");

        let callgraph = registry.get("call-graph");
        assert!(callgraph.is_some(), "expected call-graph to be registered");
        assert_eq!(callgraph.unwrap().title, "Call Graph");

        let source = registry.get("source");
        assert!(source.is_some(), "expected source to be registered");
        assert_eq!(source.unwrap().title, "Source");

        let quality = registry.get("quality");
        assert!(quality.is_some(), "expected quality to be registered");
        assert_eq!(quality.unwrap().title, "Quality");
    }

    // --- Doc and Evidence executors are registered (D4) ---

    #[test]
    fn doc_executor_is_registered() {
        let registry = ViewRegistry::new(None);
        let views = registry.list_for(InspectableObjectType::Doc);
        assert!(
            !views.is_empty(),
            "expected at least 1 view for Doc, got {}",
            views.len()
        );
        let ids: Vec<&str> = views.iter().map(|v| v.id.as_str()).collect();
        assert!(
            ids.contains(&"doc-source"),
            "expected doc-source in views for Doc, got {ids:?}"
        );
    }

    #[test]
    fn evidence_executor_is_registered() {
        let registry = ViewRegistry::new(None);
        let views = registry.list_for(InspectableObjectType::Evidence);
        assert!(
            !views.is_empty(),
            "expected at least 1 view for Evidence, got {}",
            views.len()
        );
        let ids: Vec<&str> = views.iter().map(|v| v.id.as_str()).collect();
        // evidence-overview is distinct from evidence (which applies to Symbol)
        assert!(
            ids.contains(&"evidence-overview"),
            "expected evidence-overview in views for Evidence, got {ids:?}"
        );
    }

    // --- ViewSpecStore error conversions ---

    #[test]
    fn view_spec_store_error_display() {
        use super::ViewSpecStoreError;
        let err = ViewSpecStoreError::Store("connection failed".into());
        assert!(err.to_string().contains("connection failed"));

        let err = ViewSpecStoreError::Conflict("duplicate title".into());
        assert!(err.to_string().contains("duplicate title"));

        let err = ViewSpecStoreError::NotFound("missing-id".into());
        assert!(err.to_string().contains("missing-id"));
    }

    // --- ViewSpecStore is Send + Sync (marker trait guarantee) ---

    #[test]
    fn view_spec_store_is_send_sync() {
        fn _check<T: Send + Sync>() {}
        // The ViewSpecStore trait itself is Send + Sync by requirement.
        // We just verify the trait bound compiles.
        fn _accept_store<S: super::ViewSpecStore>(_: &S) {}
    }

    // --- E23 uniqueness guard: panic on duplicate ViewKind ---

    /// Scenario 21: assert_view_kind_uniqueness panics when two ids share the same ViewKind.
    #[test]
    #[should_panic(expected = "duplicate ViewKind registration")]
    fn assert_view_kind_uniqueness_panics_on_duplicate() {
        let pairs = &[("a", ViewKind::CallGraph), ("b", ViewKind::CallGraph)];
        super::assert_view_kind_uniqueness(pairs);
    }

    /// E23 positive uniqueness: every real executor has a distinct ViewKind.
    #[test]
    fn real_executors_view_kinds_are_distinct() {
        let map = super::real_executors();
        let mut ids: Vec<&str> = Vec::new();
        let mut kinds: Vec<ViewKind> = Vec::new();
        for (id, executor) in map.iter() {
            ids.push(id);
            kinds.push(executor.view_kind());
        }
        let unique_kinds: std::collections::HashSet<_> = kinds.iter().collect();
        assert_eq!(
            unique_kinds.len(),
            kinds.len(),
            "duplicate ViewKind across executors: ids={:?}, kinds={:?}",
            ids,
            kinds
        );
    }
}
