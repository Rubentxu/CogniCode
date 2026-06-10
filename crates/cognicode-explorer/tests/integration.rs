//! Integration tests against the real `.cognicode/cognicode.db` at the
//! workspace root. These tests are skipped automatically when the DB is
//! missing — the contract is "pass when an indexed workspace is present".
//!
//! To run: `cargo test -p cognicode-explorer --test integration`
//! To skip: leave `.cognicode/cognicode.db` absent in the workspace.
//!
//! File-level feature gate (`postgres-default-config` PR 1): when the
//! `sqlite` feature is disabled on `cognicode-explorer`, this entire
//! test file is excluded from the build (the `SqliteGraphStore` it
//! depends on does not exist). Use
//! `cargo test -p cognicode-explorer --features sqlite` to opt in.
#![cfg(feature = "sqlite")]

use std::path::PathBuf;
use std::sync::Arc;

use cognicode_core::domain::aggregates::CallGraph;
use cognicode_db::SqliteGraphStore;
use cognicode_explorer::adapters::{CallGraphRepository, FsSourceReader};
use cognicode_explorer::dto::InspectableObjectType;
use cognicode_explorer::ports::symbol_repository::SymbolRepository;
use cognicode_explorer::service::ExplorerService;

/// Shared setup helper: open the workspace DB, load the graph, build the
/// service. Returns `None` (and the test should `return` early) when the
/// DB is not present OR when the graph blob is empty, so tests gracefully
/// skip in either case.
fn build_service_from_workspace_db() -> Option<(ExplorerService, Arc<CallGraph>)> {
    let db_path = workspace_db()?;
    let cwd = db_path
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf();

    let store = SqliteGraphStore::open(&db_path).ok()?;
    let graph = store.load_graph().ok()?;
    let graph = match graph {
        Some(g) => g,
        None => {
            eprintln!(
                "[integration] cognicode.db is present at {} but contains no graph — skipping",
                db_path.display()
            );
            return None;
        }
    };
    let graph_arc = Arc::new(graph);

    let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph_arc.clone()));
    let reader = Arc::new(FsSourceReader::new(cwd.clone()));
    let service = ExplorerService::new(repo, reader, cwd.clone());
    Some((service, graph_arc))
}

fn workspace_db() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let candidate = cwd.join(".cognicode/cognicode.db");
    if candidate.exists() {
        Some(candidate)
    } else {
        None
    }
}

#[test]
fn inspect_object_against_real_db_returns_summary() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found or empty — skipping");
        return;
    };

    // Pick the first symbol from the graph.
    let first = graph_arc
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let mvp_id = format!(
        "symbol:{}:{}:{}",
        first.1.location().file(),
        first.1.name(),
        first.1.location().line()
    );

    let summary = service.inspect_object(&mvp_id).expect("inspect_object ok");
    assert_eq!(summary.id, mvp_id);
    assert!(
        !summary.available_views.is_empty(),
        "should expose at least 1 view"
    );
}

#[test]
fn contextual_view_evidence_returns_evidence_blocks() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found or empty — skipping");
        return;
    };

    let first = graph_arc
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let mvp_id = format!(
        "symbol:{}:{}:{}",
        first.1.location().file(),
        first.1.name(),
        first.1.location().line()
    );

    let view = service
        .contextual_view(&mvp_id, "evidence")
        .expect("evidence view ok");
    assert_eq!(view.view_id, "evidence");
    assert!(
        !view.evidence.is_empty(),
        "evidence view must contain at least one evidence block"
    );
    let kinds: Vec<&str> = view.evidence.iter().map(|b| b.kind.as_str()).collect();
    // Spec: at least the symbol_metadata kind must be present.
    assert!(
        kinds.contains(&"symbol_metadata"),
        "missing symbol_metadata evidence, got: {kinds:?}"
    );

    // Phase 1C: freshness must be present on every evidence block (serde
    // default means a missing field is fine for old fixtures, but every
    // freshly-built block carries one).
    for block in &view.evidence {
        assert!(
            block.freshness.is_some(),
            "evidence block {:?} missing freshness",
            block.id
        );
    }
}

#[test]
fn inspect_object_rejects_malformed_id() {
    let cwd = std::env::current_dir().expect("cwd");
    let reader = Arc::new(FsSourceReader::new(cwd.clone()));
    // No DB needed — the parser must reject before any graph lookup.
    let empty: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(Arc::new(
        cognicode_core::domain::aggregates::CallGraph::new(),
    )));
    let service = ExplorerService::new(empty, reader, cwd);

    let err = service.inspect_object("not_a_mvp_id").unwrap_err();
    assert!(matches!(
        err,
        cognicode_explorer::ExplorerError::ResolutionFailed(_)
    ));
}

// ---------------------------------------------------------------------------
// Phase 1B — Spotter Search
// ---------------------------------------------------------------------------

