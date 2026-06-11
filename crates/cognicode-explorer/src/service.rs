use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde_json::json;

use crate::domain::evidence::build_evidence_blocks;
use crate::domain::lens::{LensContext, LensRegistry, default_registry};
use crate::domain::object_identity::ObjectIdentity;
use crate::domain::views;
use crate::dto::{
    ChildrenSection, ContextualGraphResponse, ContextualView, DecisionArtifactSummary,
    ExplorationPath, GenerateArtifactRequest, GraphEdge, GraphNode, GraphStatus,
    InspectableObjectSummary, InspectableObjectType, LensDescriptor, LensResult, NamedView,
    NamedViewDescriptor, ObjectIdentityEntry, OpenWorkspaceRequest, ParentSection, Property,
    SameLevelSection, SaveExplorationRequest, SpotterResult, ViewBlock, ViewDescriptor,
    WorkspaceSummary, truncate_description,
};
use crate::error::{ExplorerError, ExplorerResult};
use crate::moldql::{MoldQLExecutor, MoldQLResult, MoldQLView};
use crate::ports::quality_repository::QualityRepository;
use crate::ports::search_repository::{SearchHit, SearchRepository};
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::{ResolvedSymbol, SymbolRepository};

use cognicode_core::domain::aggregates::SymbolId;

#[cfg(feature = "postgres")]
use cognicode_core::infrastructure::persistence::PostgresRepository;

/// How many hotspots the scope view surfaces.
const SCOPE_HOTSPOT_LIMIT: usize = 5;

/// Cap on the number of Spotter results returned per query. FTS5 + exact
/// matches are merged, deduplicated, then trimmed to this many.
const SPOTTER_RESULT_LIMIT: usize = 20;

pub struct ExplorerService {
    repo: Arc<dyn SymbolRepository>,
    reader: Arc<dyn SourceReader>,
    root_path: PathBuf,
    /// Optional FTS5 / fuzzy search backend. `None` = exact-match only.
    search: Option<Arc<dyn SearchRepository>>,
    /// Optional quality backend. `None` = quality views degrade to empty
    /// state (no findings, no gate, no error). The `Option` keeps the
    /// service construction backward-compatible — the public constructors
    /// `new` and `with_search` keep working without changes.
    quality: Option<Arc<dyn QualityRepository>>,
    /// Registry of design lenses (Phase 4). Every constructor builds the
    /// default set (`hotspots`, `dependencies`, `architecture`) so callers
    /// never see an empty registry unless they explicitly construct one.
    lens_registry: Arc<LensRegistry>,
    /// In-memory store of saved exploration paths, keyed by exploration id.
    /// Phase 1C: process-lifetime only — paths do not survive a restart.
    paths: Arc<Mutex<HashMap<String, ExplorationPath>>>,
    /// Optional PostgreSQL repository for named views CRUD. Only
    /// populated when the binary was started with the `postgres`
    /// feature and a `PostgresRepository` was wired in via
    /// [`ExplorerService::with_postgres_repo`]. When `None`, every
    /// `*_view` method short-circuits with
    /// `ExplorerError::FeatureDisabled` and the MCP dispatch
    /// surfaces the canonical
    /// `"named_views_require_postgres_feature"` message.
    #[cfg(feature = "postgres")]
    postgres_repo: Option<Arc<PostgresRepository>>,
    /// Optional Generic Graph Layer port for multimodal queries.
    /// Populated when `multimodal` feature is enabled and a
    /// GraphRepository has been wired in.
    #[cfg(feature = "multimodal")]
    graph_repo: Option<Arc<dyn crate::ports::GraphRepository>>,
}

impl ExplorerService {
    /// Build a service with no FTS5 backend (Phase 1B behaviour).
    pub fn new(
        repo: Arc<dyn SymbolRepository>,
        reader: Arc<dyn SourceReader>,
        root_path: impl Into<PathBuf>,
    ) -> Self {
        Self::with_all(repo, reader, root_path, None, None)
    }

    /// Build a service with an optional FTS5 / fuzzy search backend.
    pub fn with_search(
        repo: Arc<dyn SymbolRepository>,
        reader: Arc<dyn SourceReader>,
        root_path: impl Into<PathBuf>,
        search: Option<Arc<dyn SearchRepository>>,
    ) -> Self {
        Self::with_all(repo, reader, root_path, search, None)
    }

    /// Build a service with an optional quality backend but no FTS5
    /// backend. Mirrors `with_search` for the quality case.
    pub fn with_quality(
        repo: Arc<dyn SymbolRepository>,
        reader: Arc<dyn SourceReader>,
        root_path: impl Into<PathBuf>,
        quality: Option<Arc<dyn QualityRepository>>,
    ) -> Self {
        Self::with_all(repo, reader, root_path, None, quality)
    }

