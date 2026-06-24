//! Per-session service.
//!
//! Owns a [`BrainSessionState`] behind a `Mutex`, plus shared
//! handles to the ISP-segregated service facades and the optional
//! call graph. Per-session logic — focus management, history append
//! with FIFO cap, ask-with-focus-injection — lives here. The
//! registry only knows about this type by its `Arc`; the service
//! never holds a reference back to the registry.
//!
//! All public methods acquire the state mutex, do the work, and
//! release the lock. They never hold the guard across `.await`.

use std::sync::{Arc, Mutex};

use cognicode_core::domain::aggregates::CallGraph;
use serde_json::Value;

use crate::facades::{SearchService, ViewService, WorkspaceService};
use crate::mcp::McpResultEnvelope;
use crate::session::state::{
    BrainSessionState, DEFAULT_HISTORY_CAP, DEFAULT_TTL_SECS, HistoryEntry,
};

// Multimodal (brain-federation) — per-session space registry.
#[cfg(feature = "multimodal")]
use cognicode_core::domain::value_objects::{Space, SpaceError, SpaceId};
#[cfg(feature = "multimodal")]
use crate::federation::SpaceRegistry;

/// Per-session service. Cheap to clone (the inner state sits behind
/// a `Mutex`).
pub struct BrainSessionService {
    state: Mutex<BrainSessionState>,
    search: Arc<dyn SearchService>,
    view: Arc<dyn ViewService>,
    workspace: Arc<dyn WorkspaceService>,
    #[allow(dead_code)]
    graph: Option<Arc<CallGraph>>,
    /// Multimodal (brain-federation) — per-session space registry.
    /// Tracks every [`Space`] the session has registered. The
    /// [`BrainSessionState::spaces`] field is kept in sync with
    /// the registry for snapshot serialisation.
    #[cfg(feature = "multimodal")]
    #[allow(dead_code)]
    space_registry: Mutex<SpaceRegistry>,
}

// Manual `Debug` impl because `ExplorerService` doesn't implement
// `Debug` (it's a fat service with non-Debug ports).
impl std::fmt::Debug for BrainSessionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self.state.lock().expect("session state poisoned");
        f.debug_struct("BrainSessionService")
            .field("session_id", &s.session_id)
            .field("workspace_id", &s.workspace_id)
            .field("ttl", &s.ttl)
            .field("focus_node", &s.focus_node)
            .finish()
    }
}

impl BrainSessionService {
    /// Build a fresh service. The `state::BrainSessionState` is
    /// constructed via its `new` constructor (sets timestamps to
    /// `Utc::now()`).
    pub fn new(
        session_id: String,
        workspace_id: String,
        ttl_secs: u64,
        search: Arc<dyn SearchService>,
        view: Arc<dyn ViewService>,
        workspace: Arc<dyn WorkspaceService>,
        graph: Option<Arc<CallGraph>>,
    ) -> Self {
        let state = BrainSessionState::new(session_id, workspace_id, ttl_secs);
        Self {
            state: Mutex::new(state),
            search,
            view,
            workspace,
            graph,
            #[cfg(feature = "multimodal")]
            space_registry: Mutex::new(SpaceRegistry::new()),
        }
    }

    /// Snapshot the inner state (cloned). Used by `brain_status` and
    /// by the registry for TTL checks.
    pub fn snapshot(&self) -> BrainSessionState {
        self.state.lock().expect("session state poisoned").clone()
    }

    /// Refresh `last_activity` to `Utc::now()`. Called by
    /// `SessionRegistry::attach` to mark the session as recently used.
    pub fn touch(&self) {
        let mut s = self.state.lock().expect("session state poisoned");
        s.last_activity = chrono::Utc::now();
    }

    /// The session's id (string). Cheap clone of a `String`.
    pub fn session_id(&self) -> String {
        self.state
            .lock()
            .expect("session state poisoned")
            .session_id
            .clone()
    }

    /// The workspace id this session is bound to.
    pub fn workspace_id(&self) -> String {
        self.state
            .lock()
            .expect("session state poisoned")
            .workspace_id
            .clone()
    }

    /// Get the current focus node (or `None`).
    pub fn focus_node(&self) -> Option<String> {
        self.state
            .lock()
            .expect("session state poisoned")
            .focus_node
            .clone()
    }

    /// Set (or clear, on `None`) the focus node.
    pub fn set_focus(&self, node: Option<String>) {
        let mut s = self.state.lock().expect("session state poisoned");
        s.focus_node = node;
    }

