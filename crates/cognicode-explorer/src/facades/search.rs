//! [`SearchService`] implementation.

use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::object_identity::ObjectIdentity;
use crate::domain::views::scope_contains_file;
use crate::dto::{
    InspectableObjectSummary, InspectableObjectType, Property, SpotterResult, SpotterSearchResult,
    ViewSpecSummary,
};
use crate::error::{ExplorerError, ExplorerResult};
use crate::facades::SearchService;
use crate::ports::search_repository::SearchHit;
use crate::ports::symbol_repository::{ResolvedSymbol, SymbolRepository};
use crate::registry::ViewRegistry;
use crate::registry::ViewSpecStore;

/// Cap on the number of Spotter results returned per query.
const SPOTTER_RESULT_LIMIT: usize = 20;

/// Concrete implementation of [`SearchService`].
///
/// Holds the same ports that `ExplorerService` uses for search and inspection.
pub struct SearchServiceImpl {
    repo: Arc<dyn SymbolRepository>,
    search: Option<Arc<dyn crate::ports::SearchRepository>>,
    view_registry: Arc<ViewRegistry>,
    view_spec_store: Option<Arc<dyn ViewSpecStore>>,
    quality: Option<Arc<dyn crate::ports::QualityRepository>>,
}

impl SearchServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        repo: Arc<dyn SymbolRepository>,
        search: Option<Arc<dyn crate::ports::SearchRepository>>,
        view_registry: Arc<ViewRegistry>,
        view_spec_store: Option<Arc<dyn ViewSpecStore>>,
        quality: Option<Arc<dyn crate::ports::QualityRepository>>,
    ) -> Self {
        Self {
            repo,
            search,
            view_registry,
            view_spec_store,
            quality,
        }
    }
}

#[async_trait]
impl SearchService for SearchServiceImpl {
    async fn spotter_search(
        &self,
        query: &str,
        kind: Option<&str>,
    ) -> ExplorerResult<Vec<SpotterResult>> {
        // Run sync search in a blocking thread to avoid blocking the async runtime.
        let repo = self.repo.clone();
        let search = self.search.clone();
        let view_registry = self.view_registry.clone();
        let query = query.to_string();
        let kind = kind.map(|s| s.to_string());

        tokio::task::spawn_blocking(move || {
            spotter_search_impl(
                &repo,
                search.as_ref(),
                &view_registry,
                &query,
                kind.as_deref(),
            )
        })
        .await
        .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("join error: {e}")))?
    }

    async fn spotter_search_with_viewspecs(
        &self,
        query: &str,
        kind: Option<&str>,
        workspace_id: Option<&str>,
    ) -> ExplorerResult<Vec<SpotterSearchResult>> {
        // 1) Get symbol/file hits (sync via blocking task)
        let symbol_results = self.spotter_search(query, kind).await?;

        // 2) Get ViewSpec hits (async)
        let mut viewspec_results: Vec<SpotterSearchResult> = Vec::new();
        if let (Some(workspace_id), Some(store)) = (workspace_id, &self.view_spec_store) {
            let query_lower = query.to_lowercase();
            if let Ok(all_specs) = store
                .list_for_workspace(workspace_id, InspectableObjectType::Symbol)
                .await
            {
                for spec in all_specs {
                    let title_match = spec.title.to_lowercase().contains(&query_lower);
                    let kind_match = format!("{:?}", spec.view_kind)
                        .to_lowercase()
                        .contains(&query_lower);

                    if title_match || kind_match {
                        viewspec_results.push(SpotterSearchResult::ViewSpec(ViewSpecSummary {
                            id: spec.id.clone(),
                            title: spec.title.clone(),
                            view_kind: spec.view_kind.clone(),
                            applies_to: spec.applies_to,
                            owner: spec.owner.clone(),
                            updated_at: spec.updated_at.clone(),
                        }));
                    }
                }
            }
        }

        // 3) Build symbol results as SpotterSearchResult
        let symbol_hits: Vec<SpotterSearchResult> = symbol_results
            .into_iter()
            .map(SpotterSearchResult::Symbol)
            .collect();

        // 4) Merge: symbols first, then ViewSpecs
        let mut all_hits: Vec<SpotterSearchResult> =
            Vec::with_capacity(symbol_hits.len() + viewspec_results.len());
        all_hits.extend(symbol_hits);
        all_hits.extend(viewspec_results);

        Ok(all_hits)
    }

    async fn inspect_object(&self, object_id: &str) -> ExplorerResult<InspectableObjectSummary> {
        // Run sync inspection in a blocking thread.
        let repo = self.repo.clone();
        let search = self.search.clone();
        let view_registry = self.view_registry.clone();
        let quality = self.quality.clone();
        let object_id = object_id.to_string();

        tokio::task::spawn_blocking(move || {
            inspect_object_impl(
                &repo,
                search.as_ref(),
                &view_registry,
                quality.as_deref(),
                &object_id,
            )
        })
        .await
        .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("join error: {e}")))?
    }
}

