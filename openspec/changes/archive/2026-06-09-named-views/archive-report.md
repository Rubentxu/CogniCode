# sdd/named-views/archive-report

## Change Summary

**Change**: `named-views`
**Project**: cognicode
**Archived**: 2026-06-09
**Location**: `openspec/changes/archive/2026-06-09-named-views/`
**Status**: COMPLETE — all phases passed, all 27 TDD tests green

---

## What Changed

| Dimension | Before | After | Delta |
|-----------|--------|-------|-------|
| MCP tools | 24 | 28 | +4 (`view_save`, `view_load`, `view_list`, `view_delete`) |
| PostgreSQL tables | 0 named_views table | 1 (`named_views`) | +1 additive DDL |
| DTOs | — | `NamedView`, `NamedViewDescriptor` | new |
| Service methods | — | `save_view`, `load_view`, `list_views`, `delete_view` | +4 |
| Error variants | — | `Conflict`, `NotFound`, `FeatureDisabled`, `InvalidInput` | +4 |
| TDD tests | 0 | 27 | +27 RED→GREEN |
| Integration test file | — | `named_views_integration.rs` | new (~440 lines) |

**Total lines added**: ~1400 across 11 files

### Files Changed (Implementation)

| File | Lines Added | Role |
|------|-------------|------|
| `schema_postgres.sql` | +22 | DDL + unique index |
| `postgres_repository.rs` | +300 | 5 CRUD methods + `NamedViewRow` struct + 5 pg_test! entries |
| `dto.rs` | +140 | `NamedView`, `NamedViewDescriptor`, `truncate_description` + 4 unit tests |
| `error.rs` | +13 | 4 new variants |
| `service.rs` | +400 | 4 service methods × 2 cfgs + 5 unit tests |
| `mcp.rs` | +400 | 4 constants, 4 args, 4 dispatch arms, 4 schemas, TOOL_NAMES 24→28 + 10 tests |
| `postgres_bridge.rs` | +25 | `open_graph_with_repo` helper |
| `api.rs` | +9 | Error → status code mapping |
| `named_views_integration.rs` | +440 | 14 PG-gated integration tests |
| `glossary.md` | +6 | Named view entry |

---

## What Didn't Change

- **24 pre-existing MCP tools** — all preserved, no rename, no removal
- **Existing projection/build logic** — `view_load` re-dispatches through `contextual_view()`, no duplication
- **SQLite / in-memory path** — unchanged; `postgres` feature is purely additive
- **Trait signatures** — no existing trait modified
- **Other crates** — `cognicode-core` (aside from repo), `cognicode-axiom`, LSP proxy, etc.

---

## TDD Summary

**27 tests written FIRST (RED), then GREEN:**

| Phase | Tests | Count |
|-------|-------|-------|
| No-PG unit | `named_view_serde_roundtrip`, `truncate_description_*`, `tool_schemas_*`, `dispatch_*_feature_gate_off`, `explorer_service_pg_disabled_*` | 22 |
| PG integration | `named_views_migration_is_idempotent`, `named_views_unique_index_rejects_duplicate_name`, `named_views_load_round_trip`, `named_views_list_scope_and_order`, `named_views_delete_scope_guarded` | 5 |

**All 27 tests pass** on both default build and `--features postgres` build.

---

## Entropy Analysis

**Method**: Heuristic connascence assessment

| Component A | Component B | Connascence Type | Severity |
|-------------|-------------|------------------|----------|
| `dto.rs` | `service.rs` | Name | ✅ OK |
| `service.rs` | `mcp.rs` | Name | ✅ OK |
| `postgres_repository.rs` | `schema_postgres.sql` | Meaning | ⚠️ Low |
| `mcp.rs` | `dto.rs` | Type | ⚠️ Low |

**OCP**: H(Δ_existing) ≈ 0 — pure extension, no existing surface modified.
**Critical pairs**: None — purely additive, no existing surface modified.

---

## Artifact Inventory

| Artifact | Engram ID | OpenSpec Path | Status |
|----------|-----------|---------------|--------|
| `sdd/named-views/explore` | #1445 | `archive/exploration.md` | ✅ |
| `sdd/named-views/proposal` | #1449 | `archive/proposal.md` | ✅ |
| `sdd/named-views/spec` | #1450 | `specs/named-view-persistence/spec.md` (promoted) | ✅ |
| `sdd/named-views/design` | #1454 | `archive/design.md` | ✅ |
| `sdd/named-views/tasks` | #1459 | `archive/tasks.md` | ✅ |
| `sdd/named-views/apply-progress` | #1461 | (Engram only) | ✅ |
| `sdd/named-views/verify` | #1479 | `archive/verify-report.md` | ✅ |
| `sdd/named-views/archive-report` | #1481 | `archive/archive-report.md` | ✅ |

**Verify report CRITICAL check**: ✅ PASS — no CRITICAL issues in verification report. Safe to archive.

---

## Follow-On Unblocked

The `named-views` change is **complete and archived**. No blocked follow-on work identified.

### What was NOT done (intentionally out of scope for v1)
- Share-by-link
- Version history / editing / rename
- ACLs beyond per-call scope check
- UI surface (MCP-only in v1)
- In-memory/SQLite fallback

---

## SDD Cycle Complete

The `named-views` change has been fully planned, implemented, verified, and archived.
- ✅ Proposal: intent, scope, approach, rollback, risks
- ✅ Spec: 8 requirements, 36 scenarios, TDD RED gate with 20 tests
- ✅ Design: data model, DDL, DTOs, service delegation, MCP surface, testing strategy
- ✅ Tasks: 3-phase plan, 19 tasks, strict dependency chain
- ✅ Apply: 22+5=27 tests green, ~1400 lines across 11 files
- ✅ Verify: 8/8 requirements met, 36/36 scenarios covered, build clean
- ✅ Archive: spec promoted to `openspec/specs/named-view-persistence/`, change folder archived

**Ready for the next change.**