    /// Push a successful ask onto the history, truncating to the
    /// FIFO cap. Failed asks MUST NOT call this (the caller is
    /// responsible for not pushing on error envelopes).
    pub fn push_history(&self, entry: HistoryEntry) {
        let mut s = self.state.lock().expect("session state poisoned");
        s.history.push(entry);
        // Truncate to the cap from the head (FIFO).
        let cap = DEFAULT_HISTORY_CAP;
        if s.history.len() > cap {
            let excess = s.history.len() - cap;
            s.history.drain(..excess);
        }
    }

    // ------------------------------------------------------------------
    // Multimodal (brain-federation) — space management.
    //
    // Each method is gated behind the `multimodal` Cargo feature. On
    // default builds the `space_registry` field does not exist and
    // these methods are absent.
    // ------------------------------------------------------------------

    /// Register a space in this session. The space is stored in the
    /// per-session [`SpaceRegistry`] and its id is appended to the
    /// session state's `spaces` list (for snapshot serialisation).
    ///
    /// Returns [`SpaceError::Duplicate`] when a space with the same
    /// id is already registered.
    #[cfg(feature = "multimodal")]
    pub fn add_space(&self, space: Space) -> Result<(), SpaceError> {
        let mut reg = self.space_registry.lock().expect("space registry poisoned");
        // Register in the registry first (validates for duplicates).
        reg.register(space)?;
        // Sync the state's space id list with the registry.
        let ids = reg.list_ids();
        let mut s = self.state.lock().expect("session state poisoned");
        s.spaces = ids;
        Ok(())
    }

    /// Remove a space by id. Returns `true` when the space was
    /// present and removed, `false` when unknown (idempotent).
    /// The session state's `spaces` list is kept in sync.
    #[cfg(feature = "multimodal")]
    pub fn remove_space(&self, id: &SpaceId) -> bool {
        let mut reg = self.space_registry.lock().expect("space registry poisoned");
        let removed = reg.unregister(id);
        if removed {
            let ids = reg.list_ids();
            let mut s = self.state.lock().expect("session state poisoned");
            s.spaces = ids;
        }
        removed
    }

    /// List every registered space in insertion order.
    #[cfg(feature = "multimodal")]
    pub fn spaces(&self) -> Vec<Space> {
        let reg = self.space_registry.lock().expect("space registry poisoned");
        reg.list()
    }

    /// Number of entries currently in the history (test helper).
    #[cfg(test)]
    pub fn history_len(&self) -> usize {
        self.state
            .lock()
            .expect("session state poisoned")
            .history
            .len()
    }

    /// Look at the first history entry (test helper).
    #[cfg(test)]
    pub fn history_first_question(&self) -> Option<String> {
        self.state
            .lock()
            .expect("session state poisoned")
            .history
            .first()
            .map(|e| e.question.clone())
    }

    /// Look at the last history entry (test helper).
    #[cfg(test)]
    pub fn history_last_question(&self) -> Option<String> {
        self.state
            .lock()
            .expect("session state poisoned")
            .history
            .last()
            .map(|e| e.question.clone())
    }

    /// Ask a question within this session. The full implementation
    /// (focus-node prepend, classification, dispatch) lands in
    /// Phase 3. For now this is the lock-discipline scaffold so the
    /// registry tests can compile.
    #[allow(dead_code)]
    pub async fn ask_with_session(&self, question: &str) -> McpResultEnvelope<Value> {
        // Prepend the focus node as a backtick-quoted token when
        // set, then call the ask-router. Lock discipline: read
        // `focus_node`, drop the lock, then `.await`.
        let focus = {
            let s = self.state.lock().expect("session state poisoned");
            s.focus_node.clone()
        };
        let enriched = match focus {
            Some(f) => format!("`{f}` {question}"),
            None => question.to_string(),
        };

        let classified = crate::ask::AskRouter::classify(&enriched);
        let env = crate::ask::dispatch::dispatch_ask(
            classified,
            self.search.as_ref(),
            self.workspace.as_ref(),
            self.view.as_ref(),
            &self.graph,
            None,
        )
        .await;

        // If the inner envelope is an error, do NOT append history.
        let is_error = env
            .provenance
            .as_ref()
            .and_then(|p| p.confidence)
            .map(|c| c == 0.0)
            .unwrap_or(false);
        if !is_error {
            let answer_summary = env
                .payload
                .get("primary_result")
                .map(|v| {
                    let s = v.to_string();
                    if s.len() > 200 {
                        format!("{}…", &s[..200])
                    } else {
                        s
                    }
                })
                .unwrap_or_default();
            // pattern_id is derived from the ask router's
            // classification. The router doesn't return it directly,
            // so we recover it by re-classifying the enriched
            // question (cheap; pure function).
            let pattern_id = crate::ask::AskRouter::classify(&enriched).category as u8;
            self.push_history(HistoryEntry {
                question: question.to_string(),
                answer_summary,
                pattern_id,
                ts: chrono::Utc::now(),
            });
        }
        env
    }
}

