# Proposal: CP1.0 — Control Plane First Vertical (architecture read endpoint)

> Change: `cp1-control-plane-first-cycle`
> Outcome: **CLOSED** — first CP1 vertical delivered; CP1.0 closed; e78 gate re-applied.
> Baseline: HEAD `949cae62a7a35171863df0b471f8932f7f436a98` (= origin/main).

## Intent

Deliver the **first Control Plane product question** as a bounded vertical:
"What is the architecture state of workspace W at snapshot S?" — a read-only,
authority-free projection of the e77 executable architecture, exposed over HTTP
under the Control Plane namespace.

This is the **first real product consumer** of the e77 executable architecture
module (`application::architecture::*`), not an internal/test surface.

## Non-goals (carried from CP0 mandate)

- No `AttentionItem`, `CaseView`, Investigation or approval UI.
- No authn/authz beyond the existing probe-based gating.
- No domain logic in TypeScript; no Rust-in-Node.
- No canonical CogniCode state in any optional shell backend.
- No e78 (packs) work. e78 is gated by ≥2 real consumers; this change delivers
  consumer #1.

## Constraints carried from LSI mandate

- CogniCode Rust = authoritative; Control Plane = projection/orchestration.
- Frontend identity != authority. The control-plane endpoint returns **DTOs with
  references only** (ids, coordinates, grounding fact ids). It never carries
  Evaluation verdict or Constraint truth.
- Authority invariants hold unchanged. Trial/PolicyGate/Evaluation remain
  technical; external human approval is authority; `PromotionPermit` is
  capability.
- Backstage is **still** an optional shell. CP1.0 endpoint can be exposed there
  later without changing this contract.

## WU sequence (consolidated under receipt `closeout-receipt.md`)

| WU | commit | role |
|---|---|---|
| WU2/WU3 | `6169d454` feat(core): ControlQueryService — architecture read model | Introduce `application::architecture::ControlQueryService` + read-model DTOs + helpers `source_from_files`, `source_from_source_root`. **No separate WU1 commit** — preparatory work for read-model contract was consolidated into this commit. |
| WU4 | `fed57b95` feat(explorer): control-plane architecture read endpoint | Wire `ApiState.control_query` + endpoint `GET /control-plane/workspaces/:workspace_id/architecture` + C1–C5 tests. |

## Capabilities delivered

### `control-plane-architecture-read`

- **HTTP**: `GET /control-plane/workspaces/:workspace_id/architecture?source={from-files|from-source-root}&at={snapshot}`
- **Service**: `application::architecture::ControlQueryService::query_architecture`
- **DTO**: `ArchitectureReadModel { evaluation_status, constraints, violations, source }`
  where `constraints: Vec<ConstraintRef>` and `violations: Vec<ViolationRef>` carry
  `ConstraintId` / `ViolationId` only — no truth payload.
- **Fail-closed contract** (mirrors ControlQueryService):
  - No wired `ControlQueryService` → `status: "incomplete"` (never `clean`).
  - Empty admission set → `status: "incomplete"`.
  - Evaluation errors → `status: "incomplete"` with the failure class attached.
  - Valid evaluation → `status: "evaluated"` with reference projections.

### C1–C5 evidence (cp1_control_plane_endpoint.rs)

| test | invariant |
|---|---|
| `c1_not_wired_reads_incomplete_never_clean` | without a wired service, never returns a clean verdict |
| `c2_empty_admission_reads_incomplete` | empty admission yields incomplete, never silent pass |
| `c3_real_evaluation_is_evaluated` | real evaluation yields evaluated + projection |
| `c4_violation_projected_with_reference_not_truth` | violations carry ids, not payload |
| `c5_endpoint_path_is_read_only` | gets no state mutations, fails closed |

## Affected areas

| Area | Impact | Description |
|---|---|---|
| `crates/cognicode-core/src/application/architecture/` | +336 LOC | New service + read-model DTOs |
| `crates/cognicode-explorer/src/api.rs` | modified | `ApiState.control_query` + `with_control_query` builder + endpoint route at line 635 |
| `crates/cognicode-explorer/tests/cp1_control_plane_endpoint.rs` | +526 LOC | C1–C5 e2e tests |
| `crates/cognicode-core/src/application/architecture/mod.rs` | +4 LOC | Re-export `ControlQueryService` |

## Risks and mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| Read model bleeds authority/verdict into DTOs | High | C4 test enforces reference-only; review checklist |
| Silent degradation when service not wired | Med | C1 enforces incomplete (never clean) |
| Backstage or other shell tries to act on these refs | Med | DTO docs + CP1.0 does NOT publish consumer guidance yet |

## Rollback

Disable `with_control_query` injection (drops the route mount → 404). No data
migration; the service is stateless.

## Closing outcome

This change closes **CP1.0 only**. It does NOT open CP1.1.

The next CP1 problem must be identified from evidence produced by this vertical,
not from the CP0 backlog. See `closeout-receipt.md §6` for the next-problem
checklist.
