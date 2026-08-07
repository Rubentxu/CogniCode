# Tasks: multimodal-docs-source

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~1800-2300 (12 new files, 9 modified) |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR1 Foundation → PR2 Pipeline → PR3 Surface |
| Delivery strategy | ask-on-risk |
| Chain strategy | pending |

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: stacked-to-main|feature-branch-chain|size-exception|pending
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | Likely PR | Base branch | Notes |
|------|------|-----------|-------------|-------|
| 1 | Domain types + feature gate (T1-T6) | PR1 | `main` | No runtime deps; tests pure |
| 2 | PG layer + docs pipeline (T7-T15) | PR2 | `main` (or PR1) | Needs `pulldown-cmark` + sqlx PG |
| 3 | Query, frontend, search, regression (T16-T23) | PR3 | `main` (or PR2) | Frontend TS + ExplorerQL + MCP |

## Phase 1: Foundation (Batch 1)

### T1: Add `multimodal` feature gate + module wiring
- **Batch**: 1
- **Depends on**: —
- **Spec reqs covered**: R1.7
- **RED gate**: `cargo build -p cognicode-core --no-default-features` succeeds; the new modules do not exist yet. Then: add `mod node_kind;` under `#[cfg(feature = "multimodal")]` in `domain/value_objects/mod.rs` and re-run — must still build clean.
- **Files to create/modify**: `crates/cognicode-core/Cargo.toml`, `crates/cognicode-explorer/Cargo.toml`, `crates/cognicode-core/src/domain/value_objects/mod.rs`, `crates/cognicode-core/src/domain/aggregates/mod.rs`, `crates/cognicode-core/src/lib.rs`
- **Estimated LOC**: 25

### T2: `NodeKind` enum
- **Batch**: 1
- **Depends on**: T1
- **Spec reqs covered**: R1.1
- **RED gate**: Test `node_kind_roundtrips_all_5_variants` constructs each variant, JSON-serializes, deserializes, asserts equality. Then: add enum with `#[serde(tag="type", content="value")]` derive set `Debug,Clone,PartialEq,Eq,Hash,Serialize,Deserialize`.
- **Files to create/modify**: `crates/cognicode-core/src/domain/value_objects/node_kind.rs` (new)
- **Estimated LOC**: 35

### T3: `EdgeKind` enum
- **Batch**: 1
- **Depends on**: T1
- **Spec reqs covered**: R1.3
- **RED gate**: Test `edge_kind_roundtrips_all_5_variants` covers `Dependency(DependencyType::Calls)`, `Cites`, `Justifies`, `Resolves`, `CorroboratedBy` with JSON roundtrip.
- **Files to create/modify**: `crates/cognicode-core/src/domain/value_objects/edge_kind.rs` (new)
- **Estimated LOC**: 25

### T4: `NodeId` newtype + constructors
- **Batch**: 1
- **Depends on**: T2
- **Spec reqs covered**: R1.2
- **RED gate**: 7 tests in `mod tests`: 4 well-formed (`NodeId::symbol`, `NodeId::doc`, `NodeId::decision`, `NodeId::issue`, `NodeId::evidence`) + 3 malformed (`Empty`, `MalformedFormat`, mismatched kind).
- **Files to create/modify**: `crates/cognicode-core/src/domain/value_objects/node_id.rs` (new)
- **Estimated LOC**: 90

### T5: `GraphNode` aggregate
- **Batch**: 1
- **Depends on**: T2, T4
- **Spec reqs covered**: R1.5
- **RED gate**: Test `graph_node_constructs_and_json_roundtrip` builds node with `serde_json::json!({"k":"v"})` metadata, asserts id/kind/label/source_path/metadata all roundtrip.
- **Files to create/modify**: `crates/cognicode-core/src/domain/aggregates/graph_node.rs` (new)
- **Estimated LOC**: 30

### T6: `GraphEdge` aggregate
- **Batch**: 1
- **Depends on**: T3, T4
- **Spec reqs covered**: R1.4
- **RED gate**: 5 tests: `confidence=0.0` OK, `1.0` OK, `0.5` OK, `1.5` returns `Err(ConfidenceOutOfRange)`, `f64::NAN` returns `Err(ConfidenceNotFinite)`. Plus construction test with multimodal kinds.
- **Files to create/modify**: `crates/cognicode-core/src/domain/aggregates/graph_edge.rs` (new)
- **Estimated LOC**: 60

## Phase 2: Persistence + Docs Pipeline (Batch 2)

### T7: PG migration `m0009_graph_nodes_edges`
- **Batch**: 2
- **Depends on**: T1
- **Spec reqs covered**: R1.6
- **RED gate**: Integration test starts empty PG, runs migration, asserts `graph_nodes` and `graph_edges` exist with expected columns/primary keys/indexes; asserts `symbols`/`call_edges` row counts unchanged when pre-seeded.
- **Files to create/modify**: `crates/cognicode-core/src/infrastructure/persistence/schema_postgres.sql` (append DDL), `crates/cognicode-core/src/infrastructure/persistence/m0009_graph_nodes_edges.sql` (new, included via `include_str!`)
- **Estimated LOC**: 60