#[test]
fn spotter_search_finds_known_symbol() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    // Pick a real symbol from the graph and use its name as the query.
    // This makes the test robust to whatever the indexer happens to find
    // first on a given workspace — no hard-coded symbol names.
    let (id, sym) = graph_arc
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let name = sym.name().to_string();
    let expected_file = sym.location().file().to_string();
    let expected_line = sym.location().line();
    let expected_mvp = format!("symbol:{expected_file}:{name}:{expected_line}");

    let results = service
        .spotter_search(&name, None)
        .expect("spotter_search ok");

    assert!(
        !results.is_empty(),
        "spotter_search must return at least one result for an indexed name"
    );
    // Every result has score 1.0, match_type "exact" (spec Req 1).
    for r in &results {
        assert!((r.score - 1.0).abs() < f32::EPSILON);
        assert_eq!(r.match_type, "exact");
    }
    // The chosen symbol must be among the results.
    let ids: Vec<&str> = results.iter().map(|r| r.object.id.as_str()).collect();
    assert!(
        ids.contains(&expected_mvp.as_str()),
        "expected mvp {expected_mvp} in {ids:?} (graph id was {id})"
    );
    // The matched object's label equals the queried name.
    let matched = results
        .iter()
        .find(|r| r.object.id == expected_mvp)
        .expect("matched result present");
    assert_eq!(matched.object.label, name);
    assert!(matches!(
        matched.object.object_type,
        InspectableObjectType::Symbol
    ));
    // Subtitle encodes kind + location (matches existing inspect format).
    assert!(matched.object.subtitle.contains(&expected_file));
}

#[test]
fn spotter_search_empty_result() {
    let Some((service, _)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    // A name that almost certainly does not exist in any indexed workspace.
    let results = service
        .spotter_search("__zzz_no_such_symbol_in_workspace__", None)
        .expect("spotter_search ok — no error on missing name");
    assert!(
        results.is_empty(),
        "spotter_search must return an empty Vec, not an error, for unknown names"
    );
}

#[test]
fn spotter_search_kind_filter() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    // Find a function symbol to search for. The kind filter is case-insensitive
    // per spec Req 2 — pass "Function" in mixed case to verify.
    let (func_id, func_sym) = graph_arc
        .symbol_ids()
        .find(|(_, s)| s.kind().is_callable())
        .expect("at least one callable symbol in the indexed graph");
    let name = func_sym.name().to_string();
    let kind_name = func_sym.kind().name().to_string();

    // With the matching kind filter, the function must be returned.
    let matched = service
        .spotter_search(&name, Some(&kind_name.to_uppercase()))
        .expect("spotter_search ok");
    assert!(
        !matched.is_empty(),
        "expected at least one result when kind filter matches the function"
    );
    let expected_mvp = format!(
        "symbol:{}:{}:{}",
        func_sym.location().file(),
        name,
        func_sym.location().line()
    );
    assert!(
        matched.iter().any(|r| r.object.id == expected_mvp),
        "expected mvp {expected_mvp} in {:?}",
        matched.iter().map(|r| &r.object.id).collect::<Vec<_>>()
    );
    // Every matched result's subtitle should reference the same kind.
    for r in &matched {
        let label = format!("{} ", kind_name);
        assert!(
            r.object
                .subtitle
                .to_lowercase()
                .contains(&kind_name.to_lowercase()),
            "result subtitle {:?} should mention kind {kind_name:?}",
            r.object.subtitle
        );
        let _ = label; // silence unused if assertions above change
    }

    // Now filter with a kind that DOESN'T match the function — must be empty.
    // Find a kind present in the graph that this function is NOT, then query.
    let other_kind = graph_arc
        .symbol_ids()
        .map(|(_, s)| s.kind().name().to_string())
        .find(|k| k != &kind_name)
        .expect("graph contains at least two distinct kinds");
    let filtered_out = service
        .spotter_search(&name, Some(&other_kind))
        .expect("spotter_search ok");
    assert!(
        !filtered_out.iter().any(|r| r.object.id == expected_mvp),
        "kind filter {other_kind:?} must exclude function {expected_mvp} (id was {func_id})"
    );
}

#[test]
fn workspace_summary_has_stats() {
    let Some((service, _)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found or empty — skipping");
        return;
    };

    let cwd = std::env::current_dir().expect("cwd");
    // Stats are populated only when the DB exists at the requested path —
    // which is the case here, since we just opened it.
    let summary = service
        .open_workspace(cognicode_explorer::dto::OpenWorkspaceRequest {
            root_path: cwd.display().to_string(),
        })
        .expect("open_workspace ok");

    assert!(
        summary.symbol_count > 0,
        "symbol_count must be > 0 when the graph is loaded (got {})",
        summary.symbol_count
    );
    // relation_count is a sum over edges; it can be 0 for a graph with no
    // call relations, but the underlying graph should at least expose the
    // field as a usize without panicking.
    let _ = summary.relation_count;
}