    /// Most general constructor — both backends are optional. New
    /// binaries and tests should prefer the narrower constructors so
    /// the intent is explicit; this is the convergence point that
    /// every other constructor delegates to.
    pub fn with_all(
        repo: Arc<dyn SymbolRepository>,
        reader: Arc<dyn SourceReader>,
        root_path: impl Into<PathBuf>,
        search: Option<Arc<dyn SearchRepository>>,
        quality: Option<Arc<dyn QualityRepository>>,
    ) -> Self {
        Self {
            repo,
            reader,
            root_path: root_path.into(),
            search,
            quality,
            lens_registry: Arc::new(default_registry()),
            paths: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(feature = "postgres")]
            postgres_repo: None,
            #[cfg(feature = "multimodal")]
            graph_repo: None,
        }
    }

    /// Wire an `Arc<dyn GraphRepository>` into the service so ExplorerQL
    /// `FIND decisions/docs` and `graph_search` reach the Generic Graph
    /// Layer. Only available behind the `multimodal` feature.
    #[cfg(feature = "multimodal")]
    pub fn with_graph_repo(mut self, gr: Arc<dyn crate::ports::GraphRepository>) -> Self {
        self.graph_repo = Some(gr);
        self
    }

    /// Wire an `Arc<PostgresRepository>` into the service so the
    /// `*_view` MCP tools reach PG. Only available behind the
    /// `postgres` feature; default builds keep `postgres_repo: None`
    /// and every `*_view` call short-circuits to
    /// `FeatureDisabled`.
    #[cfg(feature = "postgres")]
    pub fn with_postgres_repo(mut self, pg_repo: Arc<PostgresRepository>) -> Self {
        self.postgres_repo = Some(pg_repo);
        self
    }

    pub fn root_path(&self) -> &std::path::Path {
        &self.root_path
    }

    /// Borrow the optional quality backend, if one is wired.
    #[allow(dead_code)]
    pub fn quality(&self) -> Option<&dyn QualityRepository> {
        self.quality.as_deref()
    }

    /// Borrow the symbol repository. Used by the MoldQL executor.
    #[allow(dead_code)]
    pub fn symbol_repo(&self) -> &dyn SymbolRepository {
        self.repo.as_ref()
    }

    /// Borrow the source reader. Used by the MoldQL executor.
    #[allow(dead_code)]
    pub fn source_reader(&self) -> &dyn SourceReader {
        self.reader.as_ref()
    }

    /// Execute a MoldQL query against this service. Parses the query,
    /// then runs the executor against the existing ports.
    #[allow(clippy::type_complexity)]
    pub fn execute_query(&self, query: &str) -> crate::ExplorerResult<MoldQLResult> {
        let ast = crate::moldql::parser::parse(query)
            .map_err(|e| ExplorerError::ResolutionFailed(e.to_string()))?;
        let apply_lens: std::sync::Arc<
            dyn Fn(&str, &str) -> crate::ExplorerResult<crate::dto::LensResult> + Send + Sync,
        > = {
            // The closure captures `self` via interior Arc; build an Arc
            // closure by holding an Arc to self.
            let svc = self.clone_service_handle();
            std::sync::Arc::new(move |mvp, lens_id| svc.apply_lens(mvp, lens_id))
        };
        let view = MoldQLView {
            repo: self.repo.clone(),
            quality: self.quality.clone(),
            reader: self.reader.clone(),
            apply_lens,
            #[cfg(feature = "multimodal")]
            graph_repo: self.graph_repo.clone(),
        };
        MoldQLExecutor::new(&view).execute(ast)
    }

    /// Compile + execute a query against a specific `CompileTarget`.
    /// Used by `explorer_query_moldql` when the caller passes
    /// `target: "pg" | "petgraph"`.
    pub fn execute_query_with_target(
        &self,
        query: &str,
        target: crate::moldql::compile::CompileTarget,
    ) -> crate::ExplorerResult<MoldQLResult> {
        let ast = crate::moldql::parser::parse(query)
            .map_err(|e| ExplorerError::ResolutionFailed(e.to_string()))?;
        let apply_lens: std::sync::Arc<
            dyn Fn(&str, &str) -> crate::ExplorerResult<crate::dto::LensResult> + Send + Sync,
        > = {
            let svc = self.clone_service_handle();
            std::sync::Arc::new(move |mvp, lens_id| svc.apply_lens(mvp, lens_id))
        };
        let view = MoldQLView {
            repo: self.repo.clone(),
            quality: self.quality.clone(),
            reader: self.reader.clone(),
            apply_lens,
            #[cfg(feature = "multimodal")]
            graph_repo: self.graph_repo.clone(),
        };
        MoldQLExecutor::new(&view).execute_with_target(ast, target)
    }

    /// Cheap clone of the service into a fresh `Arc<ExplorerService>`.
    /// Used by `execute_query` to capture `self` inside the apply_lens
    /// closure.
    fn clone_service_handle(&self) -> std::sync::Arc<ExplorerService> {
        // ExplorerService is held as a value inside its own constructors
        // (it's never shared via Arc from outside), so we wrap a
        // clone in an Arc here. This costs a single heap allocation
        // per MoldQL query — acceptable for the query throughput
        // we expect.
        std::sync::Arc::new(ExplorerService {
            repo: self.repo.clone(),
            reader: self.reader.clone(),
            root_path: self.root_path.clone(),
            search: self.search.clone(),
            quality: self.quality.clone(),
            lens_registry: self.lens_registry.clone(),
            paths: self.paths.clone(),
            #[cfg(feature = "postgres")]
            postgres_repo: self.postgres_repo.clone(),
            #[cfg(feature = "multimodal")]
            graph_repo: self.graph_repo.clone(),
        })
    }

    pub fn open_workspace(
        &self,
        request: OpenWorkspaceRequest,
    ) -> ExplorerResult<WorkspaceSummary> {
        let root_path = PathBuf::from(&request.root_path);
        if !root_path.exists() {
            return Err(ExplorerError::WorkspaceNotFound(request.root_path));
        }

        let db_path = root_path.join(".cognicode/cognicode.db");
        let graph_status = if db_path.exists() {
            GraphStatus::Ready
        } else {
            GraphStatus::Missing
        };

        // Spec Req 4: only populate real stats when the graph is ready
        // (db_path present). Otherwise leave both counts at zero — that is
        // the contract callers rely on for "graph not loaded yet".
        let (symbol_count, relation_count) = if db_path.exists() {
            let stats = self.repo.graph_stats();
            (stats.symbol_count, stats.relation_count)
        } else {
            (0, 0)
        };

        Ok(WorkspaceSummary {
            id: workspace_id(&root_path),
            root_path: root_path.display().to_string(),
            graph_status,
            indexed_at: None,
            symbol_count,
            relation_count,
        })
    }

    pub fn current_workspace(&self) -> ExplorerResult<WorkspaceSummary> {
        self.open_workspace(OpenWorkspaceRequest {
            root_path: self.root_path.display().to_string(),
        })
    }

    /// Spotter search: query exact matches and (optionally) the FTS5
    /// backend, merge, deduplicate, filter, sort, cap.
    pub fn spotter_search(
        &self,
        query: &str,
        kind: Option<&str>,
    ) -> ExplorerResult<Vec<SpotterResult>> {
        if query.is_empty() {
            return Ok(Vec::new());
        }

        // 1) Exact matches always come from the symbol repository.
        let exact_matches = self.repo.find_symbols_by_name(query)?;

        // 2) FTS5 / fuzzy matches, only when a backend is wired.
        let fts5_matches: Vec<SearchHit> = match &self.search {
            Some(search) => search.search(query, SPOTTER_RESULT_LIMIT)?,
            None => Vec::new(),
        };

        // 3) Build a unified hit stream, then resolve FTS5 hits against the
        //    symbol repository so each hit has a line number + mvp_id.
        let mut hits: Vec<SearchHit> = Vec::with_capacity(exact_matches.len() + fts5_matches.len());

        for resolved in exact_matches {
            hits.push(SearchHit::resolved(
                resolved.name.clone(),
                resolved.kind.name(),
                resolved.file.clone(),
                resolved.line,
                1.0,
                "exact",
            ));
        }

        for fts5_hit in fts5_matches {
            // FTS5 stores only name + file; resolve line via the symbol
            // repository so the MVP id is meaningful. If the symbol is no
            // longer in the graph (e.g. file removed since indexing) we
            // silently drop the hit.
            let resolved = self
                .repo
                .find_symbols_by_name(&fts5_hit.name)?
                .into_iter()
                .find(|s| s.file == fts5_hit.file);
            match resolved {
                Some(sym) => {
                    hits.push(SearchHit::resolved(
                        sym.name.clone(),
                        sym.kind.name(),
                        sym.file.clone(),
                        sym.line,
                        fts5_hit.score,
                        "fts5",
                    ));
                }
                None => continue,
            }
        }

        // 4) Deduplicate by MVP id, keeping the higher score (exact > fts5).
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.dedup_by(|a, b| a.mvp_id == b.mvp_id);

        // 5) Kind filter, then cap.
        let filtered: Vec<SearchHit> = hits
            .into_iter()
            .filter(|h| match kind {
                Some(k) => h.kind.eq_ignore_ascii_case(k),
                None => true,
            })
            .take(SPOTTER_RESULT_LIMIT)
            .collect();

        // 6) Materialise SpotterResult. The MVP id has already been
        //    constructed by `SearchHit::resolved`, so we trust it directly
        //    — no second repository lookup is needed.
        Ok(filtered
            .into_iter()
            .map(|hit| {
                let subtitle = format!("{} at {}:{}", hit.kind, hit.file, hit.line);
                SpotterResult {
                    object: InspectableObjectSummary {
                        id: hit.mvp_id,
                        object_type: InspectableObjectType::Symbol,
                        label: hit.name,
                        subtitle,
                        properties: Vec::new(), // properties fetched lazily in inspect_object
                        available_views: symbol_descriptor_list(),
                    },
                    score: hit.score,
                    match_type: hit.match_type,
                }
            })
            .collect())
    }

    /// Parse an MVP id, resolve the object through the repository, and build
    /// a summary with the available views for its kind.
    pub fn inspect_object(&self, object_id: &str) -> ExplorerResult<InspectableObjectSummary> {
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        match &identity {
            ObjectIdentity::Symbol { .. } => self.inspect_symbol(&identity),
            ObjectIdentity::File { path } => self.inspect_file(&identity, path),
            ObjectIdentity::Scope { path } => self.inspect_scope(&identity, path),
            ObjectIdentity::QualityIssue { id } => self.inspect_quality_issue(&identity, *id),
            ObjectIdentity::Rule { rule_id } => self.inspect_rule(&identity, rule_id),
        }
    }

    fn inspect_symbol(
        &self,
        identity: &ObjectIdentity,
    ) -> ExplorerResult<InspectableObjectSummary> {
        let symbol_id = identity
            .to_symbol_id()
            .expect("Symbol identity always yields a SymbolId");
        let resolved = self
            .repo
            .resolve(&symbol_id)?
            .ok_or_else(|| ExplorerError::ObjectNotFound(identity.to_mvp_id()))?;

        let properties = build_summary_properties(&resolved, self.repo.as_ref());

        Ok(InspectableObjectSummary {
            id: resolved_to_mvp(&resolved),
            object_type: InspectableObjectType::Symbol,
            label: resolved.name.clone(),
            subtitle: format!(
                "{} at {}:{}",
                resolved.kind.name(),
                resolved.file,
                resolved.line
            ),
            properties,
            available_views: symbol_descriptor_list(),
        })
    }

    fn inspect_file(
        &self,
        identity: &ObjectIdentity,
        path: &str,
    ) -> ExplorerResult<InspectableObjectSummary> {
        let symbols = self.repo.find_symbols_by_file(path)?;
        let line_count = self
            .reader
            .read_lines(path, 1, u32::MAX)
            .map(|lines| lines.iter().map(|(n, _)| *n).max().unwrap_or(0) as usize)
            .unwrap_or(0);

        let kinds = count_kinds(&symbols);
        let mut properties = vec![
            Property {
                key: "path".into(),
                value: serde_json::Value::String(path.to_string()),
                value_type: "string".into(),
                source: "ObjectIdentity".into(),
            },
            Property {
                key: "line_count".into(),
                value: serde_json::Value::Number(line_count.into()),
                value_type: "usize".into(),
                source: "SourceReader".into(),
            },
            Property {
                key: "symbol_count".into(),
                value: serde_json::Value::Number(symbols.len().into()),
                value_type: "usize".into(),
                source: "CallGraph".into(),
            },
            Property {
                key: "kinds".into(),
                value: serde_json::to_value(&kinds).unwrap_or(serde_json::json!({})),
                value_type: "map".into(),
                source: "CallGraph".into(),
            },
        ];
        // Keep the property order stable for tests / golden files.
        properties.sort_by(|a, b| a.key.cmp(&b.key));

        Ok(InspectableObjectSummary {
            id: identity.to_mvp_id(),
            object_type: InspectableObjectType::File,
            label: path.to_string(),
            subtitle: if symbols.is_empty() {
                format!("{line_count} lines, no symbols")
            } else {
                format!("{line_count} lines, {} symbol(s)", symbols.len())
            },
            properties,
            available_views: file_descriptor_list(),
        })
    }

    fn inspect_scope(
        &self,
        identity: &ObjectIdentity,
        path: &str,
    ) -> ExplorerResult<InspectableObjectSummary> {
        let (member_files, member_symbols) = derive_scope_members(self.repo.as_ref(), path);
        let kinds = count_kinds(&member_symbols);

        let mut properties = vec![
            Property {
                key: "file_count".into(),
                value: serde_json::Value::Number(member_files.len().into()),
                value_type: "usize".into(),
                source: "CallGraph".into(),
            },
            Property {
                key: "kinds".into(),
                value: serde_json::to_value(&kinds).unwrap_or(serde_json::json!({})),
                value_type: "map".into(),
                source: "CallGraph".into(),
            },
            Property {
                key: "path".into(),
                value: serde_json::Value::String(path.to_string()),
                value_type: "string".into(),
                source: "ObjectIdentity".into(),
            },
            Property {
                key: "promotion_ready".into(),
                value: serde_json::Value::Bool(false),
                value_type: "bool".into(),
                source: "ModuleCandidate".into(),
            },
            Property {
                key: "symbol_count".into(),
                value: serde_json::Value::Number(member_symbols.len().into()),
                value_type: "usize".into(),
                source: "CallGraph".into(),
            },
        ];
        properties.sort_by(|a, b| a.key.cmp(&b.key));

        let subtitle = if member_files.is_empty() {
            "Empty module candidate".to_string()
        } else {
            format!(
                "Module candidate ({} file(s), {} symbol(s))",
                member_files.len(),
                member_symbols.len()
            )
        };

        Ok(InspectableObjectSummary {
            id: identity.to_mvp_id(),
            object_type: InspectableObjectType::Scope,
            label: path.to_string(),
            subtitle,
            properties,
            available_views: scope_descriptor_list(),
        })
    }

    pub fn available_views(&self, object_id: &str) -> ExplorerResult<Vec<ViewDescriptor>> {
        // We parse to validate the shape, then dispatch on the variant tag.
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        Ok(match identity {
            ObjectIdentity::Symbol { .. } => symbol_descriptor_list(),
            ObjectIdentity::File { .. } => file_descriptor_list(),
            ObjectIdentity::Scope { .. } => scope_descriptor_list(),
            ObjectIdentity::QualityIssue { .. } => issue_descriptor_list(),
            ObjectIdentity::Rule { .. } => rule_descriptor_list(),
        })
    }

    /// List the design lenses that apply to the given object. The list is
    /// filtered by each lens's `applicable_types` declaration, so an issue
    /// object will return `[]` (no lens is meaningful there).
    pub fn available_lenses(&self, object_id: &str) -> ExplorerResult<Vec<LensDescriptor>> {
        // Parse to validate the shape (and to get the variant tag).
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        let object_type = identity_to_type(&identity);
        Ok(self.lens_registry.applicable_to(&object_type))
    }

    /// Apply a registered lens to an inspectable object. The lens runs
    /// against the existing ports; when the quality backend is absent
    /// the lens degrades gracefully (lower confidence, fewer findings).
    pub fn apply_lens(&self, object_id: &str, lens_id: &str) -> ExplorerResult<LensResult> {
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        let lens = self
            .lens_registry
            .get(lens_id)
            .ok_or_else(|| ExplorerError::ResolutionFailed(format!("lens not found: {lens_id}")))?;
        let ctx = LensContext::new(
            identity,
            self.repo.clone(),
            self.quality.clone(),
            self.reader.clone(),
        );
        lens.apply(&ctx)
    }

    /// Borrow the lens registry. Tests use this to swap in mock lenses
    /// — production code uses `available_lenses` and `apply_lens` and
    /// does not need direct access.
    #[allow(dead_code)]
    pub fn lens_registry(&self) -> &LensRegistry {
        &self.lens_registry
    }

    // ===========================================================
    // Named Views CRUD delegation
    // ===========================================================
    //
    // All four methods are reachable from MCP dispatch. The
    // feature-gate is enforced at the service boundary: when
    // `postgres_repo` is `None` (default build OR no PG wired),
    // every call returns `ExplorerError::FeatureDisabled("...")`
    // — the MCP layer maps that to the canonical
    // `named_views_require_postgres_feature` envelope.

    /// Save a named view. Generates a UUID string id server-side,
    /// inserts the row, and returns the persisted
    /// [`NamedView`].
    ///
    /// Validation:
    /// - `workspace_id`, `owner`, `name`, `level`, `lens`,
    ///   `focus_node` must be non-empty
    /// - `name` ≤ 200 chars
    /// - `description` ≤ 2000 chars (when present)
    /// - `max_depth >= 0`
    ///
    /// Errors:
    /// - `ExplorerError::InvalidInput` for any validation failure
    /// - `ExplorerError::FeatureDisabled` when no PG is wired
    /// - `ExplorerError::Conflict` on a unique-violation (PG 23505)
    /// - `ExplorerError::Storage` for any other DB failure
    #[cfg(feature = "postgres")]
    pub async fn save_view(
        &self,
        workspace_id: &str,
        owner: &str,
        name: &str,
        description: Option<&str>,
        level: &str,
        lens: &str,
        focus_node: &str,
        max_depth: i32,
    ) -> ExplorerResult<NamedView> {
        // Validate up-front so invalid input never touches PG.
        Self::validate_view_inputs(
            workspace_id,
            owner,
            name,
            description,
            level,
            lens,
            focus_node,
            max_depth,
        )?;
        // Feature-gate after validation: validation errors surface
        // even on a no-PG build (the spec contract for
        // `view_save_rejects_empty_name` and
        // `view_save_rejects_negative_max_depth`).
        self.require_postgres_repo("save_view")?;
        let id = uuid_v4_string();
        let repo = self
            .postgres_repo
            .as_ref()
            .expect("feature gate verified above");
        repo.save_named_view(
            &id,
            workspace_id,
            owner,
            name,
            description,
            level,
            lens,
            focus_node,
            max_depth,
        )
        .await
        .map_err(|e| match e {
            cognicode_core::domain::traits::RepositoryError::UniqueViolation(msg) => {
                ExplorerError::Conflict(msg)
            }
            other => ExplorerError::Anyhow(anyhow::anyhow!("save_view: {other}")),
        })?;
        // Re-fetch the row so the caller gets a fully-populated
        // `created_at` straight from PG (the DEFAULT now() value
        // is server-assigned).
        let row = repo
            .load_named_view(&id, workspace_id, owner)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("save_view reload: {e}")))?
            .ok_or_else(|| {
                ExplorerError::Anyhow(anyhow::anyhow!(
                    "save_view: row vanished after insert (id={id})"
                ))
            })?;
        Ok(named_view_from_row(row))
    }

    /// Load a named view by id, scoped to the caller-supplied
    /// `(workspace_id, owner)`. Returns the rebuilt
    /// [`ContextualView`] (re-invoking the existing
    /// `contextual_view` pipeline) so the projection reflects the
    /// current graph state — not a stale snapshot.
    ///
    /// Errors:
    /// - `ExplorerError::NotFound` for unknown id OR scope mismatch
    /// - `ExplorerError::FeatureDisabled` when no PG is wired
    /// - `ExplorerError::Storage` for DB failures
    #[cfg(feature = "postgres")]
    pub async fn load_view(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> ExplorerResult<ContextualView> {
        self.require_postgres_repo("load_view")?;
        let repo = self
            .postgres_repo
            .as_ref()
            .expect("feature gate verified above");
        let row = repo
            .load_named_view(id, workspace_id, owner)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("load_view: {e}")))?
            .ok_or_else(|| ExplorerError::NotFound(format!("named_view: {id}")))?;
        // Rebuild the live ContextualView from the saved
        // (level, lens, focus_node) tuple. The rebuild uses the
        // existing `contextual_view(focus_node, lens)` path so
        // every lens-specific transformation (callgraph,
        // overview, ...) is applied.
        let mvp_id = view_focus_mvp_id(&row.level, &row.focus_node);
        self.contextual_view(&mvp_id, lens_to_view_id(&row.lens))
    }

    /// List named views for a `(workspace_id, owner)` scope,
    /// newest-first. Returns descriptors with the `description`
    /// field truncated to ≤ 201 chars (200 + `…`) when the
    /// stored text is longer.
    #[cfg(feature = "postgres")]
    pub async fn list_views(
        &self,
        workspace_id: &str,
        owner: &str,
    ) -> ExplorerResult<Vec<NamedViewDescriptor>> {
        self.require_postgres_repo("list_views")?;
        if workspace_id.is_empty() {
            return Err(ExplorerError::InvalidInput(
                "workspace_id is required".into(),
            ));
        }
        if owner.is_empty() {
            return Err(ExplorerError::InvalidInput("owner is required".into()));
        }
        let repo = self
            .postgres_repo
            .as_ref()
            .expect("feature gate verified above");
        let rows = repo
            .list_named_views(workspace_id, owner)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("list_views: {e}")))?;
        Ok(rows
            .into_iter()
            .map(|r| NamedViewDescriptor {
                id: r.id,
                workspace_id: r.workspace_id,
                owner: r.owner,
                name: r.name,
                description: truncate_description(r.description, 200),
                level: r.level,
                lens: r.lens,
                focus_node: r.focus_node,
                max_depth: r.max_depth,
                created_at: r.created_at,
            })
            .collect())
    }

    /// Delete a named view. Returns `true` iff a row was removed.
    /// Unknown id and scope mismatch both return `Ok(false)` —
    /// the dispatch layer surfaces that as a `not_found` error.
    #[cfg(feature = "postgres")]
    pub async fn delete_view(
        &self,
        id: &str,
        workspace_id: &str,
        owner: &str,
    ) -> ExplorerResult<bool> {
        self.require_postgres_repo("delete_view")?;
        if id.is_empty() {
            return Err(ExplorerError::InvalidInput("id is required".into()));
        }
        if workspace_id.is_empty() {
            return Err(ExplorerError::InvalidInput(
                "workspace_id is required".into(),
            ));
        }
        if owner.is_empty() {
            return Err(ExplorerError::InvalidInput("owner is required".into()));
        }
        let repo = self
            .postgres_repo
            .as_ref()
            .expect("feature gate verified above");
        let removed = repo
            .delete_named_view(id, workspace_id, owner)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("delete_view: {e}")))?;
        Ok(removed)
    }

    /// Internal: feature-gate guard. Returns `Ok(())` when the
    /// `postgres` feature is active AND a `PostgresRepository` was
    /// wired at construction; `Err(FeatureDisabled)` otherwise.
    /// Call sites convert the `()` to the concrete
    /// `&Arc<PostgresRepository>` via the `#[cfg(feature =
    /// "postgres")]` block at the call site.
    #[cfg(feature = "postgres")]
    fn require_postgres_repo(&self, op: &str) -> ExplorerResult<()> {
        if self.postgres_repo.is_some() {
            Ok(())
        } else {
            Err(ExplorerError::FeatureDisabled(format!(
                "named_views require postgres feature (op={op})"
            )))
        }
    }

    /// Internal: always-returns-FeatureDisabled stub for default
    /// (no `--features postgres`) builds. The `#[cfg]` mirror of
    /// `require_postgres_repo` so the call sites compile in both
    /// modes.
    #[cfg(not(feature = "postgres"))]
    #[allow(dead_code)]
    fn require_postgres_repo(&self, _op: &str) -> ExplorerResult<()> {
        Err(ExplorerError::FeatureDisabled(
            "named_views require postgres feature".into(),
        ))
    }

    // -- `not(feature = "postgres")` mirrors of the 4 PG-only methods ---
    // The real implementations live above under `#[cfg(feature = "postgres")]`.
    // These stubs keep the call sites in `mcp.rs` compiling on default builds.

    #[cfg(not(feature = "postgres"))]
    #[allow(dead_code)]
    pub async fn save_view(
        &self,
        workspace_id: &str,
        owner: &str,
        name: &str,
        description: Option<&str>,
        level: &str,
        lens: &str,
        focus_node: &str,
        max_depth: i32,
    ) -> ExplorerResult<crate::dto::NamedView> {
        // Validate BEFORE the feature-gate error so that
        // `view_save_rejects_empty_name` /
        // `view_save_rejects_negative_max_depth` can be asserted
        // on a default build (the dispatch tests run without
        // `--features postgres`).
        Self::validate_view_inputs(
            workspace_id,
            owner,
            name,
            description,
            level,
            lens,
            focus_node,
            max_depth,
        )?;
        Err(ExplorerError::FeatureDisabled(
            "named_views require postgres feature".into(),
        ))
    }

    #[cfg(not(feature = "postgres"))]
    #[allow(dead_code)]
    pub async fn load_view(
        &self,
        _id: &str,
        _workspace_id: &str,
        _owner: &str,
    ) -> ExplorerResult<crate::dto::ContextualView> {
        Err(ExplorerError::FeatureDisabled(
            "named_views require postgres feature".into(),
        ))
    }

    #[cfg(not(feature = "postgres"))]
    #[allow(dead_code)]
    pub async fn list_views(
        &self,
        _workspace_id: &str,
        _owner: &str,
    ) -> ExplorerResult<Vec<crate::dto::NamedViewDescriptor>> {
        Err(ExplorerError::FeatureDisabled(
            "named_views require postgres feature".into(),
        ))
    }

    #[cfg(not(feature = "postgres"))]
    #[allow(dead_code)]
    pub async fn delete_view(
        &self,
        _id: &str,
        _workspace_id: &str,
        _owner: &str,
    ) -> ExplorerResult<bool> {
        Err(ExplorerError::FeatureDisabled(
            "named_views require postgres feature".into(),
        ))
    }

    /// Internal: validate every input the spec demands be
    /// non-empty / length-bounded / non-negative. All checks
    /// happen BEFORE the PG call, so an invalid input never
    /// produces a row.
    fn validate_view_inputs(
        workspace_id: &str,
        owner: &str,
        name: &str,
        description: Option<&str>,
        level: &str,
        lens: &str,
        focus_node: &str,
        max_depth: i32,
    ) -> ExplorerResult<()> {
        if workspace_id.is_empty() {
            return Err(ExplorerError::InvalidInput(
                "workspace_id is required".into(),
            ));
        }
        if owner.is_empty() {
            return Err(ExplorerError::InvalidInput("owner is required".into()));
        }
        if name.is_empty() {
            return Err(ExplorerError::InvalidInput("name is required".into()));
        }
        if name.chars().count() > 200 {
            return Err(ExplorerError::InvalidInput(
                "name must be at most 200 characters".into(),
            ));
        }
        if level.is_empty() {
            return Err(ExplorerError::InvalidInput("level is required".into()));
        }
        if lens.is_empty() {
            return Err(ExplorerError::InvalidInput("lens is required".into()));
        }
        if focus_node.is_empty() {
            return Err(ExplorerError::InvalidInput("focus_node is required".into()));
        }
        if max_depth < 0 {
            return Err(ExplorerError::InvalidInput("max_depth must be >= 0".into()));
        }
        if let Some(d) = description {
            if d.chars().count() > 2000 {
                return Err(ExplorerError::InvalidInput(
                    "description must be at most 2000 characters".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn contextual_view(
        &self,
        object_id: &str,
        view_id: &str,
    ) -> ExplorerResult<ContextualView> {
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        match &identity {
            ObjectIdentity::Symbol { .. } => self.contextual_view_symbol(&identity, view_id),
            ObjectIdentity::File { path } => self.contextual_view_file(&identity, path, view_id),
            ObjectIdentity::Scope { path } => self.contextual_view_scope(&identity, path, view_id),
            ObjectIdentity::QualityIssue { id } => {
                self.contextual_view_issue(&identity, *id, view_id)
            }
            ObjectIdentity::Rule { rule_id } => {
                self.contextual_view_rule(&identity, rule_id, view_id)
            }
        }
    }

    fn contextual_view_symbol(
        &self,
        identity: &ObjectIdentity,
        view_id: &str,
    ) -> ExplorerResult<ContextualView> {
        let symbol_id = identity
            .to_symbol_id()
            .expect("Symbol identity always yields a SymbolId");
        let resolved = self
            .repo
            .resolve(&symbol_id)?
            .ok_or_else(|| ExplorerError::ObjectNotFound(identity.to_mvp_id()))?;

        match view_id {
            "overview" => Ok(views::build_overview(&resolved, self.repo.as_ref())),
            "call-graph" => Ok(views::build_callgraph(&resolved, self.repo.as_ref())),
            "source" => Ok(views::build_source(&resolved, self.reader.as_ref())),
            "evidence" => Ok(build_evidence_view(
                &resolved,
                self.repo.as_ref(),
                self.reader.as_ref(),
            )),
            "quality" => Ok(views::build_symbol_quality_view(&resolved, self.quality())),
            other => Err(ExplorerError::ViewNotAvailable {
                object_id: identity.to_mvp_id(),
                view_id: other.to_string(),
            }),
        }
    }

    fn contextual_view_file(
        &self,
        identity: &ObjectIdentity,
        path: &str,
        view_id: &str,
    ) -> ExplorerResult<ContextualView> {
        let symbols = self.repo.find_symbols_by_file(path)?;
        match view_id {
            "overview" => Ok(views::build_file_overview(
                &symbols,
                path,
                self.reader.as_ref(),
            )),
            "symbols" => Ok(views::build_file_symbols(&symbols, path)),
            "quality" => Ok(views::build_file_quality_view(path, self.quality())),
            other => Err(ExplorerError::ViewNotAvailable {
                object_id: identity.to_mvp_id(),
                view_id: other.to_string(),
            }),
        }
    }

    fn contextual_view_scope(
        &self,
        identity: &ObjectIdentity,
        path: &str,
        view_id: &str,
    ) -> ExplorerResult<ContextualView> {
        let (member_files, member_symbols) = derive_scope_members(self.repo.as_ref(), path);
        match view_id {
            "overview" => Ok(views::build_scope_overview(
                path,
                &member_files,
                &member_symbols,
            )),
            "dependencies" => Ok(views::build_scope_dependencies(path, self.repo.as_ref())),
            "hotspots" => {
                let hotspots =
                    top_hotspots(&member_symbols, self.repo.as_ref(), SCOPE_HOTSPOT_LIMIT);
                Ok(views::build_scope_hotspots(path, &hotspots))
            }
            "quality" => Ok(views::build_scope_quality_view(path, self.quality())),
            other => Err(ExplorerError::ViewNotAvailable {
                object_id: identity.to_mvp_id(),
                view_id: other.to_string(),
            }),
        }
    }

    // -----------------------------------------------------------------------
    // Contextual Graph — visualization-stack Phase 2 (Contextual Views)
    // -----------------------------------------------------------------------
    //
    // Returns a `ContextualGraphResponse` composed of the focus node,
    // the file-level parent + children, and a BFS of same-level call
    // neighbours (callers + callees up to `depth` hops, bounded by
    // `max_nodes`).
    //
    // Validation:
    // - `level` MUST be "file" in Phase 1 (returns `InvalidQuery` otherwise)
    // - `depth` MUST be in 1..=2 (returns `InvalidQuery` otherwise)
    // - `max_nodes` MUST be in 50..=500 (returns `InvalidQuery` otherwise)
    //
    // Returns `SymbolNotFound` when the focus id is not in the repository.
    pub fn build_contextual_graph(
        &self,
        focus_id: &SymbolId,
        level: &str,
        depth: u8,
        max_nodes: usize,
    ) -> ExplorerResult<ContextualGraphResponse> {
        // 1) Validate the request.
        if level != "file" {
            return Err(ExplorerError::InvalidQuery(format!(
                "level must be 'file' in Phase 1 (got: {level})"
            )));
        }
        if !(1..=2).contains(&depth) {
            return Err(ExplorerError::InvalidQuery(format!(
                "depth must be in 1..=2 (got: {depth})"
            )));
        }
        if !(50..=500).contains(&max_nodes) {
            return Err(ExplorerError::InvalidQuery(format!(
                "max_nodes must be in 50..=500 (got: {max_nodes})"
            )));
        }

        // 2) Resolve the focus symbol.
        let focus_resolved = self
            .repo
            .resolve(focus_id)?
            .ok_or_else(|| ExplorerError::SymbolNotFound(focus_id.to_string()))?;
        let focus_node = symbol_to_node(&focus_resolved);

        // 3) Build the parent + children section (file-level projection).
        let file_siblings = self.repo.find_symbols_by_file(&focus_resolved.file)?;
        let (parent, children, children_clipped) = if file_siblings.is_empty() {
            // Orphan: cannot derive a parent file. Both sections are null.
            (None, None, false)
        } else {
            let parent_node = GraphNode {
                id: format!("file:{}", focus_resolved.file),
                label: focus_resolved.file.clone(),
                kind: "file".to_string(),
                file: Some(focus_resolved.file.clone()),
                line: None,
                style_class: "module".to_string(),
            };
            let parent_edge = GraphEdge {
                source: focus_resolved.id.to_string(),
                target: parent_node.id.clone(),
                relation: "lives_in".to_string(),
                style_class: "edge.calls".to_string(),
            };
            let parent_section = ParentSection {
                node: parent_node,
                edge: parent_edge,
            };

            // Children: every sibling EXCEPT the focus itself.
            let mut child_nodes: Vec<GraphNode> = Vec::new();
            let mut child_edges: Vec<GraphEdge> = Vec::new();
            for sib in file_siblings.iter().filter(|s| s.id != focus_resolved.id) {
                child_edges.push(GraphEdge {
                    source: sib.id.to_string(),
                    target: focus_resolved.id.to_string(),
                    relation: "lives_in".to_string(),
                    style_class: "edge.calls".to_string(),
                });
                child_nodes.push(symbol_to_node(sib));
            }

            // Children take priority in the cap budget. A file with
            // more siblings than `max_nodes` is the canonical
            // truncation case.
            let clipped = child_nodes.len() > max_nodes;
            if clipped {
                child_nodes.truncate(max_nodes);
                let kept: std::collections::HashSet<String> =
                    child_nodes.iter().map(|n| n.id.clone()).collect();
                child_edges.retain(|e| kept.contains(&e.source));
            }
            (
                Some(parent_section),
                Some(ChildrenSection {
                    nodes: child_nodes,
                    edges: child_edges,
                }),
                clipped,
            )
        };

        // 4) Build the same-level BFS up to `depth` hops, capped at
        //    the remaining budget (children take priority).
        let remaining_cap = max_nodes.saturating_sub(
            children
                .as_ref()
                .map(|c| c.nodes.len())
                .unwrap_or(0),
        );
        let (same_nodes, same_edges) = if remaining_cap == 0 {
            (Vec::new(), Vec::new())
        } else {
            bfs_same_level(self.repo.as_ref(), focus_id, depth, remaining_cap)
        };

        // 5) Truncation flag. We set `truncated=true` when:
        //    - the children list was clipped to `max_nodes`, OR
        //    - the BFS hit the remaining cap before exhausting
        //      reachable nodes.
        let bfs_clipped = !same_nodes.is_empty() && same_nodes.len() >= remaining_cap
            && (self.repo.fan_in(focus_id) + self.repo.fan_out(focus_id)) > remaining_cap as usize;
        let truncated = children_clipped || bfs_clipped;
        let truncation_reason = if truncated {
            Some("max_nodes_exceeded".to_string())
        } else {
            None
        };

        Ok(ContextualGraphResponse {
            focus_node,
            parent,
            children,
            same_level: SameLevelSection {
                nodes: same_nodes,
                edges: same_edges,
            },
            level: "file".to_string(),
            truncated,
            truncation_reason,
        })
    }

    /// Inspect a single quality issue by id. When the quality backend
    /// is missing or the id is unknown, the summary still renders with
    /// the data we have — the backend's `None` path produces an empty
    /// issue, and unknown ids surface as `IssueNotFound` at the view
    /// layer.
    fn inspect_quality_issue(
        &self,
        identity: &ObjectIdentity,
        id: i64,
    ) -> ExplorerResult<InspectableObjectSummary> {
        let issue = self
            .quality
            .as_deref()
            .and_then(|q| q.issue_by_id(id).ok().flatten());

        let (label, subtitle, properties) = match issue.as_ref() {
            Some(i) => {
                let mut properties = vec![
                    Property {
                        key: "id".into(),
                        value: serde_json::Value::Number(i.id.into()),
                        value_type: "i64".into(),
                        source: "QualityRepository".into(),
                    },
                    Property {
                        key: "rule_id".into(),
                        value: serde_json::Value::String(i.rule_id.clone()),
                        value_type: "string".into(),
                        source: "QualityRepository".into(),
                    },
                    Property {
                        key: "severity".into(),
                        value: serde_json::Value::String(i.severity.clone()),
                        value_type: "string".into(),
                        source: "QualityRepository".into(),
                    },
                    Property {
                        key: "category".into(),
                        value: serde_json::Value::String(i.category.clone()),
                        value_type: "string".into(),
                        source: "QualityRepository".into(),
                    },
                    Property {
                        key: "file".into(),
                        value: serde_json::Value::String(i.file.clone()),
                        value_type: "string".into(),
                        source: "QualityRepository".into(),
                    },
                    Property {
                        key: "line".into(),
                        value: serde_json::Value::Number(i.line.into()),
                        value_type: "u32".into(),
                        source: "QualityRepository".into(),
                    },
                    Property {
                        key: "status".into(),
                        value: serde_json::Value::String(i.status.clone()),
                        value_type: "string".into(),
                        source: "QualityRepository".into(),
                    },
                ];
                properties.sort_by(|a, b| a.key.cmp(&b.key));
                (
                    format!("{}: {}", i.rule_id, i.message),
                    format!("{} at {}:{}", i.severity, i.file, i.line),
                    properties,
                )
            }
            None => (
                format!("Issue #{id}"),
                if self.quality.is_some() {
                    "Issue not found".to_string()
                } else {
                    "No quality backend wired".to_string()
                },
                Vec::new(),
            ),
        };

        Ok(InspectableObjectSummary {
            id: identity.to_mvp_id(),
            object_type: InspectableObjectType::QualityIssue,
            label,
            subtitle,
            properties,
            available_views: issue_descriptor_list(),
        })
    }

    /// Inspect a single quality rule by id. Pulls the open count and
    /// description from the repo; degrades to "no data" labels when
    /// no quality backend is wired.
    fn inspect_rule(
        &self,
        identity: &ObjectIdentity,
        rule_id: &str,
    ) -> ExplorerResult<InspectableObjectSummary> {
        let summary = self
            .quality
            .as_deref()
            .and_then(|q| q.rule_summary(rule_id).ok());

        let (label, subtitle, properties) = match summary.as_ref() {
            Some(s) => {
                let mut properties = vec![
                    Property {
                        key: "description".into(),
                        value: serde_json::Value::String(s.description.clone()),
                        value_type: "string".into(),
                        source: "QualityRepository".into(),
                    },
                    Property {
                        key: "open_count".into(),
                        value: serde_json::Value::Number(s.open_count.into()),
                        value_type: "usize".into(),
                        source: "QualityRepository".into(),
                    },
                    Property {
                        key: "rule_id".into(),
                        value: serde_json::Value::String(s.rule_id.clone()),
                        value_type: "string".into(),
                        source: "QualityRepository".into(),
                    },
                ];
                properties.sort_by(|a, b| a.key.cmp(&b.key));
                (
                    format!("Rule {}", s.rule_id),
                    format!("{} open finding(s)", s.open_count),
                    properties,
                )
            }
            None => (
                format!("Rule {rule_id}"),
                if self.quality.is_some() {
                    "Rule not found".to_string()
                } else {
                    "No quality backend wired".to_string()
                },
                Vec::new(),
            ),
        };

        Ok(InspectableObjectSummary {
            id: identity.to_mvp_id(),
            object_type: InspectableObjectType::Rule,
            label,
            subtitle,
            properties,
            available_views: rule_descriptor_list(),
        })
    }

    /// Contextual view for a single quality issue. Only the "overview"
    /// view is exposed; future additions might add a "diff" or
    /// "history" view.
    fn contextual_view_issue(
        &self,
        identity: &ObjectIdentity,
        id: i64,
        view_id: &str,
    ) -> ExplorerResult<ContextualView> {
        match view_id {
            "overview" => {
                let issue = self
                    .quality
                    .as_deref()
                    .and_then(|q| q.issue_by_id(id).ok().flatten())
                    .ok_or_else(|| ExplorerError::ObjectNotFound(identity.to_mvp_id()))?;
                Ok(views::build_issue_detail(&issue))
            }
            other => Err(ExplorerError::ViewNotAvailable {
                object_id: identity.to_mvp_id(),
                view_id: other.to_string(),
            }),
        }
    }

    /// Contextual view for a single quality rule.
    fn contextual_view_rule(
        &self,
        identity: &ObjectIdentity,
        rule_id: &str,
        view_id: &str,
    ) -> ExplorerResult<ContextualView> {
        match view_id {
            "overview" => Ok(views::build_rule_detail(rule_id, self.quality())),
            other => Err(ExplorerError::ViewNotAvailable {
                object_id: identity.to_mvp_id(),
                view_id: other.to_string(),
            }),
        }
    }

    pub fn save_exploration(
        &self,
        request: SaveExplorationRequest,
    ) -> ExplorerResult<ExplorationPath> {
        if request.columns.is_empty() {
            return Err(ExplorerError::ResolutionFailed(
                "exploration path requires at least one column".to_string(),
            ));
        }

        // Validate every column id is well-formed before we persist anything,
        // and resolve each into an `ObjectIdentityEntry`. Duplicates in
        // `request.columns` collapse to a single object in the path's
        // `objects` vec.
        let created_at = Utc::now().to_rfc3339();
        let mut seen: HashMap<String, ObjectIdentityEntry> = HashMap::new();
        for column in &request.columns {
            let identity = ObjectIdentity::parse_mvp_id(&column.object_id)?;
            let entry = ObjectIdentityEntry {
                id: identity.to_mvp_id(),
                object_type: identity.object_type(),
                natural_key: identity.natural_key(),
                first_seen: created_at.clone(),
            };
            seen.entry(entry.id.clone()).or_insert(entry);
        }
        let objects: Vec<ObjectIdentityEntry> = seen.into_values().collect();

        let path = ExplorationPath {
            id: format!("exploration:{}", Utc::now().timestamp_millis()),
            workspace_id: request.workspace_id,
            columns: request.columns,
            objects,
            lens: request.lens,
            created_at,
        };

        // Store the path so `generate_artifact` can look it up by id.
        self.paths
            .lock()
            .map_err(|_| ExplorerError::Anyhow(anyhow::anyhow!("exploration path store poisoned")))?
            .insert(path.id.clone(), path.clone());

        Ok(path)
    }

    pub fn generate_artifact(
        &self,
        exploration_id: &str,
        request: GenerateArtifactRequest,
    ) -> ExplorerResult<DecisionArtifactSummary> {
        let path = self
            .paths
            .lock()
            .map_err(|_| ExplorerError::Anyhow(anyhow::anyhow!("path store poisoned")))?
            .get(exploration_id)
            .cloned();

        match request.format {
            crate::dto::ArtifactFormat::JsonReplay => {
                let body = match path.as_ref() {
                    Some(p) => render_replay_json(p),
                    None => render_replay_json_unknown(exploration_id),
                };
                Ok(DecisionArtifactSummary {
                    id: format!("artifact:{exploration_id}:json"),
                    format: request.format,
                    title: "Exploration JSON replay".into(),
                    content: body,
                })
            }
            crate::dto::ArtifactFormat::Markdown | crate::dto::ArtifactFormat::Html => {
                let body = match path.as_ref() {
                    Some(p) => render_replay_markdown(p),
                    None => render_replay_markdown_unknown(exploration_id),
                };
                Ok(DecisionArtifactSummary {
                    id: format!("artifact:{exploration_id}:md"),
                    format: request.format,
                    title: "Symbol exploration report".into(),
                    content: body,
                })
            }
        }
    }
}

fn build_evidence_view(
    resolved: &crate::ports::symbol_repository::ResolvedSymbol,
    repo: &dyn SymbolRepository,
    reader: &dyn SourceReader,
) -> ContextualView {
    let evidence = build_evidence_blocks(resolved, repo, reader);
    let blocks = vec![ViewBlock {
        id: "evidence_summary".into(),
        title: "Evidence blocks".into(),
        body: json!({
            "count": evidence.len(),
            "kinds": evidence.iter().map(|b| b.kind.clone()).collect::<Vec<_>>(),
        }),
    }];

    ContextualView {
        object_id: resolved_to_mvp(resolved),
        view_id: "evidence".into(),
        title: "Evidence".into(),
        blocks,
        relations: Vec::new(),
        evidence,
        findings: Vec::new(),
    }
}

fn build_summary_properties(
    resolved: &crate::ports::symbol_repository::ResolvedSymbol,
    repo: &dyn SymbolRepository,
) -> Vec<Property> {
    use serde_json::Value;
    let mut properties = vec![
        Property {
            key: "name".into(),
            value: Value::String(resolved.name.clone()),
            value_type: "string".into(),
            source: "CallGraph".into(),
        },
        Property {
            key: "kind".into(),
            value: Value::String(resolved.kind.name().to_string()),
            value_type: "string".into(),
            source: "CallGraph".into(),
        },
        Property {
            key: "file".into(),
            value: Value::String(resolved.file.clone()),
            value_type: "string".into(),
            source: "CallGraph".into(),
        },
        Property {
            key: "line".into(),
            value: Value::Number(resolved.line.into()),
            value_type: "u32".into(),
            source: "CallGraph".into(),
        },
        Property {
            key: "fan_in".into(),
            value: Value::Number(repo.fan_in(&resolved.id).into()),
            value_type: "usize".into(),
            source: "CallGraph".into(),
        },
        Property {
            key: "fan_out".into(),
            value: Value::Number(repo.fan_out(&resolved.id).into()),
            value_type: "usize".into(),
            source: "CallGraph".into(),
        },
    ];
    if resolved.kind.is_callable() {
        properties.push(Property {
            key: "signature".into(),
            value: Value::String(resolved.signature.clone().unwrap_or_default()),
            value_type: "string".into(),
            source: "CallGraph".into(),
        });
    }
    properties
}

fn resolved_to_mvp(resolved: &crate::ports::symbol_repository::ResolvedSymbol) -> String {
    format!(
        "symbol:{}:{}:{}",
        resolved.file, resolved.name, resolved.line
    )
}

fn symbol_descriptor_list() -> Vec<ViewDescriptor> {
    vec![
        ViewDescriptor {
            id: "overview".into(),
            title: "Overview".into(),
        },
        ViewDescriptor {
            id: "call-graph".into(),
            title: "Call Graph".into(),
        },
        ViewDescriptor {
            id: "source".into(),
            title: "Source".into(),
        },
        ViewDescriptor {
            id: "evidence".into(),
            title: "Evidence".into(),
        },
        ViewDescriptor {
            id: "quality".into(),
            title: "Quality".into(),
        },
    ]
}

fn file_descriptor_list() -> Vec<ViewDescriptor> {
    vec![
        ViewDescriptor {
            id: "overview".into(),
            title: "Overview".into(),
        },
        ViewDescriptor {
            id: "symbols".into(),
            title: "Symbols".into(),
        },
        ViewDescriptor {
            id: "quality".into(),
            title: "Quality".into(),
        },
    ]
}

fn scope_descriptor_list() -> Vec<ViewDescriptor> {
    vec![
        ViewDescriptor {
            id: "overview".into(),
            title: "Overview".into(),
        },
        ViewDescriptor {
            id: "dependencies".into(),
            title: "Dependencies".into(),
        },
        ViewDescriptor {
            id: "hotspots".into(),
            title: "Hotspots".into(),
        },
        ViewDescriptor {
            id: "quality".into(),
            title: "Quality".into(),
        },
    ]
}

fn issue_descriptor_list() -> Vec<ViewDescriptor> {
    vec![ViewDescriptor {
        id: "overview".into(),
        title: "Overview".into(),
    }]
}

fn rule_descriptor_list() -> Vec<ViewDescriptor> {
    vec![ViewDescriptor {
        id: "overview".into(),
        title: "Overview".into(),
    }]
}

/// Collect the unique files and the resolved symbols that belong to `scope_path`.
///
/// Membership uses [`views::scope_contains_file`] so prefixes do not bleed
/// across module-name boundaries (e.g. `scope:src` does not match
/// `src_extra.rs`). Symbols are returned sorted by file then line so the
/// "Member files" block in the scope overview view renders in a stable order.
fn derive_scope_members(
    repo: &dyn SymbolRepository,
    scope_path: &str,
) -> (Vec<String>, Vec<ResolvedSymbol>) {
    let mut files: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut symbols: Vec<ResolvedSymbol> = Vec::new();
    if let Ok(all) = repo.all_symbols() {
        for sym in all {
            if views::scope_contains_file(scope_path, &sym.file) {
                files.insert(sym.file.clone());
                symbols.push(sym);
            }
        }
    }
    symbols.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.line.cmp(&b.line)));
    (files.into_iter().collect(), symbols)
}

/// Top `limit` symbols in the scope by `fan_in`. When fewer than `limit`
/// symbols exist, returns them all. Empty input returns an empty vec.
fn top_hotspots(
    symbols: &[ResolvedSymbol],
    repo: &dyn SymbolRepository,
    limit: usize,
) -> Vec<ResolvedSymbol> {
    let mut sorted: Vec<ResolvedSymbol> = symbols.to_vec();
    sorted.sort_by(|a, b| {
        let fa = repo.fan_in(&a.id);
        let fb = repo.fan_in(&b.id);
        fb.cmp(&fa).then_with(|| a.name.cmp(&b.name))
    });
    sorted.truncate(limit);
    sorted
}

/// Count symbols per kind, returning a stable map (always `String` keys
/// so JSON serialisation does not collapse it to `null`).
fn count_kinds(symbols: &[ResolvedSymbol]) -> std::collections::BTreeMap<String, usize> {
    let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for s in symbols {
        *kinds.entry(s.kind.name().to_string()).or_insert(0) += 1;
    }
    kinds
}

fn render_replay_json(path: &ExplorationPath) -> String {
    let body = json!({
        "exploration_id": path.id,
        "version": 1,
        "objects": path.objects,
    });
    serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string())
}

fn render_replay_json_unknown(exploration_id: &str) -> String {
    let body = json!({
        "exploration_id": exploration_id,
        "version": 1,
        "objects": [],
        "warning": "exploration path not found in session store — no resolved objects available",
    });
    serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string())
}

