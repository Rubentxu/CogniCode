//! Reusable facades for the tests that need a context.
//!
//! The services of the product are the seams where a test that exercises a
//! tool end to end has to plug in. A test that needs one of them finds it
//! here instead of writing its own: a fixture written twice is a fixture
//! that drifts, and the drift is invisible until one of the two copies
//! stops describing the service it stands for.

use async_trait::async_trait;

use crate::dto::{
    InspectableObjectSummary, InspectableObjectType, LensResult, OpenWorkspaceRequest,
    SpotterResult, ViewDescriptorDto, WorkspaceSummary,
};
use crate::facades::{SearchService, ViewService, WorkspaceService};

/// A search that finds nothing, which is what a workspace with no indexed
/// object looks like to the services.
#[derive(Clone)]
pub struct MockSearchService;

#[async_trait]
impl SearchService for MockSearchService {
    async fn spotter_search(
        &self,
        _query: &str,
        _kind: Option<&str>,
    ) -> crate::ExplorerResult<Vec<SpotterResult>> {
        Ok(vec![])
    }

    async fn spotter_search_with_viewspecs(
        &self,
        _query: &str,
        _kind: Option<&str>,
        _workspace_id: Option<&str>,
    ) -> crate::ExplorerResult<Vec<crate::dto::SpotterSearchResult>> {
        Ok(vec![])
    }

    async fn inspect_object(
        &self,
        object_id: &str,
    ) -> crate::ExplorerResult<InspectableObjectSummary> {
        // The object exists and has a name. A test that needs a service that
        // answers finds one here, and a test that needs a service that
        // refuses finds the other one.
        Ok(InspectableObjectSummary {
            id: object_id.to_string(),
            object_type: InspectableObjectType::Symbol,
            label: object_id.to_string(),
            subtitle: String::new(),
            properties: Vec::new(),
            available_views: Vec::new(),
        })
    }
}

/// A view service that answers with a view of nothing.
#[derive(Clone)]
pub struct MockViewService;

#[async_trait]
impl ViewService for MockViewService {
    async fn available_views(
        &self,
        _object_id: &str,
    ) -> crate::ExplorerResult<Vec<ViewDescriptorDto>> {
        Ok(vec![])
    }

    async fn contextual_view(
        &self,
        object_id: &str,
        view_id: &str,
    ) -> crate::ExplorerResult<crate::dto::ContextualView> {
        // A view that answers and has nothing in it, which is what a view of
        // nothing looks like.
        Ok(crate::dto::ContextualView {
            object_id: object_id.to_string(),
            view_id: view_id.to_string(),
            title: view_id.to_string(),
            ..Default::default()
        })
    }

    async fn build_contextual_graph(
        &self,
        _focus_id: &str,
        _level: &str,
        _depth: u8,
        _max_nodes: usize,
    ) -> crate::ExplorerResult<crate::dto::ContextualGraphResponse> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }

    async fn available_lenses(
        &self,
        _object_id: &str,
    ) -> crate::ExplorerResult<Vec<crate::dto::LensDescriptor>> {
        Ok(vec![])
    }

    async fn apply_lens(
        &self,
        _object_id: &str,
        _lens_id: &str,
    ) -> crate::ExplorerResult<LensResult> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }

    async fn execute_view_spec(
        &self,
        _spec: &crate::dto::ViewSpec,
        _object_id: &str,
    ) -> crate::ExplorerResult<crate::dto::ContextualView> {
        Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
    }
}

/// A workspace service with no workspace open.
#[derive(Clone)]
pub struct MockWorkspaceService;

#[async_trait]
impl WorkspaceService for MockWorkspaceService {
    async fn open_workspace(
        &self,
        _request: OpenWorkspaceRequest,
    ) -> crate::ExplorerResult<WorkspaceSummary> {
        Err(crate::error::ExplorerError::WorkspaceNotFound(
            "mock".into(),
        ))
    }

    fn current_workspace(&self) -> crate::ExplorerResult<WorkspaceSummary> {
        Err(crate::error::ExplorerError::WorkspaceNotFound(
            "mock".into(),
        ))
    }
}

/// The three services a context needs, ready to be plugged in.
pub fn build_mocks() -> (MockSearchService, MockViewService, MockWorkspaceService) {
    (MockSearchService, MockViewService, MockWorkspaceService)
}