// ---------------------------------------------------------------------------
// Phase 1C — FTS5 search, JSON replay objects, evidence freshness
// ---------------------------------------------------------------------------

use cognicode_explorer::adapters::Fts5SearchAdapter;
use cognicode_explorer::dto::{
    ArtifactFormat, ExplorationColumn, GenerateArtifactRequest, SaveExplorationRequest,
};
use cognicode_explorer::ports::search_repository::SearchRepository;

#[test]
fn fts5_search_returns_hits_for_indexed_name() {
    let Some(_db) = workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping FTS5 test");
        return;
    };
    let db_path = _db;
    let adapter = Fts5SearchAdapter::new(db_path);
    // The FTS5 index may or may not have anything depending on indexing
    // state, so we only verify the adapter doesn't error and either
    // returns hits or an empty vec.
    let result = adapter.search("alpha", 10);
    assert!(result.is_ok(), "FTS5 adapter must not error: {result:?}");
    // We can't assert hits are present without knowing the index state, but
    // we can assert the result is a valid `Vec<SearchHit>`.
    let _ = result.unwrap();
}

#[test]
fn save_exploration_round_trips_via_generate_artifact() {
    let Some((mut service, graph_arc)) = (|| -> Option<(ExplorerService, Arc<CallGraph>)> {
        // Build with FTS5 wired in when the DB exists.
        let db_path = workspace_db()?;
        let cwd = db_path
            .parent()
            .and_then(|p| p.parent())
            .unwrap()
            .to_path_buf();
        let store = SqliteGraphStore::open(&db_path).ok()?;
        let graph = store.load_graph().ok()??;
        let graph_arc = Arc::new(graph);
        let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph_arc.clone()));
        let reader = Arc::new(FsSourceReader::new(cwd.clone()));
        let fts5: Option<Arc<dyn cognicode_explorer::ports::SearchRepository>> =
            Some(Arc::new(Fts5SearchAdapter::new(db_path)));
        let service = ExplorerService::with_search(repo, reader, cwd.clone(), fts5);
        Some((service, graph_arc))
    })() else {
        eprintln!("[integration] workspace DB empty or missing — skipping");
        return;
    };
    // Silence unused mut warning
    let _ = &mut service;

    // Pick a real symbol from the graph.
    let (id, sym) = graph_arc
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let mvp_id = format!(
        "symbol:{}:{}:{}",
        sym.location().file(),
        sym.name(),
        sym.location().line()
    );
    let _ = id;

    // Save the exploration with two columns — one real, one duplicate.
    let path = service
        .save_exploration(SaveExplorationRequest {
            workspace_id: "workspace:test".into(),
            columns: vec![
                ExplorationColumn {
                    object_id: mvp_id.clone(),
                    active_view: Some("overview".into()),
                },
                ExplorationColumn {
                    object_id: mvp_id.clone(),
                    active_view: None,
                },
            ],
            lens: None,
        })
        .expect("save_exploration ok");
    assert_eq!(path.objects.len(), 1, "duplicate columns must dedupe");
    let first_obj = &path.objects[0];
    assert_eq!(first_obj.id, mvp_id);
    assert_eq!(first_obj.object_type, "symbol");
    assert!(first_obj.natural_key.contains(':'));

    // Generate the JSON replay and verify the objects array is populated.
    let summary = service
        .generate_artifact(
            &path.id,
            GenerateArtifactRequest {
                format: ArtifactFormat::JsonReplay,
            },
        )
        .expect("generate_artifact ok");
    let body: serde_json::Value = serde_json::from_str(&summary.content).expect("valid json");
    let objects = body["objects"].as_array().expect("objects array");
    assert_eq!(
        objects.len(),
        1,
        "JSON replay must include the resolved object"
    );
    assert_eq!(objects[0]["id"], mvp_id);
    assert_eq!(objects[0]["object_type"], "symbol");
}

#[test]
fn evidence_blocks_carry_freshness_signal() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] workspace DB empty or missing — skipping");
        return;
    };
    let (_, sym) = graph_arc
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let mvp_id = format!(
        "symbol:{}:{}:{}",
        sym.location().file(),
        sym.name(),
        sym.location().line()
    );
    let view = service
        .contextual_view(&mvp_id, "evidence")
        .expect("evidence view ok");
    assert!(!view.evidence.is_empty());
    // Every freshly-built block carries a freshness signal.
    for block in &view.evidence {
        let kind = block.kind.as_str();
        let f = block.freshness.as_deref();
        match kind {
            "symbol_metadata" | "call_graph" => {
                assert_eq!(f, Some("unknown"), "{kind} freshness");
            }
            "source_file" | "fs_index" => {
                assert!(
                    f == Some("fresh") || f == Some("stale"),
                    "{kind} freshness should be fresh|stale, got {f:?}"
                );
            }
            _ => {} // future kinds can carry whatever they want
        }
    }
}