fn render_replay_markdown(path: &ExplorationPath) -> String {
    let mut out = String::new();
    out.push_str("# Symbol exploration report\n\n");
    out.push_str(&format!("Exploration: `{}`\n\n", path.id));
    out.push_str(&format!("Created: `{}`\n\n", path.created_at));
    out.push_str(&format!("Objects ({}):\n\n", path.objects.len()));
    for obj in &path.objects {
        out.push_str(&format!(
            "- `{}` — type=`{}` natural_key=`{}` first_seen=`{}`\n",
            obj.id, obj.object_type, obj.natural_key, obj.first_seen
        ));
    }
    out
}

fn render_replay_markdown_unknown(exploration_id: &str) -> String {
    format!(
        "# Symbol exploration report\n\nExploration: `{exploration_id}`\n\n_No path data found in session store — the exploration may have been created in another process._\n"
    )
}

/// Convert a `ResolvedSymbol` to the wire-side `GraphNode`. Mirrors
/// the helper in `api.rs` (which is a near-duplicate) — kept local to
/// the service so the contextual graph logic does not depend on the
/// `api` module's public surface.
fn symbol_to_node(s: &ResolvedSymbol) -> GraphNode {
    let kind_label = format!("{:?}", s.kind).to_lowercase();
    GraphNode {
        id: s.id.to_string(),
        label: s.name.clone(),
        kind: kind_label.clone(),
        file: Some(s.file.clone()),
        line: Some(s.line),
        style_class: style_class_for_kind(&kind_label).to_string(),
    }
}