// ---------------------------------------------------------------------------
// Implementation helpers (sync, run inside spawn_blocking)
// ---------------------------------------------------------------------------

fn spotter_search_impl(
    repo: &Arc<dyn SymbolRepository>,
    search: Option<&Arc<dyn crate::ports::SearchRepository>>,
    view_registry: &Arc<ViewRegistry>,
    query: &str,
    kind: Option<&str>,
) -> ExplorerResult<Vec<SpotterResult>> {
    if query.is_empty() {
        return Ok(Vec::new());
    }

    // 1) Exact matches from symbol repository.
    let exact_matches = repo.find_symbols_by_name(query)?;

    // 2) FTS5 / fuzzy matches.
    let fts5_matches: Vec<SearchHit> = match search {
        Some(search) => search.search(query, SPOTTER_RESULT_LIMIT)?,
        None => Vec::new(),
    };

    // 3) Build unified hit stream.
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
        let resolved = repo
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

    // 4) Deduplicate by MVP id.
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

    // 6) Materialise SpotterResult.
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
                    properties: Vec::new(),
                    available_views: view_registry.list_for(InspectableObjectType::Symbol),
                },
                score: hit.score,
                match_type: hit.match_type,
            }
        })
        .collect())
}

fn inspect_object_impl(
    repo: &Arc<dyn SymbolRepository>,
    _search: Option<&Arc<dyn crate::ports::SearchRepository>>,
    view_registry: &Arc<ViewRegistry>,
    quality: Option<&dyn crate::ports::QualityRepository>,
    object_id: &str,
) -> ExplorerResult<InspectableObjectSummary> {
    let identity = ObjectIdentity::parse_mvp_id(object_id)?;
    match &identity {
        ObjectIdentity::Symbol { .. } => inspect_symbol_impl(repo, view_registry, &identity),
        ObjectIdentity::File { path } => inspect_file_impl(repo, view_registry, &identity, path),
        ObjectIdentity::Scope { path } => inspect_scope_impl(repo, view_registry, &identity, path),
        ObjectIdentity::QualityIssue { id } => {
            inspect_quality_issue_impl(quality, view_registry, &identity, *id)
        }
        ObjectIdentity::Rule { rule_id } => {
            inspect_rule_impl(quality, view_registry, &identity, rule_id)
        }
    }
}

fn inspect_symbol_impl(
    repo: &Arc<dyn SymbolRepository>,
    view_registry: &Arc<ViewRegistry>,
    identity: &ObjectIdentity,
) -> ExplorerResult<InspectableObjectSummary> {
    let symbol_id = identity
        .to_symbol_id()
        .expect("Symbol identity always yields a SymbolId");
    let resolved = repo
        .resolve(&symbol_id)?
        .ok_or_else(|| ExplorerError::ObjectNotFound(identity.to_mvp_id()))?;

    let properties = build_summary_properties(&resolved, None);

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
        available_views: view_registry.list_for(InspectableObjectType::Symbol),
    })
}