// ---------------------------------------------------------------------------
// Phase 2 — File and Scope inspection
// ---------------------------------------------------------------------------

#[test]
fn inspect_file_against_real_db_returns_file_summary() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    // Pick the first file in the graph that has at least one symbol.
    let first_file = graph_arc
        .symbol_ids()
        .next()
        .map(|(_, s)| s.location().file().to_string())
        .expect("at least one symbol indexed");
    let mvp_id = format!("file:{first_file}");

    let summary = service.inspect_object(&mvp_id).expect("file inspect ok");
    assert_eq!(summary.id, mvp_id);
    assert!(matches!(summary.object_type, InspectableObjectType::File));
    let ids: Vec<&str> = summary
        .available_views
        .iter()
        .map(|v| v.id.as_str())
        .collect();
    assert_eq!(ids, vec!["overview", "symbols", "quality"]);
    // Must carry the path / counts in properties.
    let keys: std::collections::HashSet<&str> =
        summary.properties.iter().map(|p| p.key.as_str()).collect();
    assert!(keys.contains("path"));
    assert!(keys.contains("line_count"));
    assert!(keys.contains("symbol_count"));
    assert!(keys.contains("kinds"));
}

#[test]
fn inspect_scope_against_real_db_returns_scope_summary() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    // Use the first module path as the scope id.
    let first_module = graph_arc
        .modules()
        .into_iter()
        .next()
        .expect("at least one module indexed");
    let mvp_id = format!("scope:{first_module}");

    let summary = service.inspect_object(&mvp_id).expect("scope inspect ok");
    assert_eq!(summary.id, mvp_id);
    assert!(matches!(summary.object_type, InspectableObjectType::Scope));
    let ids: Vec<&str> = summary
        .available_views
        .iter()
        .map(|v| v.id.as_str())
        .collect();
    assert_eq!(ids, vec!["overview", "dependencies", "hotspots", "quality"]);
    // `promotion_ready` is always `false` in Phase 2.
    let promotion = summary
        .properties
        .iter()
        .find(|p| p.key == "promotion_ready")
        .expect("promotion_ready property present");
    assert_eq!(promotion.value, serde_json::json!(false));
}

#[test]
fn contextual_view_file_symbols_against_real_db() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    let first_file = graph_arc
        .symbol_ids()
        .next()
        .map(|(_, s)| s.location().file().to_string())
        .expect("at least one symbol indexed");
    let mvp_id = format!("file:{first_file}");

    let view = service
        .contextual_view(&mvp_id, "symbols")
        .expect("file symbols view ok");
    assert_eq!(view.view_id, "symbols");
    // The graph has at least one symbol in this file (we picked it from
    // there), so the relations vec must be non-empty.
    assert!(
        !view.relations.is_empty(),
        "file symbols view must emit at least one CONTAINS relation"
    );
    for rel in &view.relations {
        assert_eq!(rel.relation_type, "CONTAINS");
        assert!(rel.target_object_id.starts_with("symbol:"));
    }
    // Evidence block carries a freshness signal.
    assert!(!view.evidence.is_empty());
    assert!(view.evidence[0].freshness.is_some());
}

#[test]
fn contextual_view_scope_hotspots_against_real_db() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    let first_module = graph_arc
        .modules()
        .into_iter()
        .next()
        .expect("at least one module indexed");
    let mvp_id = format!("scope:{first_module}");

    let view = service
        .contextual_view(&mvp_id, "hotspots")
        .expect("scope hotspots view ok");
    assert_eq!(view.view_id, "hotspots");
    // Hotspots view is well-formed regardless of fan_in: the relations
    // vec is non-empty when the scope has at least one member symbol.
    let member_count = graph_arc
        .symbol_ids()
        .filter(|(_, s)| {
            s.location().file().starts_with(&format!("{first_module}/"))
                || s.location().file() == first_module
        })
        .count();
    if member_count > 0 {
        assert!(!view.relations.is_empty());
    }
    assert!(!view.evidence.is_empty());
    assert!(view.evidence[0].freshness.is_some());
}

// ---------------------------------------------------------------------------
// Phase 3 — Quality lens
// ---------------------------------------------------------------------------

use cognicode_explorer::adapters::SqliteQualityAdapter;
use cognicode_explorer::ports::quality_repository::QualityRepository;

