# Proposal: E29-0 Trustworthy Delivery Baseline

## Intent

CI and ROADMAP report green on unexecutable routes: the fresh PostgreSQL
migration-order defect was corrected in v0.71.0 but lacks a permanent readiness
gate, ingestion deadlocks after ten results
(P0-2), Explorer fails to compile (P0-4), and PG tests skip silently on migration
errors. This change establishes a trustworthy baseline: the critical path MUST
migrate, ingest, compile, and render before any capacity is marked done.

## Scope

### In Scope
- Regression gate for the shipped m0019→m0018 fresh-DB migration ordering fix
- PgUpsert conflict targets aligned to workspace-scoped identity
- Node/edge notify-trigger separation (edge table lacks `source_path`)
- Ingest >10-result bounded-channel deadlock fix
- Explorer TypeScript compile restoration
- PG tests fail loudly on migration error (not silent skip)
- E2E smoke contract: open workspace → scan → job → stats → landing

### Out of Scope
- Immutable revision persistence (P0-3 — separate change)
- MoldQL production executor wiring (E28.1/E28.2)
- `domain/plan/graph_plan.rs` — E28.2 edge-filter contract, outside this change
- C4 / ViewSpec semantic fidelity; scale / load benchmarks

## Capabilities

> CONTRACT with sddk-spec. Researched `openspec/specs/` (55 existing capabilities).

### New Capabilities
- `delivery-readiness-gates`: Fresh-DB migration invariant, PG-test loud-failure
  contract, and the open→scan→job→stats→landing smoke gate that blocks merge
  until the critical path is verifiably green.
- `ingest-pipeline-integrity`: Workspace-scoped upsert conflict targets, node/edge
  notify-trigger separation, and deadlock-free bounded-channel streaming.

### Modified Capabilities
- `ci-postgres-pipeline`: The "Test gating respects `TEST_DATABASE_URL`"
  requirement changes from "skip silently when env absent" to "fail loudly
  (non-zero exit) on migration error; skip only when `TEST_DATABASE_URL` is
  genuinely absent from the environment."

## Approach

Preserve the v0.71.0 ordering in which m0019's unique index precedes m0018's
foreign keys, and add a fresh-database regression gate so it cannot regress.
Fix `ON CONFLICT` to `(workspace_id, id, kind)` for nodes and
`(workspace_id, source_id, source_kind, target_id, target_kind, kind)` for
edges. Split
`notify_graph_change()` into node-aware and edge-aware functions. Spawn
`pg_upsert_streaming` as a concurrent task **before** filling the channel in
`run_scan`. Hoist the `z` import in `useInvestigations.ts` and fix remaining TS
errors. Add an E2E smoke contract.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| migration runner + fresh-DB test | Verified | Preserve m0019-before-m0018 ordering and fail on regression |
| `.../persistence/m0010_pipeline_schema.sql:219-240` | Modified | Split notify triggers |
| `.../application/ingest/pg_upsert_stage.rs:158,217` | Modified | Workspace-scoped conflicts |
| `.../application/ingest/service.rs:120-129` | Modified | Deadlock fix |
| `.../domain/plan/graph_plan.rs` | **Untouched** | E28.2 contract — no modifications |
| `apps/explorer-ui/src/.../useInvestigations.ts` | Modified | Import hoist + type fixes |
| `apps/explorer-ui/e2e/smoke-*.spec.ts` | New | E2E smoke contract |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Fresh-DB regression gate diverges from populated upgrades | Med | Run both empty and populated migration fixtures without changing the shipped ordering |
| Channel refactor introduces ordering regression | Low | Add >10-result ingest pg_test |
| Additional Explorer TS errors beyond `z` | Med | `tsc --noEmit` as hard gate; fix iteratively |
| Schema↔ingest↔manifest connascence (4.0 bits) | High | Tight Phase-0 scope; no surface expansion |

**Entropy budget**: DQS 0.38→0.45 target. Reduces schema↔ingest connascence
(4.0 bits per assessment) by aligning conflict targets and trigger contracts. No
new coupling introduced.

## Rollback Plan

1. Revert only the fresh-DB regression gate if it is faulty; do not undo the
   shipped v0.71.0 migration ordering correction.
2. Revert PgUpsert conflict targets, notify-trigger split, and channel refactor.
3. Revert Explorer TS fixes, smoke test, and loud-failure gate.
4. No data loss — all migrations use idempotent `IF NOT EXISTS` guards.

## Dependencies

- Docker Compose PG 16 stack (operational per `ci-postgres-pipeline` capability)
- `TEST_DATABASE_URL` environment variable for PG-dependent tests

## Success Criteria

- [ ] Fresh empty DB: `run_migrations()` succeeds without manual intervention
- [ ] Ingest fixture with >10 source files completes without deadlock
- [ ] `npm run build` in `apps/explorer-ui` exits 0
- [ ] PG test suite aborts loudly (non-zero exit) on migration error — no silent skip
- [ ] E2E smoke: open workspace → scan → job polling → graph stats >0 → landing renders — all pass
- [ ] `domain/plan/graph_plan.rs` is byte-identical to the E28.2 baseline
