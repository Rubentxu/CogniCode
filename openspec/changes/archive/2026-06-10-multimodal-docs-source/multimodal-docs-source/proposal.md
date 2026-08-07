# Proposal: Multimodal Nodes + Docs Source Adapter

## Intent

Extend CogniCode's graph model from code-only nodes to heterogeneous nodes (Decision, Doc, Issue, Evidence) with a docs source adapter. Phase 4 of the Explorer Graph roadmap. Currently the graph only understands `Symbol` nodes — this change introduces a generic layer that preserves existing code-graph behavior while enabling documentation, ADR, and issue artifacts as first-class graph citizens.

## Scope

### In Scope
- Generic `NodeKind` enum wrapping `SymbolKind` + new types (Decision, Doc, Issue, Evidence)
- Generic `EdgeKind` enum: `Dependency(DependencyType)` + new kinds (Cites, Justifies, Resolves, CorroboratedBy)
- `GraphEdge` with `source`/`target` replacing `caller`/`callee` naming
- `NodeId` newtype replacing `SymbolId` in edge maps
- `SourceExtractor` trait for pluggable ingestion
- `DocsExtractor` implementing Markdown/ADR parsing with `DocsConfidenceRules`
- New PG tables: `graph_nodes`, `graph_edges` (additive alongside existing)
- `docs_ingest` MCP tool + CLI trigger
- ExplorerQL: new `TargetType` values (Decisions, Docs, Issues)
- Frontend: 4 new node shapes + 4 new edge styles with distinct colors

### Out of Scope
- Issue tracker adapter (Jira/GitHub Issues) — deferred to Phase 5
- Federation (cross-repo graph) — deferred
- Corroboration view (multi-source evidence aggregation) — deferred
- Rewrite of `CallGraph` aggregate — existing code-graph path untouched

## Capabilities

### New Capabilities
- `generic-graph-model`: `NodeKind`, `NodeId`, `EdgeKind`, `GraphEdge` domain types; `graph_nodes`/`graph_edges` PG tables; generic `GraphRepository` port
- `docs-source-adapter`: `SourceExtractor` trait, `DocsExtractor` for `.md`/ADR files, `DocsConfidenceRules` (link_exact=0.9, link_fuzzy=0.6, heading_match=0.7, unresolved=0.3), `docs_ingest` MCP tool + CLI
- `multimodal-frontend`: 4 node style classes (decision, doc, issue, evidence) + 4 edge style classes (cites, justifies, resolves, corroborated_by), Cytoscape shapes/colors

### Modified Capabilities
- `explorerql-targets`: `TargetType` enum adds Decisions, Docs, Issues variants
- `mcp-multimodal-tools`: New MCP tool `docs_ingest` in explorer group

## Approach