#[test]
fn inspect_issue_against_real_db_returns_summary() {
    let Some(_db) = workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };
    let db_path = _db;
    let cwd = db_path
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf();

    // Build a service that also has the quality backend wired. The
    // service does NOT need a graph blob for issue/rule inspection —
    // pass an empty graph to keep the test focused on quality.
    let store = SqliteGraphStore::open(&db_path).expect("open db");
    let graph = store.load_graph().ok().flatten().unwrap_or_default();
    let graph_arc = Arc::new(graph);
    let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph_arc));
    let reader = Arc::new(FsSourceReader::new(cwd.clone()));
    let quality: Option<Arc<dyn QualityRepository>> =
        Some(Arc::new(SqliteQualityAdapter::new(db_path.clone())));
    let service = ExplorerService::with_all(repo, reader, cwd, None, quality);

    // Probe for any open issue in the DB; if none, skip — the contract
    // is "pass when the DB is loaded", not "issues must be present".
    let adapter = SqliteQualityAdapter::new(db_path);
    let open_count = adapter.open_issues_count().expect("open count");
    if open_count == 0 {
        eprintln!(
            "[integration] no open issues in the workspace DB — skipping issue inspection test"
        );
        return;
    }

    // Try ids 1..=count+1; first hit wins. The first id is sufficient
    // because the contract only requires that the inspect pipeline
    // returns a summary for a real issue.
    let target_id = (1..=open_count as i64 + 1)
        .find(|id| adapter.issue_by_id(*id).ok().flatten().is_some())
        .expect("at least one issue id should resolve");
    let mvp = format!("issue:{target_id}");

    let summary = service.inspect_object(&mvp).expect("issue inspect ok");
    assert_eq!(summary.id, mvp);
    assert!(matches!(
        summary.object_type,
        cognicode_explorer::dto::InspectableObjectType::QualityIssue
    ));
    let ids: Vec<&str> = summary
        .available_views
        .iter()
        .map(|v| v.id.as_str())
        .collect();
    assert_eq!(ids, vec!["overview"]);
}

#[test]
fn inspect_rule_against_real_db_returns_summary() {
    let Some(_db) = workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };
    let db_path = _db;
    let cwd = db_path
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf();

    let store = SqliteGraphStore::open(&db_path).expect("open db");
    let graph = store.load_graph().ok().flatten().unwrap_or_default();
    let graph_arc = Arc::new(graph);
    let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph_arc));
    let reader = Arc::new(FsSourceReader::new(cwd.clone()));
    let quality: Option<Arc<dyn QualityRepository>> =
        Some(Arc::new(SqliteQualityAdapter::new(db_path.clone())));
    let service = ExplorerService::with_all(repo, reader, cwd, None, quality);

    // Use any known rule id from the DB.
    let adapter = SqliteQualityAdapter::new(db_path);
    // Walk the first 50 issue ids and pick the first non-empty rule_id.
    let target_rule: Option<String> = (1..=50i64)
        .filter_map(|id| adapter.issue_by_id(id).ok().flatten())
        .map(|i| i.rule_id)
        .next();
    let Some(rule_id) = target_rule else {
        eprintln!("[integration] no issues in DB to extract a rule id — skipping");
        return;
    };
    let mvp = format!("rule:{rule_id}");

    let summary = service.inspect_object(&mvp).expect("rule inspect ok");
    assert_eq!(summary.id, mvp);
    assert!(matches!(
        summary.object_type,
        cognicode_explorer::dto::InspectableObjectType::Rule
    ));
    let ids: Vec<&str> = summary
        .available_views
        .iter()
        .map(|v| v.id.as_str())
        .collect();
    assert_eq!(ids, vec!["overview"]);
}

#[test]
fn available_views_includes_quality_for_symbol_file_scope() {
    let Some(_db) = workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };
    let db_path = _db;
    let cwd = db_path
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf();

    let store = SqliteGraphStore::open(&db_path).expect("open db");
    let graph = match store.load_graph().ok().flatten() {
        Some(g) => g,
        None => {
            eprintln!("[integration] workspace DB has no graph blob — skipping");
            return;
        }
    };
    let graph_arc = Arc::new(graph.clone());
    let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph_arc));
    let reader = Arc::new(FsSourceReader::new(cwd.clone()));
    let service = ExplorerService::new(repo, reader, cwd);

    // Pick the first symbol and the first module from the real DB.
    let first_sym = graph
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let sym_mvp = format!(
        "symbol:{}:{}:{}",
        first_sym.1.location().file(),
        first_sym.1.name(),
        first_sym.1.location().line()
    );
    let first_module = graph
        .modules()
        .into_iter()
        .next()
        .expect("at least one module");
    let scope_mvp = format!("scope:{first_module}");
    let file_mvp = format!("file:{}", first_sym.1.location().file());

    // Each of the three must include "quality" in their descriptor list.
    for mvp in [&sym_mvp, &file_mvp, &scope_mvp] {
        let views = service.available_views(mvp).expect("available_views ok");
        let ids: Vec<&str> = views.iter().map(|v| v.id.as_str()).collect();
        assert!(
            ids.contains(&"quality"),
            "object {mvp} must expose 'quality' view, got {ids:?}"
        );
    }
}

