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

/// Import raw built-in descriptor data from core to avoid duplication.
use cognicode_core::schemas::BUILTIN_DESCRIPTORS_RAW;

/// Convert raw built-in descriptor to dto::ViewDescriptorDto.
fn raw_to_view_descriptor(
    raw: &cognicode_core::schemas::BuiltinDescriptorRaw,
) -> ViewDescriptorDto {
    // Use the From impl for the ACL boundary.
    // The raw descriptor's to_view_descriptor() returns core_schema::ViewDescriptor,
    // which we then convert to ViewDescriptorDto.
    ViewDescriptorDto::from(raw.to_view_descriptor())
}

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
// ViewRegistry service
// ============================================================================

use std::sync::Arc;

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
        // REAL_EXECUTORS includes all 8 executors (Phase 1-3), some of which may not
        // have provider wrappers (Phase 3: evidence, symbols, dependencies, hotspots).
        let mut descriptors: Vec<ViewDescriptorDto> = Vec::new();

        // Add from builtin providers
        for provider in builtin_providers() {
            if provider.applies_to().contains(&object_type) {
                descriptors.push(ViewDescriptorDto::from(*provider));
            }
        }

        // Add from REAL_EXECUTORS that aren't already in providers (Phase 3 executors)
        // Uses shared BUILTIN_DESCRIPTORS_RAW from core to avoid duplication
        static REAL_EXECUTOR_DESCRIPTORS: OnceLock<Vec<ViewDescriptorDto>> = OnceLock::new();
        let real_descriptors = REAL_EXECUTOR_DESCRIPTORS.get_or_init(|| {
            BUILTIN_DESCRIPTORS_RAW
                .iter()
                .map(raw_to_view_descriptor)
                .collect()
        });

        // Add Phase 3 executors that apply to this object type and aren't duplicates
        let provider_ids: std::collections::HashSet<_> =
            descriptors.iter().map(|d| d.id.as_str()).collect();
        let mut additional: Vec<ViewDescriptorDto> = Vec::new();
        for executor_desc in real_descriptors.iter() {
            if provider_ids.contains(executor_desc.id.as_str()) {
                continue; // Already added from providers
            }
            // Check if this executor applies to the object type
            if let Some(executor) = self.get_executor(executor_desc.id.as_str()) {
                if executor.applies_to().contains(&object_type) {
                    additional.push(executor_desc.clone());
                }
            }
        }
        descriptors.extend(additional);

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
        static REAL_EXECUTORS: OnceLock<
            std::collections::HashMap<
                &'static str,
                &'static dyn crate::domain::views::ViewExecutor,
            >,
        > = OnceLock::new();
        let real = REAL_EXECUTORS.get_or_init(|| {
            std::collections::HashMap::from([
                (
                    "overview",
                    &crate::domain::views::OVERVIEW_EXECUTOR
                        as &dyn crate::domain::views::ViewExecutor,
                ),
                (
                    "call-graph",
                    &crate::domain::views::CALLGRAPH_EXECUTOR
                        as &dyn crate::domain::views::ViewExecutor,
                ),
                (
                    "source",
                    &crate::domain::views::SOURCE_EXECUTOR
                        as &dyn crate::domain::views::ViewExecutor,
                ),
                (
                    "quality",
                    &crate::domain::views::QUALITY_EXECUTOR
                        as &dyn crate::domain::views::ViewExecutor,
                ),
                (
                    "evidence",
                    &crate::domain::views::EVIDENCE_EXECUTOR
                        as &dyn crate::domain::views::ViewExecutor,
                ),
                (
                    "symbols",
                    &crate::domain::views::SYMBOLS_EXECUTOR
                        as &dyn crate::domain::views::ViewExecutor,
                ),
                (
                    "dependencies",
                    &crate::domain::views::DEPENDENCIES_EXECUTOR
                        as &dyn crate::domain::views::ViewExecutor,
                ),
                (
                    "hotspots",
                    &crate::domain::views::HOTSPOTS_EXECUTOR
                        as &dyn crate::domain::views::ViewExecutor,
                ),
                (
                    "architecture-drift",
                    &crate::domain::views::ARCHITECTURE_DRIFT_EXECUTOR
                        as &dyn crate::domain::views::ViewExecutor,
                ),
                (
                    "usage-examples",
                    &crate::domain::views::USAGE_EXAMPLES_EXECUTOR
                        as &dyn crate::domain::views::ViewExecutor,
                ),
                (
                    "api-surface",
                    &crate::domain::views::API_SURFACE_EXECUTOR
                        as &dyn crate::domain::views::ViewExecutor,
                ),
            ])
        });
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
    }

    // --- known_view_kinds is stable (same slice on multiple calls) ---

    #[test]
    fn known_view_kinds_is_stable() {
        let registry = ViewRegistry::new(None);
        let first = registry.known_view_kinds();
        let second = registry.known_view_kinds();
        assert!(std::ptr::eq(first, second));
    }

    // --- list_for returns empty for Workspace when no built-ins registered yet ---

    #[test]
    fn list_for_returns_empty_for_unregistered_type() {
        let registry = ViewRegistry::new(None);
        // Without built-in providers registered, Workspace has no matches.
        let result = registry.list_for(InspectableObjectType::Workspace);
        // If BUILTIN_PROVIDERS is empty (not yet populated), this returns [].
        // If providers are registered, Workspace might not be in their applies_to.
        assert!(
            result.is_empty(),
            "expected empty for Workspace, got {result:?}"
        );
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
}