### T8: `GraphRepository` port trait
- **Batch**: 2
- **Depends on**: T5, T6
- **Spec reqs covered**: R1.7 (port)
- **RED gate**: Compile-time test: `let r: Box<dyn GraphRepository> = Box::new(...)` — does not yet compile (trait missing). Add trait with 5 async methods + `Send+Sync` bound, dyn-compatible, no generic methods.
- **Files to create/modify**: `crates/cognicode-explorer/src/ports/graph_repository.rs` (new)
- **Estimated LOC**: 70

### T9: `PostgresGraphRepository` impl
- **Batch**: 2
- **Depends on**: T7, T8
- **Spec reqs covered**: R1.7 (impl)
- **RED gate**: 4 `sqlx::test` PG cases: `upsert_node_then_find_by_kind_returns_it`, `upsert_edge_then_find_edges_returns_it`, `find_incoming_edges_returns_3_inbound`, `pk_collision_upsert_overwrites`.
- **Files to create/modify**: `crates/cognicode-core/src/infrastructure/persistence/generic_graph_repository.rs` (new)
- **Estimated LOC**: 180

### T10: `DocsConfidenceRules`
- **Batch**: 2
- **Depends on**: T1
- **Spec reqs covered**: R2.2
- **RED gate**: 4 tests asserting exact confidence+provenance: `link_exact=0.9/Extracted`, `heading_match=0.7/Extracted`, `link_fuzzy=0.6/Ambiguous`, `unresolved=0.3/Ambiguous`.
- **Files to create/modify**: `crates/cognicode-core/src/domain/services/docs_confidence.rs` (new)
- **Estimated LOC**: 50

### T11: `SourceExtractor` trait + `SourcePath` + `ExtractedNode`
- **Batch**: 2
- **Depends on**: T5, T6
- **Spec reqs covered**: R2.1
- **RED gate**: 2 tests: `Box<dyn SourceExtractor> = Box::new(MockExtractor)` compiles; `tokio::spawn(async move { extractor.extract(path).await })` compiles (Send+Sync bound).
- **Files to create/modify**: `crates/cognicode-explorer/src/ports/source_extractor.rs` (new)
- **Estimated LOC**: 55

### T12: `DocsExtractor` (markdown + ADR)
- **Batch**: 2
- **Depends on**: T10, T11
- **Spec reqs covered**: R2.3
- **RED gate**: 6 fixture tests in `mod tests`: `plain_md_produces_doc_node`, `adr_front_matter_produces_decision_node`, `mixed_links_resolve_with_tiered_confidence`, `code_fence_preserves_lang_in_metadata`, `no_headings_emits_filename_doc`, `circular_link_does_not_loop`.
- **Files to create/modify**: `crates/cognicode-core/src/domain/services/docs_extractor.rs` (new), `crates/cognicode-core/Cargo.toml` (add `pulldown-cmark` under `multimodal` feature)
- **Estimated LOC**: 220

### T13: `DocsSourceAdapter` (filesystem walk + idempotency)
- **Batch**: 2
- **Depends on**: T9, T12
- **Spec reqs covered**: R2.6
- **RED gate**: 3 tests: `walks_recursive_and_skips_non_md`, `re_ingest_same_files_yields_zero_new_nodes`, `invalid_utf8_file_logged_and_skipped`.
- **Files to create/modify**: `crates/cognicode-explorer/src/adapters/docs_source_adapter.rs` (new)
- **Estimated LOC**: 110

### T14: CLI command `cognicode ingest-docs`
- **Batch**: 2
- **Depends on**: T13
- **Spec reqs covered**: R2.5
- **RED gate**: 2 integration tests: `cognicode ingest-docs docs` → exit 0 with summary table; mixed valid+invalid-UTF8 → exit 1, valid file ingested, invalid logged to stderr.
- **Files to create/modify**: `crates/cognicode-cli/src/main.rs` (or `cognicode` crate dispatch)
- **Estimated LOC**: 90

### T15: MCP `docs_ingest` tool
- **Batch**: 2
- **Depends on**: T13
- **Spec reqs covered**: R2.4
- **RED gate**: 4 schema validation tests: valid `{paths,recursive}`, `paths:[]` rejected, missing `paths` rejected, `paths` not an array rejected. Plus end-to-end ingest test on a fixture dir.
- **Files to create/modify**: `crates/cognicode-explorer/src/mcp.rs` (gated by `#[cfg(feature="multimodal")]`)
- **Estimated LOC**: 120

## Phase 3: Query, Frontend, Search (Batch 3)

### T16: Backend `style_class_for` / `edge_style_class_for` extensions
- **Batch**: 3
- **Depends on**: T2, T3
- **Spec reqs covered**: R3.5
- **RED gate**: 8 unit tests in `api.rs` `mod tests`: 4 for new `NodeKind` mappings (`decision`/`doc`/`issue`/`evidence`), 4 for new `EdgeKind` mappings (`edge.cites`/`edge.justifies`/`edge.resolves`/`edge.corroborated_by`). Plus regression: existing 3+3 mappings unchanged.
- **Files to create/modify**: `crates/cognicode-explorer/src/api.rs`
- **Estimated LOC**: 35