/// Map a kind label to the cytoscape `style_class` bucket. Same
/// taxonomy as the `api` module — kept inline to avoid a public
/// dependency on the API layer from the service.
fn style_class_for_kind(kind: &str) -> &'static str {
    match kind {
        "function" | "method" | "fn" => "function",
        "module" | "crate" | "trait" => "module",
        "external" => "external",
        "file" => "module",
        _ => "function",
    }
}

/// BFS of same-level neighbours (callers + callees) of `start` up to
/// `depth` hops, capped at `cap` collected nodes. The `visited` set
/// prevents cycles in the call graph. The `cap` is inclusive — when
/// adding the next node would exceed `cap`, we stop.
///
/// `start` itself is implicitly in `visited` (we never re-emit it).
fn bfs_same_level(
    repo: &dyn SymbolRepository,
    start: &SymbolId,
    depth: u8,
    cap: usize,
) -> (Vec<GraphNode>, Vec<GraphEdge>) {
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    visited.insert(start.to_string());
    let mut frontier: Vec<SymbolId> = vec![start.clone()];
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    for _ in 0..depth {
        if nodes.len() >= cap {
            break;
        }
        let mut next: Vec<SymbolId> = Vec::new();
        for n in &frontier {
            // Combine callers and callees — same-level set is the
            // union, with edges carrying the relation label.
            for rel in repo.callers(n).into_iter().chain(repo.callees(n)) {
                let nid = rel.id.to_string();
                if !visited.insert(nid.clone()) {
                    continue;
                }
                if nodes.len() >= cap {
                    // Cap hit. Drop the edge too so the response
                    // stays internally consistent (no dangling
                    // references).
                    break;
                }
                let relation = "calls".to_string();
                edges.push(GraphEdge {
                    source: n.to_string(),
                    target: nid.clone(),
                    relation: relation.clone(),
                    style_class: "edge.calls".to_string(),
                });
                // Build the GraphNode by resolving the neighbour.
                // If the repo cannot resolve the id (orphan target),
                // we still record the edge but skip the node — the
                // front-end can still draw the edge by its endpoints.
                if let Ok(Some(resolved)) = repo.resolve(&rel.id) {
                    nodes.push(symbol_to_node(&resolved));
                }
                next.push(rel.id);
            }
            if nodes.len() >= cap {
                break;
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    (nodes, edges)
}

fn workspace_id(path: &std::path::Path) -> String {
    let label = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace");
    format!("workspace:{label}")
}

/// Map an `ObjectIdentity` variant to the public `InspectableObjectType`
/// the lens registry uses for applicability matching. Kept local to
/// the service layer so `ObjectIdentity` stays free of DTO coupling.
fn identity_to_type(identity: &ObjectIdentity) -> InspectableObjectType {
    match identity {
        ObjectIdentity::Symbol { .. } => InspectableObjectType::Symbol,
        ObjectIdentity::File { .. } => InspectableObjectType::File,
        ObjectIdentity::Scope { .. } => InspectableObjectType::Scope,
        ObjectIdentity::QualityIssue { .. } => InspectableObjectType::QualityIssue,
        ObjectIdentity::Rule { .. } => InspectableObjectType::Rule,
    }
}

// ============================================================================
// Named-view helpers
// ============================================================================

/// Convert a `NamedViewRow` (PG layer) into the wire-side
/// `NamedView` DTO. The two structs share every field; this
/// is a single allocation-free rename.
#[cfg(feature = "postgres")]
fn named_view_from_row(
    row: cognicode_core::infrastructure::persistence::NamedViewRow,
) -> NamedView {
    NamedView {
        id: row.id,
        workspace_id: row.workspace_id,
        owner: row.owner,
        name: row.name,
        description: row.description,
        level: row.level,
        lens: row.lens,
        focus_node: row.focus_node,
        max_depth: row.max_depth,
        created_at: row.created_at,
    }
}

/// Build the MVP id used to re-invoke `contextual_view` from a
/// stored `(level, focus_node)` tuple. The MVP id has the shape
/// `"{kind}:{key}"`; for named-view v1 we treat every focus as
/// either a symbol (default) or a file/scope/issue based on the
/// `level` hint.
#[cfg(feature = "postgres")]
fn view_focus_mvp_id(level: &str, focus_node: &str) -> String {
    let prefix = match level {
        "file" => "file",
        "scope" | "module" => "scope",
        "issue" | "quality" => "issue",
        _ => "symbol",
    };
    format!("{prefix}:{focus_node}")
}

/// Translate a stored `lens` value (e.g. `"callgraph"`) into the
/// matching `view_id` accepted by `contextual_view` (e.g.
/// `"call-graph"`). The lens-vs-view distinction is a historical
/// naming difference that pre-dates the v2 schema; the DB stores
/// the user-facing `lens` form for display but the runtime
/// dispatch expects the view builder name.
#[cfg(feature = "postgres")]
fn lens_to_view_id(lens: &str) -> &str {
    match lens {
        "callgraph" => "call-graph",
        "overview" | "call-graph" | "source" | "evidence" | "quality" | "rationale" => lens,
        // Pass through any other value (e.g. a future lens) and
        // let `contextual_view` reject it with a precise
        // `ViewNotAvailable` error.
        other => other,
    }
}

/// Generate a v4-ish UUID string (RFC 4122 form) using the
/// in-process clock + a static atomic counter. We do NOT add
/// the `uuid` crate as a dependency just to mint ids — the
/// spec needs only a stable unique string, not a cryptographic
/// UUID. Format: `"xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx"`.
#[cfg(feature = "postgres")]
fn uuid_v4_string() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mix = now.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(n);
    let bytes: [u8; 16] = bytemuck_like_bytes(mix);
    let mut hex = String::with_capacity(36);
    for (i, b) in bytes.iter().enumerate() {
        if i == 4 || i == 6 || i == 8 || i == 10 {
            hex.push('-');
        }
        hex.push_str(&format!("{b:02x}"));
    }
    // Force the v4 nibble + variant nibble per RFC 4122:
    //   byte[6] = (b & 0x0F) | 0x40
    //   byte[8] = (b & 0x3F) | 0x80
    let mut chars: Vec<char> = hex.chars().collect();
    // Per RFC 4122 §4.4: version nibble at position 14 MUST be '4'.
    chars[14] = '4';
    // Variant bits at position 19 MUST be 10xx → '8', '9', 'a', or 'b'.
    let v = chars[19];
    chars[19] = match v {
        '8' | '9' | 'a' | 'b' => v,
        _ => '8',
    };
    chars.into_iter().collect()
}

/// Deterministic 16-byte expansion of a 64-bit seed.
#[cfg(feature = "postgres")]
fn bytemuck_like_bytes(seed: u64) -> [u8; 16] {
    let lo = seed;
    let hi = seed.wrapping_mul(0xFF51_AFD7_ED55_28CC);
    let mut out = [0u8; 16];
    out[0..8].copy_from_slice(&lo.to_le_bytes());
    out[8..16].copy_from_slice(&hi.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::FsSourceReader;
    use crate::dto::ExplorationColumn;
    use crate::ports::search_repository::SearchHit;
    use crate::ports::symbol_repository::{GraphStats, ResolvedSymbol, SymbolRepository};
    use cognicode_core::domain::aggregates::SymbolId;
    use cognicode_core::domain::value_objects::SymbolKind;
    use std::collections::HashMap as StdHashMap;

    /// Mock symbol repository backed by a map.
    struct MockRepo {
        by_name: StdHashMap<String, Vec<ResolvedSymbol>>,
        by_id: StdHashMap<String, ResolvedSymbol>,
    }

    impl MockRepo {
        fn new() -> Self {
            Self {
                by_name: StdHashMap::new(),
                by_id: StdHashMap::new(),
            }
        }
        fn with_symbol(
            &mut self,
            name: &str,
            file: &str,
            line: u32,
            kind: SymbolKind,
        ) -> &mut Self {
            let id = SymbolId::new(format!("{file}:{name}:{line}"));
            let sym = ResolvedSymbol {
                id: id.clone(),
                name: name.to_string(),
                kind,
                file: file.to_string(),
                line,
                signature: None,
            };
            self.by_id.insert(id.to_string(), sym.clone());
            self.by_name.entry(name.to_string()).or_default().push(sym);
            self
        }
    }

    impl SymbolRepository for MockRepo {
        fn resolve(&self, id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
            Ok(self.by_id.get(id.as_str()).cloned())
        }
        fn callers(&self, _id: &SymbolId) -> Vec<crate::ports::RelationTarget> {
            Vec::new()
        }
        fn callees(&self, _id: &SymbolId) -> Vec<crate::ports::RelationTarget> {
            Vec::new()
        }
        fn fan_in(&self, _id: &SymbolId) -> usize {
            0
        }
        fn fan_out(&self, _id: &SymbolId) -> usize {
            0
        }
        fn find_symbols_by_name(&self, name: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.by_name.get(name).cloned().unwrap_or_default())
        }
        fn find_symbols_by_file(&self, file: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self
                .by_id
                .values()
                .filter(|s| s.file == file)
                .cloned()
                .collect())
        }
        fn module_list(&self) -> Vec<String> {
            let mut modules: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            for s in self.by_id.values() {
                if let Some(parent) = std::path::Path::new(&s.file).parent() {
                    let p = parent.to_string_lossy().to_string();
                    if !p.is_empty() {
                        modules.insert(p);
                    }
                }
            }
            modules.into_iter().collect()
        }
        fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.by_id.values().cloned().collect())
        }
        fn graph_stats(&self) -> GraphStats {
            GraphStats::default()
        }
    }

    /// Mock search backend that returns pre-canned hits. Filters by an
    /// optional query substring so tests can drive which hits surface for
    /// a given query — important because a real FTS5 backend is query-aware.
    #[derive(Default)]
    struct MockSearch {
        hits: Vec<SearchHit>,
        /// If set, only return hits whose name contains this substring.
        match_substring: Option<String>,
    }
    impl MockSearch {
        fn new(hits: Vec<SearchHit>) -> Self {
            Self {
                hits,
                match_substring: None,
            }
        }
        fn with_filter(hits: Vec<SearchHit>, match_substring: impl Into<String>) -> Self {
            Self {
                hits,
                match_substring: Some(match_substring.into()),
            }
        }
    }
    impl SearchRepository for MockSearch {
        fn search(&self, _query: &str, _limit: usize) -> ExplorerResult<Vec<SearchHit>> {
            let hits: Vec<SearchHit> = match &self.match_substring {
                Some(needle) => self
                    .hits
                    .iter()
                    .filter(|h| h.name.contains(needle.as_str()))
                    .cloned()
                    .collect(),
                None => self.hits.clone(),
            };
            Ok(hits)
        }
    }

    fn build_service_with_search(
        repo: MockRepo,
        search: Option<MockSearch>,
    ) -> (ExplorerService, Arc<MockRepo>) {
        let repo_arc = Arc::new(repo);
        let reader = Arc::new(FsSourceReader::new("/tmp"));
        let repo_dyn: Arc<dyn SymbolRepository> = repo_arc.clone();
        let search: Option<Arc<dyn SearchRepository>> = search.map(|s| {
            let arc: Arc<dyn SearchRepository> = Arc::new(s);
            arc
        });
        let service = ExplorerService::with_search(repo_dyn, reader, "/tmp", search);
        (service, repo_arc)
    }

    fn empty_repo() -> MockRepo {
        MockRepo::new()
    }

    #[test]
    fn spotter_exact_only_when_no_search_backend() {
        let mut repo = empty_repo();
        repo.with_symbol("alpha", "src/a.rs", 1, SymbolKind::Function);
        let (service, _) = build_service_with_search(repo, None);
        let results = service.spotter_search("alpha", None).expect("ok");
        assert_eq!(results.len(), 1);
        assert!((results[0].score - 1.0).abs() < f32::EPSILON);
        assert_eq!(results[0].match_type, "exact");
    }

    #[test]
    fn spotter_merges_fts5_hits_below_exact() {
        let mut repo = empty_repo();
        repo.with_symbol("alpha", "src/a.rs", 1, SymbolKind::Function);
        let fts5 = MockSearch::new(vec![SearchHit {
            mvp_id: String::new(),
            name: "alpha".to_string(),
            kind: "Function".to_string(),
            file: "src/a.rs".to_string(),
            line: 0,
            score: 0.95,
            match_type: "fts5".to_string(),
        }]);
        let (service, _) = build_service_with_search(repo, Some(fts5));
        let results = service.spotter_search("alpha", None).expect("ok");
        // Same MVP id deduplicated; exact wins.
        assert_eq!(results.len(), 1);
        assert!((results[0].score - 1.0).abs() < f32::EPSILON);
        assert_eq!(results[0].match_type, "exact");
    }

    #[test]
    fn spotter_fts5_only_when_no_exact_match() {
        let repo = empty_repo();
        let fts5 = MockSearch::new(vec![SearchHit {
            mvp_id: String::new(),
            name: "beta".to_string(),
            kind: "Struct".to_string(),
            file: "src/b.rs".to_string(),
            line: 0,
            score: 0.95,
            match_type: "fts5".to_string(),
        }]);
        let (service, _) = build_service_with_search(repo, Some(fts5));
        // Repo has no "beta" — fts5 hit must be dropped.
        let results = service.spotter_search("beta", None).expect("ok");
        assert!(
            results.is_empty(),
            "fts5 hit must drop when symbol is not in repo: {results:?}"
        );
    }

    #[test]
    fn spotter_fts5_resolution_overrides_fts5_metadata() {
        // Verify that FTS5 hits are resolved against the repository: the
        // repository's kind/file/line win over whatever the FTS5 backend
        // claims. When the FTS5 hit refers to a symbol the repository also
        // has, the exact match wins via dedup — but the resolved metadata
        // must still be the repository's truth.
        let mut repo = empty_repo();
        repo.with_symbol("gamma", "src/c.rs", 42, SymbolKind::Trait);
        let fts5 = MockSearch::new(vec![SearchHit {
            mvp_id: String::new(),
            name: "gamma".to_string(),
            kind: "function".to_string(),      // wrong kind on purpose
            file: "wrong_file.rs".to_string(), // wrong file on purpose
            line: 999,                         // wrong line on purpose
            score: 0.80,
            match_type: "fts5".to_string(),
        }]);
        let (service, _) = build_service_with_search(repo, Some(fts5));
        let results = service.spotter_search("gamma", None).expect("ok");
        assert_eq!(results.len(), 1);
        // The FTS5 hit's "wrong_file.rs" is overridden: the resolution
        // looks up "gamma" and finds it at src/c.rs:42 (Trait), so the
        // subtitle and mvp id carry the repo's truth.
        assert!(
            results[0].object.subtitle.contains("src/c.rs"),
            "subtitle should reference the repo file, got: {:?}",
            results[0].object.subtitle
        );
        assert!(
            results[0].object.subtitle.to_lowercase().contains("trait"),
            "subtitle should carry the repo kind 'trait', got: {:?}",
            results[0].object.subtitle
        );
        assert!(
            results[0].object.id.contains(":42"),
            "mvp id should carry the repo line :42, got: {:?}",
            results[0].object.id
        );
    }

    #[test]
    fn spotter_fts5_only_keeps_fts5_label_when_no_exact_overlap() {
        // When the FTS5 backend surfaces a hit for a name the exact path
        // does NOT find (because the symbol only matches via FTS5 prefix /
        // fuzzy, not via exact equality), the result keeps the "fts5" label
        // and the lower score.
        let mut repo = empty_repo();
        repo.with_symbol("gamma_extension", "src/c.rs", 42, SymbolKind::Trait);
        let fts5 = MockSearch::new(vec![SearchHit {
            mvp_id: String::new(),
            name: "gamma_extension".to_string(),
            kind: "function".to_string(),
            file: "src/c.rs".to_string(),
            line: 0,
            score: 0.90,
            match_type: "fts5".to_string(),
        }]);
        let (service, _) = build_service_with_search(repo, Some(fts5));
        // The exact path uses case-insensitive equality on the WHOLE name.
        // A query of "gamma_ext" will not exact-match "gamma_extension",
        // but the FTS5 backend (mocked to return the hit unconditionally)
        // does surface it. The mock ignores the query string in this test
        // to simulate a fuzzy match.
        let results = service.spotter_search("gamma_ext", None).expect("ok");
        assert_eq!(results.len(), 1, "FTS5-only hit should surface");
        assert_eq!(results[0].match_type, "fts5");
        assert!((results[0].score - 0.90).abs() < 0.01);
        // Line + file come from the repo resolution.
        assert!(results[0].object.id.contains(":42"));
        assert!(results[0].object.id.contains("src/c.rs"));
    }

    #[test]
    fn spotter_fts5_query_aware_mock_filters_unrelated_hits() {
        // Verify the mock backend's query filter does what it claims: a
        // query that doesn't match any of the canned hits returns nothing.
        let mut repo = empty_repo();
        repo.with_symbol("alpha", "src/a.rs", 1, SymbolKind::Function);
        let fts5 = MockSearch::with_filter(
            vec![SearchHit {
                mvp_id: String::new(),
                name: "gamma".to_string(),
                kind: "function".to_string(),
                file: "src/b.rs".to_string(),
                line: 0,
                score: 0.80,
                match_type: "fts5".to_string(),
            }],
            "gamma", // mock only returns hits whose name contains "gamma"
        );
        let (service, _) = build_service_with_search(repo, Some(fts5));
        // Query "alpha" — exact path returns alpha; FTS5 mock returns nothing.
        let results = service.spotter_search("alpha", None).expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].match_type, "exact");
    }

    #[test]
    fn spotter_kind_filter_applies_to_both_sources() {
        let mut repo = empty_repo();
        repo.with_symbol("alpha", "src/a.rs", 1, SymbolKind::Function);
        let fts5 = MockSearch::new(vec![SearchHit {
            mvp_id: String::new(),
            name: "beta".to_string(),
            kind: "Struct".to_string(),
            file: "src/b.rs".to_string(),
            line: 0,
            score: 0.95,
            match_type: "fts5".to_string(),
        }]);
        let (service, _) = build_service_with_search(repo, Some(fts5));
        // Kind "function" matches the exact hit; the fts5 "Struct" hit is filtered out.
        let results = service
            .spotter_search("alpha", Some("function"))
            .expect("ok");
        assert_eq!(results.len(), 1);
        assert!(
            results[0]
                .object
                .subtitle
                .to_lowercase()
                .contains("function")
        );
    }

    #[test]
    fn save_exploration_resolves_object_identities() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let path = service
            .save_exploration(SaveExplorationRequest {
                workspace_id: "workspace:foo".into(),
                columns: vec![
                    ExplorationColumn {
                        object_id: "symbol:src/a.rs:alpha:1".into(),
                        active_view: Some("overview".into()),
                    },
                    ExplorationColumn {
                        // Duplicate of the first column — must dedupe to one entry.
                        object_id: "symbol:src/a.rs:alpha:1".into(),
                        active_view: None,
                    },
                    ExplorationColumn {
                        object_id: "symbol:src/b.rs:beta:5".into(),
                        active_view: None,
                    },
                ],
                lens: Some("default".into()),
            })
            .expect("ok");
        assert_eq!(path.objects.len(), 2, "duplicate columns must dedupe");
        let ids: Vec<&str> = path.objects.iter().map(|o| o.id.as_str()).collect();
        assert!(ids.contains(&"symbol:src/a.rs:alpha:1"));
        assert!(ids.contains(&"symbol:src/b.rs:beta:5"));
        let alpha = path
            .objects
            .iter()
            .find(|o| o.id.ends_with("alpha:1"))
            .unwrap();
        assert_eq!(alpha.object_type, "symbol");
        assert_eq!(alpha.natural_key, "src/a.rs:alpha:1");
        assert_eq!(alpha.first_seen, path.created_at);
    }

    #[test]
    fn generate_artifact_json_replay_includes_objects() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let path = service
            .save_exploration(SaveExplorationRequest {
                workspace_id: "workspace:foo".into(),
                columns: vec![ExplorationColumn {
                    object_id: "symbol:src/a.rs:alpha:1".into(),
                    active_view: None,
                }],
                lens: None,
            })
            .expect("ok");
        let summary = service
            .generate_artifact(
                &path.id,
                GenerateArtifactRequest {
                    format: crate::dto::ArtifactFormat::JsonReplay,
                },
            )
            .expect("ok");
        let body: serde_json::Value = serde_json::from_str(&summary.content).expect("valid json");
        let objects = body["objects"].as_array().expect("objects array");
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0]["id"], "symbol:src/a.rs:alpha:1");
        assert_eq!(objects[0]["object_type"], "symbol");
        assert_eq!(objects[0]["natural_key"], "src/a.rs:alpha:1");
    }

    #[test]
    fn generate_artifact_for_unknown_exploration_warns_but_does_not_error() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let summary = service
            .generate_artifact(
                "exploration:__missing__",
                GenerateArtifactRequest {
                    format: crate::dto::ArtifactFormat::JsonReplay,
                },
            )
            .expect("ok");
        let body: serde_json::Value = serde_json::from_str(&summary.content).expect("valid json");
        assert_eq!(body["objects"].as_array().unwrap().len(), 0);
        assert!(body["warning"].as_str().unwrap().contains("not found"));
    }

    #[test]
    fn new_constructor_backward_compatible() {
        // The old `new()` signature must still work — it's a thin wrapper
        // around `with_search(.., None)`.
        let mut repo = MockRepo::new();
        repo.with_symbol("alpha", "src/a.rs", 1, SymbolKind::Function);
        let reader = Arc::new(FsSourceReader::new("/tmp"));
        let repo_dyn: Arc<dyn SymbolRepository> = Arc::new(repo);
        let service = ExplorerService::new(repo_dyn, reader, "/tmp");
        let results = service.spotter_search("alpha", None).expect("ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].match_type, "exact");
    }

    // -----------------------------------------------------------------------
    // Phase 2 — File and Scope dispatch
    // -----------------------------------------------------------------------

    #[test]
    fn inspect_file_returns_file_summary() {
        let mut repo = empty_repo();
        repo.with_symbol("alpha", "src/main.rs", 1, SymbolKind::Function);
        repo.with_symbol("beta", "src/main.rs", 10, SymbolKind::Struct);
        let (service, _) = build_service_with_search(repo, None);
        let summary = service
            .inspect_object("file:src/main.rs")
            .expect("file inspect ok");
        assert_eq!(summary.object_type, InspectableObjectType::File);
        assert_eq!(summary.id, "file:src/main.rs");
        assert_eq!(summary.label, "src/main.rs");
        let ids: Vec<&str> = summary
            .available_views
            .iter()
            .map(|v| v.id.as_str())
            .collect();
        assert_eq!(ids, vec!["overview", "symbols", "quality"]);
        let keys: std::collections::HashSet<&str> =
            summary.properties.iter().map(|p| p.key.as_str()).collect();
        for required in ["path", "line_count", "symbol_count", "kinds"] {
            assert!(keys.contains(required), "missing property {required}");
        }
    }

    #[test]
    fn inspect_file_with_no_symbols_still_returns_summary() {
        let repo = empty_repo();
        let (service, _) = build_service_with_search(repo, None);
        let summary = service
            .inspect_object("file:src/empty.rs")
            .expect("ok — empty file is still inspectable");
        assert_eq!(summary.object_type, InspectableObjectType::File);
        let symbol_count = summary
            .properties
            .iter()
            .find(|p| p.key == "symbol_count")
            .unwrap();
        assert_eq!(symbol_count.value, serde_json::json!(0));
    }

    #[test]
    fn inspect_scope_returns_scope_summary() {
        let mut repo = empty_repo();
        repo.with_symbol("alpha", "src/foo/a.rs", 1, SymbolKind::Function);
        repo.with_symbol("beta", "src/foo/b.rs", 2, SymbolKind::Struct);
        repo.with_symbol("gamma", "src/bar/c.rs", 3, SymbolKind::Function);
        let (service, _) = build_service_with_search(repo, None);
        let summary = service.inspect_object("scope:src/foo").expect("ok");
        assert_eq!(summary.object_type, InspectableObjectType::Scope);
        assert_eq!(summary.id, "scope:src/foo");
        let ids: Vec<&str> = summary
            .available_views
            .iter()
            .map(|v| v.id.as_str())
            .collect();
        assert_eq!(ids, vec!["overview", "dependencies", "hotspots", "quality"]);
        let keys: std::collections::HashSet<&str> =
            summary.properties.iter().map(|p| p.key.as_str()).collect();
        for required in [
            "path",
            "file_count",
            "symbol_count",
            "promotion_ready",
            "kinds",
        ] {
            assert!(keys.contains(required), "missing property {required}");
        }
        let promotion = summary
            .properties
            .iter()
            .find(|p| p.key == "promotion_ready")
            .unwrap();
        assert_eq!(promotion.value, serde_json::json!(false));
    }

    #[test]
    fn inspect_scope_does_not_match_bleeding_prefix() {
        // `scope:src` must NOT include `src_extra/...` because the boundary
        // is anchored on `/`. The MockRepo's `module_list` derives the
        // parent directory of every indexed file, so a file at
        // `src_extra/x.rs` reports a parent of `src_extra`, not `src`.
        let mut repo = empty_repo();
        repo.with_symbol("alpha", "src/a.rs", 1, SymbolKind::Function);
        repo.with_symbol("beta", "src_extra/b.rs", 2, SymbolKind::Function);
        let (service, _) = build_service_with_search(repo, None);
        let summary = service.inspect_object("scope:src").expect("ok");
        let symbol_count = summary
            .properties
            .iter()
            .find(|p| p.key == "symbol_count")
            .unwrap();
        assert_eq!(symbol_count.value, serde_json::json!(1));
    }

    #[test]
    fn available_views_dispatches_per_variant() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        assert_eq!(
            service
                .available_views("symbol:src/a.rs:alpha:1")
                .expect("ok")
                .iter()
                .map(|v| v.id.as_str())
                .collect::<Vec<_>>(),
            vec!["overview", "call-graph", "source", "evidence", "quality"]
        );
        assert_eq!(
            service
                .available_views("file:src/a.rs")
                .expect("ok")
                .iter()
                .map(|v| v.id.as_str())
                .collect::<Vec<_>>(),
            vec!["overview", "symbols", "quality"]
        );
        assert_eq!(
            service
                .available_views("scope:src")
                .expect("ok")
                .iter()
                .map(|v| v.id.as_str())
                .collect::<Vec<_>>(),
            vec!["overview", "dependencies", "hotspots", "quality"]
        );
    }

    #[test]
    fn available_views_rejects_unknown_prefix() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        // The parser rejects unknown prefixes before the dispatcher
        // can match on the variant — this is intentional so the surface
        // stays explicit and future identity types are added deliberately.
        let err = service.available_views("module:src").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn contextual_view_dispatches_file_to_correct_builder() {
        let mut repo = empty_repo();
        repo.with_symbol("alpha", "src/main.rs", 1, SymbolKind::Function);
        let (service, _) = build_service_with_search(repo, None);

        let overview = service
            .contextual_view("file:src/main.rs", "overview")
            .expect("overview ok");
        assert_eq!(overview.view_id, "overview");
        assert!(!overview.evidence.is_empty());

        let symbols = service
            .contextual_view("file:src/main.rs", "symbols")
            .expect("symbols ok");
        assert_eq!(symbols.view_id, "symbols");
        assert_eq!(symbols.relations.len(), 1);
        assert_eq!(symbols.relations[0].relation_type, "CONTAINS");
        assert_eq!(
            symbols.relations[0].target_object_id,
            "symbol:src/main.rs:alpha:1"
        );
    }

    #[test]
    fn contextual_view_dispatches_scope_to_correct_builder() {
        let mut repo = empty_repo();
        repo.with_symbol("alpha", "src/foo/a.rs", 1, SymbolKind::Function);
        let (service, _) = build_service_with_search(repo, None);

        let overview = service
            .contextual_view("scope:src/foo", "overview")
            .expect("overview ok");
        assert_eq!(overview.view_id, "overview");

        let dependencies = service
            .contextual_view("scope:src/foo", "dependencies")
            .expect("dependencies ok");
        assert_eq!(dependencies.view_id, "dependencies");

        let hotspots = service
            .contextual_view("scope:src/foo", "hotspots")
            .expect("hotspots ok");
        assert_eq!(hotspots.view_id, "hotspots");
        assert_eq!(hotspots.relations.len(), 1);
        assert_eq!(hotspots.relations[0].relation_type, "HOTSPOT");
    }

    #[test]
    fn contextual_view_rejects_unknown_view_id_per_variant() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        // A symbol-only view id on a file identity is rejected.
        let err = service
            .contextual_view("file:src/main.rs", "call-graph")
            .unwrap_err();
        assert!(matches!(err, ExplorerError::ViewNotAvailable { .. }));
        // A file-only view id on a scope identity is rejected.
        let err = service.contextual_view("scope:src", "symbols").unwrap_err();
        assert!(matches!(err, ExplorerError::ViewNotAvailable { .. }));
    }

    #[test]
    fn save_exploration_accepts_file_and_scope_identities() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let path = service
            .save_exploration(SaveExplorationRequest {
                workspace_id: "workspace:foo".into(),
                columns: vec![
                    ExplorationColumn {
                        object_id: "file:src/main.rs".into(),
                        active_view: Some("symbols".into()),
                    },
                    ExplorationColumn {
                        object_id: "scope:src".into(),
                        active_view: Some("overview".into()),
                    },
                ],
                lens: None,
            })
            .expect("ok");
        assert_eq!(path.objects.len(), 2);
        let by_id: std::collections::HashMap<String, &crate::dto::ObjectIdentityEntry> =
            path.objects.iter().map(|o| (o.id.clone(), o)).collect();
        let file_entry = by_id.get("file:src/main.rs").expect("file entry");
        assert_eq!(file_entry.object_type, "file");
        assert_eq!(file_entry.natural_key, "src/main.rs");
        let scope_entry = by_id.get("scope:src").expect("scope entry");
        assert_eq!(scope_entry.object_type, "scope");
        assert_eq!(scope_entry.natural_key, "src");
    }

    // -----------------------------------------------------------------------
    // Phase 4 — Design Lenses (service-level)
    // -----------------------------------------------------------------------

    #[test]
    fn available_lenses_filters_by_object_type() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let sym_lenses = service
            .available_lenses("symbol:src/a.rs:alpha:1")
            .expect("ok");
        let file_lenses = service.available_lenses("file:src/a.rs").expect("ok");
        let scope_lenses = service.available_lenses("scope:src").expect("ok");
        let issue_lenses = service.available_lenses("issue:42").expect("ok");
        // Symbol, File, Scope each have 3 lenses; Issue gets an empty list
        // (no lens applies to issues).
        assert_eq!(sym_lenses.len(), 3);
        assert_eq!(file_lenses.len(), 3);
        assert_eq!(scope_lenses.len(), 3);
        assert!(issue_lenses.is_empty());
    }

    #[test]
    fn available_lenses_rejects_unknown_prefix() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let err = service.available_lenses("module:src").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn apply_lens_with_unknown_id_errors() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let err = service
            .apply_lens("symbol:src/a.rs:alpha:1", "does-not-exist")
            .unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn apply_lens_unknown_symbol_returns_empty_findings() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        // The symbol is not in the repo, so hotspots' resolve call returns
        // None and the lens emits no findings — but the call is still Ok.
        let result = service
            .apply_lens("symbol:src/a.rs:alpha:1", "hotspots")
            .expect("ok");
        assert_eq!(result.lens_id, "hotspots");
        assert!(result.findings.is_empty());
        assert!(result.summary.contains("No hotspots"));
    }

    #[test]
    fn apply_lens_dependencies_on_empty_scope_returns_empty() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let result = service
            .apply_lens("scope:src/empty", "dependencies")
            .expect("ok");
        assert!(result.findings.is_empty());
    }

    #[test]
    fn apply_lens_architecture_on_empty_scope_returns_empty() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let result = service
            .apply_lens("scope:src/empty", "architecture")
            .expect("ok");
        assert!(result.findings.is_empty());
    }

    // -----------------------------------------------------------------------
    // Named Views — feature-gate unit tests
    // -----------------------------------------------------------------------
    //
    // The `with_all` constructor leaves `postgres_repo: None`,
    // so every `*_view` call on the resulting service must
    // return `Err(ExplorerError::FeatureDisabled(..))` — the
    // canonical "postgres feature not active" soft error.
    //
    // The validation tests (`view_save_rejects_*`) are also
    // feature-gated: validation runs BEFORE the PG call, so the
    // invalid-input errors surface even on a no-PG build.

    #[tokio::test]
    async fn explorer_service_pg_disabled_save_returns_feature_disabled() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let err = service
            .save_view(
                "w1",
                "u1",
                "hotspots",
                None,
                "function",
                "callgraph",
                "crate::foo",
                3,
            )
            .await
            .expect_err("save_view must error when PG is not wired");
        assert!(
            matches!(err, ExplorerError::FeatureDisabled(_)),
            "expected FeatureDisabled, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn explorer_service_pg_disabled_load_returns_feature_disabled() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let err = service
            .load_view("some-id", "w1", "u1")
            .await
            .expect_err("load_view must error when PG is not wired");
        assert!(
            matches!(err, ExplorerError::FeatureDisabled(_)),
            "expected FeatureDisabled, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn explorer_service_pg_disabled_list_returns_feature_disabled() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let err = service
            .list_views("w1", "u1")
            .await
            .expect_err("list_views must error when PG is not wired");
        assert!(
            matches!(err, ExplorerError::FeatureDisabled(_)),
            "expected FeatureDisabled, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn explorer_service_pg_disabled_delete_returns_feature_disabled() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let err = service
            .delete_view("some-id", "w1", "u1")
            .await
            .expect_err("delete_view must error when PG is not wired");
        assert!(
            matches!(err, ExplorerError::FeatureDisabled(_)),
            "expected FeatureDisabled, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn explorer_service_pg_disabled_returns_feature_disabled_for_all_four() {
        // Aggregate assertion: every one of the four CRUD
        // methods returns FeatureDisabled on a default build.
        // Companion to the per-method tests above; the per-method
        // tests pinpoint which method regressed, this test is
        // the spec's contractual gate.
        let (service, _) = build_service_with_search(empty_repo(), None);
        assert!(matches!(
            service
                .save_view("w", "u", "n", None, "l", "ln", "f", 0)
                .await
                .unwrap_err(),
            ExplorerError::FeatureDisabled(_)
        ));
        assert!(matches!(
            service.load_view("i", "w", "u").await.unwrap_err(),
            ExplorerError::FeatureDisabled(_)
        ));
        assert!(matches!(
            service.list_views("w", "u").await.unwrap_err(),
            ExplorerError::FeatureDisabled(_)
        ));
        assert!(matches!(
            service.delete_view("i", "w", "u").await.unwrap_err(),
            ExplorerError::FeatureDisabled(_)
        ));
    }

    // -----------------------------------------------------------------------
    // Phase 6 — MoldQL service-level integration
    // -----------------------------------------------------------------------

    #[test]
    fn execute_query_find_symbols_returns_sorted_matches() {
        let mut repo = empty_repo();
        repo.with_symbol("alpha", "src/a.rs", 1, SymbolKind::Function);
        repo.with_symbol("beta", "src/b.rs", 5, SymbolKind::Struct);
        repo.with_symbol("gamma", "src/c.rs", 3, SymbolKind::Function);
        let (service, _) = build_service_with_search(repo, None);

        let result = service
            .execute_query("FIND symbols WHERE kind = \"Function\"")
            .expect("ok");
        assert_eq!(result.total, 2);
        assert_eq!(result.items[0].object_id, "symbol:src/a.rs:alpha:1");
        assert_eq!(result.items[1].object_id, "symbol:src/c.rs:gamma:3");
        // The query string is echoed back.
        assert!(result.query.contains("FIND symbols"));
    }

    #[test]
    fn execute_query_explore_returns_seed_in_items() {
        let mut repo = empty_repo();
        repo.with_symbol("main", "src/main.rs", 1, SymbolKind::Function);
        let (service, _) = build_service_with_search(repo, None);

        let result = service
            .execute_query("EXPLORE symbol:src/main.rs:main:1 THROUGH callees DEPTH 0")
            .expect("ok");
        assert_eq!(result.total, 1);
        assert_eq!(result.items[0].object_id, "symbol:src/main.rs:main:1");
    }

    #[test]
    fn execute_query_invalid_query_returns_resolution_error() {
        let (service, _) = build_service_with_search(empty_repo(), None);
        let err = service.execute_query("FOO symbols").unwrap_err();
        assert!(matches!(err, ExplorerError::ResolutionFailed(_)));
    }

    #[test]
    fn execute_query_quality_condition_with_no_backend_returns_empty() {
        let mut repo = empty_repo();
        repo.with_symbol("alpha", "src/a.rs", 1, SymbolKind::Function);
        let (service, _) = build_service_with_search(repo, None);

        // No quality backend wired → quality.critical is always 0 → strict `> 0` is false.
        let result = service
            .execute_query("FIND symbols WHERE quality.critical > 0")
            .expect("ok");
        assert_eq!(result.total, 0);
    }
}