#[test]
fn contextual_view_quality_against_real_db_returns_view() {
    let Some(_db) = workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };
    let db_path = _db;
    let cwd = db_path
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf();

    let store = SqliteGraphStore::open(&db_path).expect("open db");
    let graph = match store.load_graph().ok().flatten() {
        Some(g) => g,
        None => {
            eprintln!("[integration] workspace DB has no graph blob — skipping");
            return;
        }
    };
    let graph_arc = Arc::new(graph.clone());
    let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph_arc));
    let reader = Arc::new(FsSourceReader::new(cwd.clone()));
    let quality: Option<Arc<dyn QualityRepository>> =
        Some(Arc::new(SqliteQualityAdapter::new(db_path)));
    let service = ExplorerService::with_all(repo, reader, cwd, None, quality);

    let first_sym = graph
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let mvp = format!(
        "symbol:{}:{}:{}",
        first_sym.1.location().file(),
        first_sym.1.name(),
        first_sym.1.location().line()
    );

    // The "quality" view must be servable (it may be empty if no issues
    // exist at this symbol's line, but the call must not error).
    let view = service
        .contextual_view(&mvp, "quality")
        .expect("quality view ok");
    assert_eq!(view.view_id, "quality");
    assert!(
        !view.evidence.is_empty(),
        "quality view must carry evidence"
    );
    assert_eq!(view.evidence[0].kind, "quality_finding");
}

// ---------------------------------------------------------------------------
// Phase 4 — Design Lenses
// ---------------------------------------------------------------------------

#[test]
fn available_lenses_for_real_object_returns_three() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    let (_, sym) = graph_arc
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let mvp = format!(
        "symbol:{}:{}:{}",
        sym.location().file(),
        sym.name(),
        sym.location().line()
    );

    let lenses = service.available_lenses(&mvp).expect("ok");
    let ids: Vec<String> = lenses.iter().map(|d| d.id.clone()).collect();
    assert!(
        ids.contains(&"hotspots".to_string()),
        "hotspots lens must be available, got: {ids:?}"
    );
    assert!(
        ids.contains(&"dependencies".to_string()),
        "dependencies lens must be available, got: {ids:?}"
    );
    assert!(
        ids.contains(&"architecture".to_string()),
        "architecture lens must be available, got: {ids:?}"
    );
    // Every descriptor carries an applicable_types list — sanity check
    // that the response is shaped correctly.
    for d in &lenses {
        assert!(!d.name.is_empty());
        assert!(!d.description.is_empty());
    }
}

#[test]
fn apply_hotspots_lens_to_real_symbol_returns_lens_result() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    let (_, sym) = graph_arc
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let mvp = format!(
        "symbol:{}:{}:{}",
        sym.location().file(),
        sym.name(),
        sym.location().line()
    );

    let result = service
        .apply_lens(&mvp, "hotspots")
        .expect("hotspots apply ok");
    assert_eq!(result.lens_id, "hotspots");
    // The result shape is correct even when zero findings — a real symbol
    // may not be a hotspot.
    assert!(
        result.findings.len() <= 20,
        "must respect the 20-finding cap"
    );
    // Every finding, if any, carries the lens id and a hypothesis string.
    for f in &result.findings {
        assert_eq!(f.lens_id, "hotspots");
        assert!(!f.hypothesis.is_empty());
        assert!(!f.title.is_empty());
        assert!((0.0..=1.0).contains(&f.confidence));
    }
}

#[test]
fn apply_dependencies_lens_to_real_scope_runs_without_error() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    let first_module = graph_arc
        .modules()
        .into_iter()
        .next()
        .expect("at least one module indexed");
    let mvp = format!("scope:{first_module}");

    let result = service
        .apply_lens(&mvp, "dependencies")
        .expect("dependencies apply ok");
    assert_eq!(result.lens_id, "dependencies");
    assert!(result.findings.len() <= 20);
    // Every finding is tagged with the lens and carries object_ids the
    // caller can cross-link to.
    for f in &result.findings {
        assert_eq!(f.lens_id, "dependencies");
        assert!(!f.hypothesis.is_empty());
    }
}

#[test]
fn apply_architecture_lens_to_real_scope_runs_without_error() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    let first_module = graph_arc
        .modules()
        .into_iter()
        .next()
        .expect("at least one module indexed");
    let mvp = format!("scope:{first_module}");

    let result = service
        .apply_lens(&mvp, "architecture")
        .expect("architecture apply ok");
    assert_eq!(result.lens_id, "architecture");
    assert!(result.findings.len() <= 20);
    for f in &result.findings {
        assert_eq!(f.lens_id, "architecture");
        assert!(!f.hypothesis.is_empty());
    }
}