### T17: Frontend Zod enum extensions (3→7)
- **Batch**: 3
- **Depends on**: T16 (string contract)
- **Spec reqs covered**: R3.1, R3.2
- **RED gate**: Vitest: `GraphNodeStyleClass.parse` for 7 OK + 1 unknown ZodError; `GraphEdgeStyleClass.parse` for 7 OK + 1 unknown. Legacy `function`/`module`/`external` and `edge.calls`/`edge.implements`/`edge.uses` still parse.
- **Files to create/modify**: `apps/explorer-ui/src/api/schemas.ts`, `apps/explorer-ui/src/api/schemas.test.ts`
- **Estimated LOC**: 30

### T18: Frontend Cytoscape stylesheet (4 node + 4 edge)
- **Batch**: 3
- **Depends on**: T17
- **Spec reqs covered**: R3.3
- **RED gate**: Vitest snapshot test: `stylesheet.ts` contains exactly one block per new class (`'node[style_class = "decision"]'`, `'edge[style_class = "edge.cites"]'`, etc.). `console.warn` regression test for unknown class still fires once.
- **Files to create/modify**: `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts`, `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.test.ts` (or extend existing test)
- **Estimated LOC**: 90

### T19: `ObjectInspector` multimodal fields
- **Batch**: 3
- **Depends on**: T18
- **Spec reqs covered**: R3.4
- **RED gate**: 4 Playwright/Vitest snapshot fixtures: Decision (purple badge, status+date table), Doc (green badge, Citations list with confidence, Provenance section), Issue, Evidence. Symbol node inspector unchanged.
- **Files to create/modify**: `apps/explorer-ui/src/components/ObjectInspector/ObjectInspector.tsx`, `apps/explorer-ui/src/components/ObjectInspector/ObjectInspector.test.tsx` (new)
- **Estimated LOC**: 130

### T20: ExplorerQL `TargetType` + parser (4→6)
- **Batch**: 3
- **Depends on**: T2
- **Spec reqs covered**: R4.1, R4.2
- **RED gate**: 5 parser tests: `FIND decisions` parses, `FIND docs` parses, `FIND decisions WHERE ...` parses, `FIND DECISIONS` (uppercase) parses, `FIND widgets` rejected with error listing all 6 valid targets. Plus `TargetType::keyword` for all 6 variants.
- **Files to create/modify**: `crates/cognicode-explorer/src/moldql/ast.rs`, `crates/cognicode-explorer/src/moldql/parser.rs`, `crates/cognicode-explorer/src/moldql/parser_explorerql.rs`
- **Estimated LOC**: 80

### T21: ExplorerQL compile dispatch + WHERE fields
- **Batch**: 3
- **Depends on**: T8, T20
- **Spec reqs covered**: R4.3, R4.4
- **RED gate**: 6 tests: `find_decisions_calls_find_nodes_by_kind_decision`, `find_docs_calls_find_nodes_by_kind_doc`, `repository_none_returns_repository_unavailable`, `where_decisions_status_filters_accepted`, `where_docs_section_filters_auth`, `unknown_field_color_rejected`. Plus 4 regression tests: existing 4 code targets compile byte-for-byte identical.
- **Files to create/modify**: `crates/cognicode-explorer/src/moldql/compile.rs`, `crates/cognicode-explorer/src/moldql/compile_fixtures.rs`
- **Estimated LOC**: 110

### T22: MCP `graph_search` tool
- **Batch**: 3
- **Depends on**: T9, T15
- **Spec reqs covered**: R5.2, R5.3, R5.4
- **RED gate**: 11 tests: 7 base (success, kind filter, limit cap, empty query rejected, empty graph returns [], multimodal rank > symbol rank, symbol-only rank) + 2 pagination (first page `next_cursor=Some`, last page `next_cursor=None`) + 1 cursor tampering + 1 FTS5 sanitization (`+`/`-`/`*` wrapped, unbalanced parens rejected).
- **Files to create/modify**: `crates/cognicode-explorer/src/mcp.rs` (add `graph_search` tool gated by `#[cfg(feature="multimodal")]`)
- **Estimated LOC**: 220

### T23: Integration + regression + feature-gate tests
- **Batch**: 3
- **Depends on**: T15, T22
- **Spec reqs covered**: R1.7 (feature-gate), R5.5 (backward compat)
- **RED gate**: 3 build-mode tests: `cargo build -p cognicode-core --no-default-features` succeeds and no new symbols exported; `cargo run -p cognicode-mcp --no-default-features` registers exactly 28 tools; `cargo run -p cognicode-mcp --features multimodal` registers 30 tools. Plus end-to-end: ingest 5 `.md` fixtures → `graph_search` finds the right kinds.
- **Files to create/modify**: `crates/cognicode-mcp/tests/multimodal_feature_gate.rs` (new), `crates/cognicode-explorer/tests/regression_28_tools.rs` (new)
- **Estimated LOC**: 140