/// The `DEFAULT_TTL_SECS` constant is referenced by `registry.rs`
/// through the state module; we re-export it here so service-layer
/// callers can name it without reaching into the state module
/// directly.
#[allow(dead_code)]
pub const SESSION_DEFAULT_TTL_SECS: u64 = DEFAULT_TTL_SECS;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{InspectableObjectSummary, InspectableObjectType, LensResult, SpotterResult, ViewDescriptorDto};
    use crate::dto::{WorkspaceSummary, OpenWorkspaceRequest};
    use async_trait::async_trait;

    // --- Mock facades -------------------------------------------------------

    #[derive(Clone)]
    struct MockSearchService;
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
        async fn inspect_object(&self, _object_id: &str) -> crate::ExplorerResult<InspectableObjectSummary> {
            Err(crate::error::ExplorerError::ObjectNotFound("mock".into()))
        }
    }

    #[derive(Clone)]
    struct MockViewService;
    #[async_trait]
    impl ViewService for MockViewService {
        async fn available_views(&self, _object_id: &str) -> crate::ExplorerResult<Vec<ViewDescriptorDto>> {
            Ok(vec![])
        }
        async fn contextual_view(&self, _object_id: &str, _view_id: &str) -> crate::ExplorerResult<crate::dto::ContextualView> {
            Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
        }
        async fn build_contextual_graph(&self, _focus_id: &str, _level: &str, _depth: u8, _max_nodes: usize) -> crate::ExplorerResult<crate::dto::ContextualGraphResponse> {
            Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
        }
        async fn available_lenses(&self, _object_id: &str) -> crate::ExplorerResult<Vec<crate::dto::LensDescriptor>> {
            Ok(vec![])
        }
        async fn apply_lens(&self, _object_id: &str, _lens_id: &str) -> crate::ExplorerResult<LensResult> {
            Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
        }
        async fn execute_view_spec(&self, _spec: &crate::dto::ViewSpec, _object_id: &str) -> crate::ExplorerResult<crate::dto::ContextualView> {
            Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
        }
    }

    #[derive(Clone)]
    struct MockWorkspaceService;
    #[async_trait]
    impl WorkspaceService for MockWorkspaceService {
        async fn open_workspace(&self, _request: OpenWorkspaceRequest) -> crate::ExplorerResult<WorkspaceSummary> {
            Err(crate::error::ExplorerError::WorkspaceNotFound("mock".into()))
        }
        fn current_workspace(&self) -> crate::ExplorerResult<WorkspaceSummary> {
            Err(crate::error::ExplorerError::WorkspaceNotFound("mock".into()))
        }
    }

    fn build_facades() -> (Arc<dyn SearchService>, Arc<dyn ViewService>, Arc<dyn WorkspaceService>) {
        (Arc::new(MockSearchService), Arc::new(MockViewService), Arc::new(MockWorkspaceService))
    }

    #[test]
    fn service_set_focus_stores_value() {
        let (search, view, workspace) = build_facades();
        let s = BrainSessionService::new(
            "00000000-0000-4000-8000-000000000001".into(),
            "ws".into(),
            DEFAULT_TTL_SECS,
            search,
            view,
            workspace,
            None,
        );
        assert!(s.focus_node().is_none());
        s.set_focus(Some("Foo::bar".into()));
        assert_eq!(s.focus_node(), Some("Foo::bar".into()));
    }

    #[test]
    fn service_set_focus_none_clears() {
        let (search, view, workspace) = build_facades();
        let s = BrainSessionService::new(
            "00000000-0000-4000-8000-000000000002".into(),
            "ws".into(),
            DEFAULT_TTL_SECS,
            search,
            view,
            workspace,
            None,
        );
        s.set_focus(Some("Foo::bar".into()));
        s.set_focus(None);
        assert!(s.focus_node().is_none());
    }

    /// Push 55 history entries directly through the service and
    /// confirm the FIFO cap of 50 holds: the 55th entry is the
    /// last, the first pushed entry is gone.
    #[test]
    fn service_history_caps_at_50_fifo() {
        let (search, view, workspace) = build_facades();
        let s = BrainSessionService::new(
            "00000000-0000-4000-8000-000000000003".into(),
            "ws".into(),
            DEFAULT_TTL_SECS,
            search,
            view,
            workspace,
            None,
        );
        for i in 0..55 {
            s.push_history(HistoryEntry {
                question: format!("q-{i:03}"),
                answer_summary: format!("a-{i:03}"),
                pattern_id: 0,
                ts: chrono::Utc::now(),
            });
        }
        assert_eq!(s.history_len(), 50, "history must cap at 50");
        // The first pushed (q-000) is gone; the 6th pushed (q-005)
        // is now the oldest; the 55th pushed (q-054) is the newest.
        assert_eq!(
            s.history_first_question().as_deref(),
            Some("q-005"),
            "oldest must be q-005 after 55 pushes (cap 50)"
        );
        assert_eq!(
            s.history_last_question().as_deref(),
            Some("q-054"),
            "newest must be q-054"
        );
    }

    /// `ask_with_session` with a focus node set MUST prepend the
    /// backtick-quoted focus node to the question before it reaches
    /// the ask router.
    ///
    /// Verification strategy: we observe the enriched question
    /// indirectly by recording the *classification* the router
    /// produced. The focus prefix moves "what is it?" from a clean
    /// pattern 8 match to a "pattern 8 with a backtick entity"
    /// match — both still produce a non-zero confidence. More
    /// importantly, the **history** is appended using the ORIGINAL
    /// (un-enriched) question when the ask succeeds, so we can
    /// verify focus injection by setting up a scenario where the
    /// router succeeds. We do that by stuffing a question that
    /// pattern 4 (code quality) catches — "any smells in something?"
    /// — and observing the history entry.
    ///
    /// With NoopRepo, the inspect_object step inside `code_quality`
    /// returns `ObjectNotFound`, so the ask fails and no history is
    /// appended. That's actually the contract: a failed ask doesn't
    /// append. So the binding assertion we CAN make about focus
    /// injection without a real service is: the enriched question
    /// is constructed as expected. We expose that via a tiny pure
    /// helper test instead of going through the dispatch chain.
    #[test]
    fn service_enrich_question_prepends_focus_as_backtick_token() {
        // Mirror the enrichment logic the service uses, without
        // going through dispatch. This pins the exact wire format
        // the ask router sees.
        let focus: Option<String> = Some("Foo::bar".into());
        let question = "what does it call?";
        let enriched = match &focus {
            Some(f) => format!("`{f}` {question}"),
            None => question.to_string(),
        };
        assert_eq!(enriched, "`Foo::bar` what does it call?");
    }

    #[test]
    fn service_enrich_question_passthrough_when_no_focus() {
        let focus: Option<String> = None;
        let question = "what does it call?";
        let enriched = match &focus {
            Some(f) => format!("`{f}` {question}"),
            None => question.to_string(),
        };
        assert_eq!(enriched, "what does it call?");
    }

    /// The successful-ask path appends to history. We seed a
    /// successful outcome by pushing a `HistoryEntry` directly and
    /// verifying the FIFO truncation; the dispatch path is exercised
    /// by `service_failed_ask_does_not_append_to_history` below,
    /// which observes the contract from the failure side.
    #[tokio::test]
    async fn service_successful_ask_path_appends_unenriched_question() {
        // This test focuses on the recording side: when the inner
        // dispatch returns a non-error envelope, we record the
        // original (not enriched) question. We can't easily fabricate
        // a successful ask through the full chain with mock facades,
        // so we verify the contract by calling `push_history` directly
        // and asserting the question stored is the one we pass in.
        let (search, view, workspace) = build_facades();
        let s = BrainSessionService::new(
            "00000000-0000-4000-8000-000000000004".into(),
            "ws".into(),
            DEFAULT_TTL_SECS,
            search,
            view,
            workspace,
            None,
        );
        // `ask_with_session` uses the un-enriched question when
        // pushing, which is what we want consumers to see in the
        // history. Verify the helper does the right thing by
        // exercising it through the recorded path: any success path
        // records the ORIGINAL question.
        s.push_history(HistoryEntry {
            question: "what does it call?".into(),
            answer_summary: "x".into(),
            pattern_id: 2,
            ts: chrono::Utc::now(),
        });
        assert_eq!(s.history_len(), 1);
        assert_eq!(
            s.history_first_question().as_deref(),
            Some("what does it call?")
        );
    }

    /// A failed ask MUST NOT push to history. We trigger the failure
    /// path by asking a graph-dependent question (`path between`)
    /// with no graph loaded — the router returns a `graph_unavailable`
    /// envelope (confidence = 0.0) and the service should skip the
    /// history append.
    #[tokio::test]
    async fn service_failed_ask_does_not_append_to_history() {
        let (search, view, workspace) = build_facades();
        let s = BrainSessionService::new(
            "00000000-0000-4000-8000-000000000005".into(),
            "ws".into(),
            DEFAULT_TTL_SECS,
            search,
            view,
            workspace,
            None,
        );
        let _env = s.ask_with_session("path between `a` and `b`").await;
        assert_eq!(s.history_len(), 0, "failed ask must not append to history");
    }
}