fn inspect_file_impl(
    repo: &Arc<dyn SymbolRepository>,
    view_registry: &Arc<ViewRegistry>,
    identity: &ObjectIdentity,
    path: &str,
) -> ExplorerResult<InspectableObjectSummary> {
    let symbols = repo.find_symbols_by_file(path)?;
    let kinds = count_kinds(&symbols);

    let mut properties = vec![
        Property {
            key: "path".into(),
            value: serde_json::Value::String(path.to_string()),
            value_type: "string".into(),
            source: "ObjectIdentity".into(),
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
    properties.sort_by(|a, b| a.key.cmp(&b.key));

    Ok(InspectableObjectSummary {
        id: identity.to_mvp_id(),
        object_type: InspectableObjectType::File,
        label: path.to_string(),
        subtitle: if symbols.is_empty() {
            "0 symbols".to_string()
        } else {
            format!("{} symbol(s)", symbols.len())
        },
        properties,
        available_views: view_registry.list_for(InspectableObjectType::File),
    })
}

fn inspect_scope_impl(
    repo: &Arc<dyn SymbolRepository>,
    view_registry: &Arc<ViewRegistry>,
    identity: &ObjectIdentity,
    path: &str,
) -> ExplorerResult<InspectableObjectSummary> {
    let (member_files, member_symbols) = derive_scope_members(repo.as_ref(), path);
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
        available_views: view_registry.list_for(InspectableObjectType::Scope),
    })
}

fn inspect_quality_issue_impl(
    quality: Option<&dyn crate::ports::QualityRepository>,
    view_registry: &Arc<ViewRegistry>,
    identity: &ObjectIdentity,
    id: i64,
) -> ExplorerResult<InspectableObjectSummary> {
    let issue = quality.and_then(|q| q.issue_by_id(id).ok().flatten());

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
                    value: serde_json::Value::String(i.file_path.clone()),
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
                format!("{} at {}:{}", i.severity, i.file_path, i.line),
                properties,
            )
        }
        None => (
            format!("Issue #{id}"),
            if quality.is_some() {
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
        available_views: view_registry.list_for(InspectableObjectType::QualityIssue),
    })
}

fn inspect_rule_impl(
    quality: Option<&dyn crate::ports::QualityRepository>,
    view_registry: &Arc<ViewRegistry>,
    identity: &ObjectIdentity,
    rule_id: &str,
) -> ExplorerResult<InspectableObjectSummary> {
    let summary = quality.and_then(|q| q.rule_summary(rule_id).ok());

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
            if quality.is_some() {
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
        available_views: view_registry.list_for(InspectableObjectType::Rule),
    })
}

fn resolved_to_mvp(resolved: &crate::ports::symbol_repository::ResolvedSymbol) -> String {
    format!(
        "symbol:{}:{}:{}",
        resolved.file, resolved.name, resolved.line
    )
}

fn build_summary_properties(
    resolved: &crate::ports::symbol_repository::ResolvedSymbol,
    graph_query: Option<&dyn cognicode_core::domain::traits::graph_query_port::GraphQueryPort>,
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
            value: Value::Number(
                graph_query
                    .map(|gq| gq.fan_in(&resolved.id))
                    .unwrap_or(0)
                    .into(),
            ),
            value_type: "usize".into(),
            source: "CallGraph".into(),
        },
        Property {
            key: "fan_out".into(),
            value: Value::Number(
                graph_query
                    .map(|gq| gq.fan_out(&resolved.id))
                    .unwrap_or(0)
                    .into(),
            ),
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

// ---------------------------------------------------------------------------
// Helper functions (mirrored from service.rs for facade independence)
// ---------------------------------------------------------------------------

/// Collect the unique files and the resolved symbols that belong to `scope_path`.
fn derive_scope_members(
    repo: &dyn SymbolRepository,
    scope_path: &str,
) -> (Vec<String>, Vec<ResolvedSymbol>) {
    let mut files: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut symbols: Vec<ResolvedSymbol> = Vec::new();
    if let Ok(all) = repo.all_symbols() {
        for sym in all {
            if scope_contains_file(scope_path, &sym.file) {
                files.insert(sym.file.clone());
                symbols.push(sym);
            }
        }
    }
    symbols.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.line.cmp(&b.line)));
    (files.into_iter().collect(), symbols)
}

/// Count symbols per kind, returning a stable map.
fn count_kinds(symbols: &[ResolvedSymbol]) -> std::collections::BTreeMap<String, usize> {
    let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for s in symbols {
        *kinds.entry(s.kind.name().to_string()).or_insert(0) += 1;
    }
    kinds
}
