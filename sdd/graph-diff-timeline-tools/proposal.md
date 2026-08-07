# Kernel Proposal: Graph Diff & Timeline MCP Tools

## Intent
`GraphReport` is cached in `graph_reports` "for temporal diffing" (CONTEXT.md), yet nothing can read it back, and the Report stage currently crashes at insert time. Expose the stored history as two MCP tools — `graph_diff` (compare two snapshots) and `graph_timeline` (trend over N days) — and unblock them by fixing the persistence bug first.

## Context Gate
| Knowledge Coverage | Quality | Taxonomy | Extra Effort |
|--------------------|---------|----------|--------------|
| sufficient | C1 | persistence-bug + new-read-capability + tool-registration | verify |

## Knowledge Alignment
- Roadmap / Backlog: None explicit (no work item located).
- Work Items / Specs: `graph_reports` table is m0010 "Sprint 2 — created now for forward-compat".
- ADR / Architecture Sources: ADR-017 (PG-native manifest/persistence); CONTEXT.md `GraphReport` definition documents temporal diffing as the intent.
- Ownership Source: `report_stage.rs` owns persist; `consolidated_handlers::handle_compare_graph` is the existing diff stub; `graph_query_handlers::handle_get_graph_report` is the report stub.
- Prior Learnings: None in Engram (first pass).

## Knowledge Decisions
- Stays memory-only: None.
- Promote to durable knowledge: ADR recommended after design (record the UUID vs gen_random_uuid decision + why TEXT id was wrong).

## Lens Routing
| Lens | Delegation | Status | Proposal Impact |
|------|------------|--------|-----------------|
| base-discipline | kernel | applied | Scoped fix before feature; invariants pinned to no data loss; entropy kept to new read paths only |
| entropy-sdd | entropy-sdd/SKILL.md | skipped → verify in design | No new connascence introduced yet; quantified at design |

## Scope
### In Scope
- Fix `graph_reports.id` insert type mismatch (BLOCKING, must ship first).
- Add `PostgresRepository::load_latest_report(workspace_id)` and `load_report_range(workspace_id, days)`.
- Add `handle_graph_diff` and `handle_graph_timeline` handlers + input/output DTOs.
- Register both tools in `rmcp_adapter::call_tool_handler` match arms.
- Wire `handle_get_graph_report` stub to read the real latest report.

### Out Of Scope
- Replacing the central `match tool_name` dispatch with `ToolHandler` trait registry (separate change; documented in CONTEXT.md as the target).
- ADR drafting (design phase).
- Frontend timeline rendering (MCP-only for v1).

## Invariants
- Report insert must never fail silently after fix — `run_report` returns the DB-generated id. (verify: report_stage + integration test)
- Diff/timeline are read-only over `graph_reports`; they never mutate the graph. (verify: no write calls in handlers)
- A workspace with zero reports returns structured empty-state, not an error. (verify: handler empty-path test)

## Domain Language
- Resolved Terms: GraphReport, AnalysisSummary, temporal diffing, ingest Report stage, MCP tool dispatch.
- Unresolved Ambiguities: None.

## Capabilities
### New Capabilities
- `graph_diff`: structural delta between two `GraphReport` snapshots — added/removed god_nodes, new/resolved dead_code, new surprising_connections, symbol/edge count deltas, health_score delta.
- `graph_timeline`: ordered series of `GraphReport` metrics (health_score, symbol_count, edge_count, god_node count) over N days for a workspace.

### Modified Capabilities
- `handle_get_graph_report`: read real latest report instead of returning `None`.

## Approach
1. **Fix first (0.25d):** remove `id` from the INSERT column list in `report_stage.rs` and rely on `DEFAULT gen_random_uuid()`; capture the id via `RETURNING id`; change return to the actual UUID. Keep `UUID` PK (changing to TEXT is inferior — it discards DB identity and index semantics).
2. **Repository reads (0.5d):** add `load_latest_report` (`ORDER BY created_at DESC LIMIT 1`) and `load_report_range` (`created_at >= now() - $days`) returning deserialized `AnalysisSummary` rows.
3. **Handlers (0.75d):** `graph_diff` loads two reports (latest vs N-days-ago, or two explicit ids) and computes set-deltas; `graph_timeline` maps the range into a metric series.
4. **Registration (0.25d):** add match arms; rewire the report stub.

## Affected Areas
| Area | Impact | Description |
|------|--------|-------------|
| m0010 schema / report_stage | bug fix | Stop binding TEXT into UUID column |
| PostgresRepository | additive | 2 new read methods |
| MCP handlers | additive | 2 new tools + 1 stub wired |
| rmcp_adapter | additive | 2 match arms |

## Entropy Budget
| Metric | Estimate | Status |
|--------|----------|--------|
| Existing change entropy | low (one-line insert fix) | OK |
| New connascence | 2 read fns depend on `AnalysisSummary` JSON shape (same as writer) | OK — design must confirm serde compat |

## Risks
| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `AnalysisSummary` JSON drift between writer & reader | med | Deserialize in design test; version field if needed |
| Old TEXT-id rows block `DEFAULT` in fresh deploys | low | Table is forward-compat/empty in prod; migration safe |
| Central match dispatch grows | low | Accepted now; flagged for `ToolHandler` registry change |

## Rollback Plan
Revert the 4 changed files; DDL unchanged so no migration rollback needed.

## Success Criteria
- [ ] `run_report` returns a valid UUID and a row is queryable post-ingest (no DB error).
- [ ] `graph_diff` returns non-empty delta after a second ingest changes the graph.
- [ ] `graph_timeline` returns N points for a workspace with ≥2 reports.
- [ ] `handle_get_graph_report` returns a populated report, not `None`.