#[test]
fn apply_unknown_lens_returns_resolution_failed() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found — skipping");
        return;
    };

    let (_, sym) = graph_arc
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let mvp = format!(
        "symbol:{}:{}:{}",
        sym.location().file(),
        sym.name(),
        sym.location().line()
    );

    let err = service
        .apply_lens(&mvp, "no-such-lens")
        .expect_err("unknown lens must error");
    assert!(matches!(
        err,
        cognicode_explorer::ExplorerError::ResolutionFailed(_)
    ));
}

// ---------------------------------------------------------------------------
// Phase 5 — MCP wiring
// ---------------------------------------------------------------------------
//
// These tests are cheap smoke-tests for the MCP surface. The actual
// stdio server loop is exercised by the end-to-end binary run
// (cognicode-explorer-mcp), which the test build already links.

/// `cognicode_explorer::mcp::ExplorerMcpHandler` must be re-exported from
/// the crate root — the MCP binary and any external agent harness
/// construct it through the public API.
#[test]
fn mcp_handler_re_exported_from_lib_root() {
    let _constructor: fn(
        std::sync::Arc<cognicode_explorer::service::ExplorerService>,
    ) -> cognicode_explorer::ExplorerMcpHandler = cognicode_explorer::ExplorerMcpHandler::new;
}

/// The MCP server's binary target must link successfully. Cargo's test
/// build already produces `target/debug/cognicode-explorer-mcp`; we
/// just verify the artifact path resolves when the workspace has been
/// built at least once.
#[test]
fn mcp_binary_link_artifact_resolves() {
    // The artifact is the MCP binary built by `cargo build`. The test
    // build links it as part of its own compilation, so by the time the
    // test runs the binary is on disk. We look for it next to the test
    // binary's directory.
    let test_exe = std::env::current_exe().expect("current_exe");
    let target_dir = test_exe
        .parent()
        .and_then(|p| p.parent())
        .expect("target/debug parent");
    let mcp_bin = target_dir.join("cognicode-explorer-mcp");
    assert!(
        mcp_bin.exists(),
        "expected MCP binary at {} (test exe: {})",
        mcp_bin.display(),
        test_exe.display()
    );
}

/// The tool name constants must match the canonical list shipped in
/// `src/mcp.rs::TOOL_NAMES` — the count includes the 4 named-views
/// tools (`view_save` / `view_load` / `view_list` / `view_delete`)
/// that landed after this test was first written. Regression guard:
/// any rename in `src/mcp.rs` is flagged here so a downstream agent
/// harness never silently breaks.
#[test]
fn mcp_tool_names_match_spec() {
    use cognicode_explorer::mcp::{
        TOOL_APPLY_LENS, TOOL_ASK, TOOL_BRAIN_ASK, TOOL_BRAIN_ATTACH, TOOL_BRAIN_CLOSE,
        TOOL_BRAIN_FOCUS, TOOL_BRAIN_OPEN, TOOL_BRAIN_STATUS, TOOL_GET_LENSES, TOOL_GET_VIEW,
        TOOL_GET_VIEWS, TOOL_GRAPH_CLUSTER, TOOL_GRAPH_EXPLAIN, TOOL_GRAPH_SUBGRAPH,
        TOOL_IMPACT_COMPONENT, TOOL_IMPACT_DETECT_CYCLES, TOOL_IMPACT_FORWARD_RADIUS,
        TOOL_IMPACT_HAS_PATH, TOOL_IMPACT_RADIUS, TOOL_IMPACT_SHORTEST_PATH, TOOL_INSPECT_OBJECT,
        TOOL_OPEN_WORKSPACE, TOOL_QUERY_MOLDQL, TOOL_SPOTTER_SEARCH, TOOL_VIEW_DELETE,
        TOOL_VIEW_LIST, TOOL_VIEW_LOAD, TOOL_VIEW_SAVE,
    };

    let expected = [
        TOOL_OPEN_WORKSPACE,
        TOOL_SPOTTER_SEARCH,
        TOOL_INSPECT_OBJECT,
        TOOL_GET_VIEWS,
        TOOL_GET_VIEW,
        TOOL_GET_LENSES,
        TOOL_APPLY_LENS,
        TOOL_QUERY_MOLDQL,
        TOOL_IMPACT_RADIUS,
        TOOL_IMPACT_HAS_PATH,
        TOOL_IMPACT_SHORTEST_PATH,
        TOOL_IMPACT_DETECT_CYCLES,
        TOOL_IMPACT_COMPONENT,
        TOOL_IMPACT_FORWARD_RADIUS,
        TOOL_GRAPH_SUBGRAPH,
        TOOL_GRAPH_CLUSTER,
        TOOL_GRAPH_EXPLAIN,
        TOOL_ASK,
        TOOL_BRAIN_OPEN,
        TOOL_BRAIN_ATTACH,
        TOOL_BRAIN_ASK,
        TOOL_BRAIN_FOCUS,
        TOOL_BRAIN_STATUS,
        TOOL_BRAIN_CLOSE,
        TOOL_VIEW_SAVE,
        TOOL_VIEW_LOAD,
        TOOL_VIEW_LIST,
        TOOL_VIEW_DELETE,
    ];
    let actual = cognicode_explorer::mcp::tool_names();
    // 28 with the default features, 29 with `multimodal`
    // (docs_ingest is added by the multimodal feature).
    let expected_count = if cfg!(feature = "multimodal") { 29 } else { 28 };
    assert_eq!(actual.len(), expected_count);
    for name in expected {
        assert!(
            actual.contains(&name),
            "tool_names() missing `{}` — got: {:?}",
            name,
            actual
        );
    }
    // When the multimodal feature is active, `docs_ingest` MUST
    // be in the list — see the RED-gate test
    // `tool_schemas_docs_ingest_hidden_without_feature` in
    // `mcp.rs` for the cross-check. We resolve the constant
    // through the same import path that the test would use at
    // runtime; the constant is only present on a multimodal
    // build, so the import is gated the same way as the call.
    #[cfg(feature = "multimodal")]
    {
        use cognicode_explorer::mcp::TOOL_DOCS_INGEST;
        assert!(
            actual.contains(&TOOL_DOCS_INGEST),
            "multimodal build must include docs_ingest in tool_names()"
        );
    }
}

