# Proposal: E29.4 — Scale & Operability Proof

## Intent
Ingest and API are not production-proven. `IngestController` holds jobs in an in-memory `HashMap` (controller.rs:42), marks every scan `Completed` even on error (controller.rs:244-246), has no cancellation, no concurrency cap, no durability (controller.rs:9-10 admits status would persist in PG). `perf-budget.toml` covers only micro-ops — no ingest/query/render scale budgets. CI locks out load testing. We need a deterministic scale proof and operability guarantees before granting `production-proven` status.

## Scope

### In Scope
- Deterministic fixtures: 10 MB + 100 MB (CI), 1 GB (optional lane)
- Incremental ingest budgets at 1% / 10% / 50% changed-file ratios
- Query/render budgets for 1k + 5k-node graphs
- Job lifecycle: live progress, error→Failed, cancellation, PG durability
- API limits: body size, timeout, max concurrency, graceful shutdown
- PG pool policy; LISTEN reconnect w/ backoff; metrics + SLOs
- Nightly/load CI lane; production-proven maturity gate

### Out of Scope
- Horizontal clustering; new analytics algorithms (E28.4+); frontend rewrites

## Capabilities

> CONTRACT with sddk-spec. `delivery-readiness-gates` is introduced by E29.0;
> this change extends it and therefore MUST be applied after E29.0.

### New Capabilities
- `ingest-scale-budgets`: seeded fixtures + wall/p99 budgets for full and incremental ingest
- `ingest-job-lifecycle`: durable job state, progress, error→Failed, cancellation, restart recovery
- `api-operability`: body/time/concurrency limits, graceful drain, LISTEN reconnect, metrics/SLOs

### Modified Capabilities
- `ci-postgres-pipeline`: unlock a nightly/load lane (amends the locked Out-of-Scope clause) to run scale fixtures
- `delivery-readiness-gates`: after E29.0 is archived, extend its conjunctive readiness contract with an executable `production-proven` maturity gate; merge eligibility is preserved when implementation-complete checks pass, while promotion is deferred until seven qualifying scheduled runs span at least seven days (1 GB remains optional evidence)

## Approach
Persist `JobStatus` to PG; route `run_scan` errors to `JobState::Failed`; add `CancellationToken` + `Semaphore`. Extend `perf-budget.toml` with scale sections; gate via nightly workflow. Seeded deterministic fixtures only.

## Entropy Budget
- Extract a `JobStore` port (ISP) — controller is CoN name+meaning coupled to its `HashMap`. Target ≤1 new inbound coupling.
- Split `start_scan` (SRP/OCP violation: spawn + state + error swallow). Store interface keeps backends pluggable.
- Design Quality Score target ≥ 0.7; job store ≤3 methods (no Information Bottleneck).

## Affected Areas

| Area | Impact |
|------|--------|
| `crates/cognicode-core/src/application/ingest/controller.rs` | Modified — durability, cancel, failure capture, concurrency |
| `perf-budget.toml`, `scripts/perf-budget-check.sh` | Modified — scale + render budgets |
| `openspec/specs/ci-postgres-pipeline/spec.md` | Modified — nightly/load lane |
| 4 new specs + `.github/workflows/` nightly | New |

## Risks

| Risk | L | Mitigation |
|------|---|------------|
| 1 GB fixtures bloat repo | Med | Generate-on-demand; never commit binaries |
| Durable store write latency | Med | Async status writes; PG LISTEN for clients |
| Nightly flakiness on shared runners | Med | Seeded fixtures; generous budgets |

## Rollback Plan
Additive only. Revert PR → in-memory `HashMap` + `Completed`-always remain the prior default. New `job_status` table is append-only (`DROP TABLE` on full revert). `perf-budget.toml` additions live in new `[ingest.scale]` sections — remove to restore. Nightly workflow is a separate file — delete it.

## Dependencies
- E29.0 (delivery-readiness-gates base capability)
- `e28-2-runtime-closure` — graph runtime path stable; shipped `graph_plan.rs` contract preserved
- E29.3 (complete Explorer interaction path for render and browser budgets)
- PG pool policy is owned by E29.4 (this change) and does NOT depend on E29.1; E29.1 is the temporal/atomic-ingest commit change, not a pool-hardening change.

## Success Criteria

Implementation-complete (required to ship this change):
- [ ] 10/100 MB ingest within budget
- [ ] Incremental 1/10/50% within delta budget
- [ ] Query + render 1k/5k nodes within budget
- [ ] Failed scan reports `failed` not `completed`
- [ ] Cancel stops a job; restart recovers durable state
- [ ] API rejects oversized/over-time requests; shutdown drains in-flight
- [ ] LISTEN reconnects ≤5s after broker drop
- [ ] PG pool policy implemented; maturity-evidence infrastructure in place (retention, run-id capture, SLO reporting)

Maturity-evidence (accumulates after ship, NOT a precondition for merge):
- [ ] Seven consecutive qualifying scheduled runs span at least seven days on `main`
- [ ] 1 GB lane is OPTIONAL extended evidence; absence MUST NOT block `production-proven`
- [ ] The maturity gate permits merge once implementation-complete checks pass; `production-proven` is granted only when the executable evidence rule passes