**Generic Graph Layer (additive)**. New domain types sit alongside existing ones. `NodeKind::Symbol(SymbolKind)` preserves exhaustive match arms. New PG tables (`graph_nodes`, `graph_edges`) coexist with `symbols`/`call_edges`. Adapter bridges old and new repositories. Feature-gated behind `multimodal` flag.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/` | New files | `graph_node.rs`, `graph_edge.rs`, `node_kind.rs`, `edge_kind.rs`, `node_id.rs` |
| `crates/cognicode-core/src/domain/services/` | New | `docs_extractor.rs`, `docs_confidence.rs` |
| `crates/cognicode-core/src/infrastructure/persistence/` | Modified | New PG tables, `generic_graph_repository.rs` |
| `crates/cognicode-explorer/src/ports/` | New | `graph_repository.rs`, `source_extractor.rs` |
| `crates/cognicode-explorer/src/adapters/` | New | `docs_source_adapter.rs` |
| `crates/cognicode-explorer/src/mcp.rs` | Modified | `docs_ingest` tool |
| `crates/cognicode-explorer/src/moldql/ast.rs` | Modified | `TargetType` extension |
| `crates/cognicode-explorer/src/dto.rs` | Modified | New style classes |
| `crates/cognicode-explorer/src/api.rs` | Modified | `style_class_for`/`edge_style_class_for` |
| `apps/explorer-ui/src/api/schemas.ts` | Modified | New Zod enums |
| `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts` | Modified | New node/edge styles |

## Entropy Budget (Protocol B)

**Method**: Heuristic (code reading from exploration)

| Metric | Estimate (bits) | Threshold | Status |
|--------|-----------------|-----------|--------|
| H(Δ_existing) | ~2.5 | < 1.0 | ❌ OCP strain |
| H(Δ_new) | ~2.3 | > 0 | ✅ |
| New connascence pairs | 4 | < 3 | ⚠️ |
| OCP compliant? | Partial | yes | ⚠️ |

**OCP Analysis**: H(Δ_existing) ≈ 2.5 bits because `dto.rs`, `api.rs`, `moldql/ast.rs`, `schemas.ts`, and `stylesheet.ts` must change. However, changes are additive (new enum variants, new match arms) — no existing behavior changes. The OCP "violation" is structural (files touched) not semantic (behavior preserved).

**New Connascence Pairs**:
| Pair | Type | I(bits) | Notes |
|------|------|---------|-------|
| `GraphEdge` ↔ `NodeId` | Type | ~1.0 | Generic, lower than SymbolId coupling |
| `GraphEdge` ↔ `EdgeKind` | Type | ~1.5 | New enum, contained |
| `SourceExtractor` ↔ `GraphEdge` | Type | ~1.0 | Trait-based, clean seam |
| Frontend style ↔ Backend DTO | Name | ~2.3 | Same pattern as existing (not worse) |

**Connascence Reduction**: `CallGraph↔SymbolId` (3.17 bits) partially replaced by `GraphEdge↔NodeId` (~1.0 bits) — net improvement for multimodal paths.

**Verdict**: 🟡 Yellow — additive OCP strain is acceptable for Phase 4 scope. Files touched are predictable and isolated.

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Dual-model confusion (CallGraph vs GenericGraph) | Medium | Feature-gate `multimodal`; clear naming conventions |
| `Evidence` name collision (DTO vs graph node) | Medium | Rename DTO variant to `EvidenceBlock` |
| Markdown link ambiguity | High | FTS5 fuzzy search; flag ambiguous edges with `Provenance::Ambiguous` |
| PG join performance (symbols + graph_nodes) | Low | Composite indexes; test with 10k+ nodes |

## Rollback Plan

1. Feature-gate: `#[cfg(feature = "multimodal")]` on all new code
2. PG tables are additive (no ALTER on existing) — `DROP TABLE graph_nodes, graph_edges` reverts schema
3. Existing `symbols`/`call_edges` tables and `CallGraph` aggregate untouched
4. Frontend falls back to existing 3+3 style classes when feature disabled

## Dependencies

- Phases 1-3 complete (28 MCP tools, ExplorerQL, Cytoscape.js graph, named views)
- `InspectableObjectType` already has `Evidence`/`DecisionArtifact` variants (DTO half-ready)
- `Provenance` enum is source-agnostic (no new variants needed)

## Open Questions (from Auto-Grill)

| # | Question | Recommended | Rationale |
|---|----------|-------------|-----------|
| 1 | Accept OCP strain (H(Δ_existing) ≈ 2.5 bits)? | **Yes** — changes are additive + feature-gated | Structural OCP, not semantic. OS=0.78 |
| 2 | New PG tables vs extend existing? | **New tables** (`graph_nodes`/`graph_edges`) | Additive DDL, zero risk. OS=0.82 |
| 3 | `SourceExtractor` as trait or enum? | **Trait** (dyn dispatch) | DIP compliance, OCP for future extractors. OS=0.85 |

Auto-grill report: `/tmp/sdd-multimodal-docs-source-auto-grill.html`

## Success Criteria

- [ ] `NodeKind` enum with 5 variants, all tests pass
- [ ] `graph_nodes`/`graph_edges` tables created, CRUD works via `GenericGraphRepository`
- [ ] `DocsExtractor` ingests `.md` files, produces nodes + edges with confidence scores
- [ ] `docs_ingest` MCP tool callable, returns ingestion summary
- [ ] ExplorerQL `FIND decisions` and `FIND docs` return results
- [ ] Frontend renders Decision/Doc/Issue/Evidence nodes with distinct shapes/colors
- [ ] Existing 28 MCP tools + ExplorerQL queries unaffected (regression tests green)
- [ ] H(Δ_existing) actual ≤ 2.5 bits (within budget)