// ============================================================================
// Phase 6 — MoldQL end-to-end integration
// ============================================================================

/// Execute a real MoldQL query against a real workspace DB and confirm
/// the executor surfaces the indexed symbols. Skipped when no DB is
/// present — the test contract is "pass when an indexed workspace
/// exists".
#[test]
fn moldql_find_symbols_against_real_db() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found or empty — skipping");
        return;
    };

    let first = graph_arc
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let expected_name = first.1.name().to_string();

    // A no-filter FIND symbols must surface at least the seed symbol.
    let result = service
        .execute_query("FIND symbols")
        .expect("FIND symbols ok");
    assert!(
        result
            .items
            .iter()
            .any(|i| i.label.starts_with(&expected_name)),
        "FIND symbols must include `{}` in results, got: {:?}",
        expected_name,
        result.items
    );
    assert!(!result.query.is_empty(), "query should be echoed back");
}

/// `FIND files IN SCOPE ...` exercises the scope clause against the
/// real graph. The scope is taken from the file path of the first
/// indexed symbol so the test is self-consistent.
#[test]
fn moldql_find_files_in_scope_against_real_db() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found or empty — skipping");
        return;
    };

    let first = graph_arc
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let first_file = first.1.location().file();
    // Use the parent directory so the scope is a directory, not a file.
    let scope = std::path::Path::new(&first_file)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| first_file.to_string());

    let result = service
        .execute_query(&format!("FIND files IN SCOPE {scope}"))
        .expect("FIND files ok");
    // At least one file in the scope must match (the seed's own file).
    assert!(
        result.items.iter().any(|i| i.label == first_file),
        "expected file `{}` in scope `{}`, got: {:?}",
        first_file,
        scope,
        result.items
    );
}

/// `EXPLORE ... THROUGH callees DEPTH 0` returns the seed symbol
/// unchanged — the BFS does not advance. This guards the BFS plumbing
/// end-to-end.
#[test]
fn moldql_explore_zero_depth_against_real_db() {
    let Some((service, graph_arc)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found or empty — skipping");
        return;
    };

    let first = graph_arc
        .symbol_ids()
        .next()
        .expect("at least one symbol indexed");
    let mvp_id = format!(
        "symbol:{}:{}:{}",
        first.1.location().file(),
        first.1.name(),
        first.1.location().line()
    );

    let result = service
        .execute_query(&format!("EXPLORE {mvp_id} THROUGH callees DEPTH 0"))
        .expect("EXPLORE ok");
    assert_eq!(result.total, 1);
    assert_eq!(result.items[0].object_id, mvp_id);
}

/// Malformed MoldQL queries must surface a `ResolutionFailed` error
/// — the parser wraps `ParseError` so the service contract holds.
#[test]
fn moldql_invalid_query_errors() {
    let Some((service, _)) = build_service_from_workspace_db() else {
        eprintln!("[integration] .cognicode/cognicode.db not found or empty — skipping");
        return;
    };
    let err = service
        .execute_query("FOO bar")
        .expect_err("malformed query must error");
    assert!(matches!(
        err,
        cognicode_explorer::ExplorerError::ResolutionFailed(_)
    ));
}
