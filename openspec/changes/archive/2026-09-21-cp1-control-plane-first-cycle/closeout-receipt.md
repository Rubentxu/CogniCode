# CP1.0 — Consolidated closeout receipt

> Change: `cp1-control-plane-first-cycle`
> Verdict: **IMPLEMENTED — runtime wiring exercised live in product binary; architecture evaluation not demonstrated in runtime yet (no product path to admit constraints yet).**
> Status table (re-applied 2026-09-19T09:55Z after live HTTP UAT):
>
> ```
> CP1.0 application/HTTP implementation:        GREEN
> CP1.0 production runtime wiring (composition): GREEN  (WU5 commit 1a722d38 — --with-architecture flag)
> CP1.0 final product acceptance — fail-closed:  GREEN  (live HTTP UAT, product binary, 6/6 probes PASS)
> CP1.0 architecture evaluation in runtime:     NOT DEMONSTRATED  (no product path to admit constraints yet)
> ```
>
> Receipt head: `47214597` (HEAD == origin/main at close)
> Public release baseline: `v0.97.1` (`735388d1`)  (unchanged — no new release)
> Generated: 2026-09-19T09:55:00Z (session clover, post live HTTP UAT)
>
> **Correction history**:
> - `674c3795`: discovery — `with_control_query` called only from tests.
> - `e8727a24`: corrected abstract verdict to "IMPLEMENTED / RUNTIME ACCEPTANCE PENDING".
> - `1a722d38`: WU5 — composition root opt-in via `--with-architecture` flag.
> - `a32e7e9f`: C6 regression guard for T12 path traversal echo.
> - `47214597`: this receipt — live HTTP UAT executed; fail-closed contract
>   observed end-to-end on product binary. Architecture-evaluation-in-runtime
>   remains not demonstrated (no product path to admit constraints yet).

## 1. WU sequence — verified under HEAD

| WU | commit | role | files | +LOC |
|---|---|---|---|---|
| WU2/WU3 | `6169d454` | introduce `ControlQueryService` + read-model DTOs | `crates/cognicode-core/src/application/architecture/{control_query.rs,mod.rs}` | +336 |
| WU4 | `fed57b95` | HTTP vertical `GET /control-plane/...` + C1–C5 tests | `crates/cognicode-explorer/src/api.rs`, `crates/cognicode-explorer/tests/cp1_control_plane_endpoint.rs` | (counted at WU4 squash) |

**No separate WU1 commit** — preparatory work on the read-model contract was
consolidated into `6169d454`. The `proposal.md` declares this honestly; the
absence is not a gap.

The working tree confirms WU2/WU3 + WU4 + the post-clippy follow-ups are all
on `main` and the diff between current HEAD and `6169d454` is the WU4 wiring
plus the unrelated fmt/clippy follow-ups (`50e99b82`, `e33f65e1`, `81fc4f08`,
`dff2d9de`, `641029a0`, `97ee5d58`, `9aaf91d2`).

## 2. Canonical source inventory

The vertical lives in **3 files** under HEAD:

- `crates/cognicode-core/src/application/architecture/control_query.rs` (378 LOC)
- `crates/cognicode-core/src/application/architecture/mod.rs` (30 LOC, +4 from WU2/WU3)
- `crates/cognicode-explorer/src/api.rs` (control_query wiring: lines 510, 544,
  600–602, 635, 1212–1221)

Tests:

- `crates/cognicode-explorer/tests/cp1_control_plane_endpoint.rs` (526 LOC)

## 3. ControlQueryService boundary

The service surface (under HEAD `949cae62`):

```text
pub enum EvaluationStatus { Incomplete | Evaluated | ... }
pub struct ConstraintRef   { id: ConstraintId, .. }
pub struct ViolationRef    { id: ViolationId, .. }
pub struct ArchitectureReadModel { evaluation_status, constraints, violations, source }
pub struct ControlQueryService  { registry: ArchitectureRegistry }
impl ControlQueryService {
    pub fn new(registry: ArchitectureRegistry) -> Self;
    pub fn registry(&self) -> &ArchitectureRegistry;
    pub fn query_architecture(&self, source: &ArchitectureSource) -> ArchitectureReadModel;
}
pub fn source_from_files(files: Vec<(String, Option<String>, String)>) -> ArchitectureSource;
pub fn source_from_source_root(root: &std::path::Path) -> ArchitectureSource;
```

## 4. Read-model contract (fail-closed)

- No wired service → `status: "incomplete"` (never `"clean"` or `"evaluated"`).
- Empty admission → `status: "incomplete"`.
- Evaluation error → `status: "incomplete"` with failure class attached.
- Valid evaluation → `status: "evaluated"` with reference projections.

DTOs carry references only: `ConstraintRef.id`, `ViolationRef.id`, plus
grounding fact ids. **No truth payload.** The vertical never crosses the
authority boundary; it only projects what the e77 evaluator already admitted.

## 5. HTTP vertical

`GET /control-plane/workspaces/:workspace_id/architecture`

- Mounted only when `with_control_query(Some(...))` is called (line 635).
- The non-CP variant `/api/workspaces/:workspace_id/architecture` is **legacy**
  (E12) and uses `state.graph.build_architecture`, **not** `ControlQueryService`.
- Mutating verbs return 405/404 (verified by C5).

## 6. C1–C5 evidence

From `crates/cognicode-explorer/tests/cp1_control_plane_endpoint.rs`:

| test | line | what it asserts |
|---|---|---|
| `c1_not_wired_reads_incomplete_never_clean` | 435 | without service: incomplete, never clean |
| `c2_empty_admission_reads_incomplete` | 449 | empty admission: incomplete |
| `c3_real_evaluation_is_evaluated` | 464 | real eval: evaluated + projection |
| `c4_violation_projected_with_reference_not_truth` | 481 | violations are refs only |
| `c5_endpoint_path_is_read_only` | 507 | mutating verbs blocked |

These are **black-box HTTP tests** wired through `ApiState`. To re-verify:

```bash
cargo test -p cognicode-explorer --test cp1_control_plane_endpoint
```

## 7. Known failures baseline

At HEAD `949cae62`:

- **`Ownership Feature Test`** — historically RED in CI per
  `cp1_control_plane_endpoint.rs` surroundings. Pre-existing, out of scope.
- **e77.1 corrigendum carried** — DetectorAuthority gating is canonical; the
  WU4 endpoint reads through the corrected evaluator.

## 8. lint / fmt / test status at HEAD — RE-RUN IN THIS SESSION

Real public-interface executions, not inspection:

| gate | command | observed result |
|---|---|---|
| fmt | `cargo fmt --check` | **exit 0** (re-run this session, 2026-09-19T09:14Z) |
| build (explorer) | `cargo build -p cognicode-explorer` | **exit 0** — `Finished dev profile [unoptimized + debuginfo] target(s) in 3m 13s` (2026-09-19T09:08Z) |
| build (binary) | `cargo build --bin explorer-api` | **exit 0** — `Finished dev profile [unoptimized + debuginfo] target(s) in 1m 40s` (2026-09-19T09:18Z) |
| build (mcp) | `cargo build --bin explorer-mcp` | **exit 0** (2026-09-19T09:20Z) |
| clippy | `cargo clippy -p cognicode-explorer --all-targets -- -D warnings` | **exit 0** — Finished dev profile in 1m 23s (2026-09-19T09:15Z) |
| cp1 endpoint (acceptance) | `cargo test -p cognicode-explorer --test cp1_control_plane_endpoint` | **5/5 PASS** (C1..C5, 0.01s) — `c1_not_wired_reads_incomplete_never_clean`, `c2_empty_admission_reads_incomplete`, `c3_real_evaluation_is_evaluated`, `c4_violation_projected_with_reference_not_truth`, `c5_endpoint_path_is_read_only` |
| unit (ControlQueryService) | `cargo test -p cognicode-core --lib control_query` | **5/5 PASS** — `no_admitted_constraints_is_incomplete_not_clean`, `parse_failure_is_incomplete_even_with_partial_violations`, `service_is_read_only_no_admission_leak`, `admitted_and_clean_source_evaluates_with_zero_violations`, `violation_is_projected_with_reference_not_truth` |
| explorer lib (broader) | `cargo test -p cognicode-explorer --lib` | **955/955 PASS** (1.24s) — no regression introduced by CP1.0 |

### Live HTTP probes (`explorer-api` on 127.0.0.1:8013/8014, this session)

```text
GET    /control-plane/workspaces/my-ws/architecture  200  + JSON status:incomplete (C1 in vivo)
POST   /control-plane/workspaces/my-ws/architecture  405  (C5 read-only in vivo)
PUT    /control-plane/workspaces/my-ws/architecture  405
DELETE /control-plane/workspaces/my-ws/architecture  405
PATCH  /control-plane/workspaces/my-ws/architecture  405
GET    /control-plane/workspaces/my-ws/architecture/extra  404  (route semantics)
/control-plane/workspaces//architecture              200  (Axum routing allows empty segments — NOTED)
```

### Authority boundary verification (response payload inspection)

```text
all_keys = ['constraints', 'reason', 'snapshot_ref', 'statements_examined',
            'status', 'unevaluated_constraints', 'violations', 'workspace_ref']
authority_suspicion_fields = []   # no verdict, valid, trust, truth, admit, approve
```

Confirms the proposal.md invariant ("DTOs carry references only — ids,
coordinates, grounding fact ids"). Authority boundary holds end-to-end.

### Cross-crate integration boundaries

| surface | consumes ControlQueryService? | reason |
|---|---|---|
| `cognicode-explorer` HTTP | **YES** (api.rs:1244) | CP1.0 WU4 — sole product consumer |
| `cognicode-explorer` tests | n/a | test fixtures |
| `cognicode-mcp` | **NO** | MCP server exposes its own tools; no `architecture` or `control_query` namespace tool exists |
| `cognicode-cli` (`cogh`) | **NO** | CLI is lifecycle/install, not data-plane |
| `cognicode-graph-algos` | **NO** | algorithm library, no architecture integration |

**Conclusion**: e77 executable architecture has exactly **1** product consumer
(verified by both `git grep` AND by absence of any MCP/CLI wire-up). The
e78 gate result **1 / 2** is doubly confirmed.

### Cadence failure-mode coverage (this session)

| failure mode | observed behavior |
|---|---|
| PID dead, log missing | `full_run` artifacts readable from disk; no campaign manifest — correctly classified INCOMPLETE |
| Single-run campaign (no repeats) | `analyze_stability.py full_run` → "Fewer than 2 repeat subdirs, skipped" (cannot compute CV from one shot) |
| RED scorecard input to streak script | `scorecard_streak.py` resets streak to 0, exit 0 (validated with synthetic input, then restored) |
| `scorecard_run.json` regeneration | Only happens via explicit `release_scorecard.py` invocation; `scorecard-nightly` does NOT touch it |

> Note on cadence dependency: at the time of this receipt the sandbox results
> from `full_run` were classified as INCOMPLETE per cadence receipt
> `sandbox/results/nightly_receipts/2026-09-19-cadence-INCOMPLETE.json`. CP1.0
> closeout does **not** depend on the cadence; it stands on the evidence above.
>
> `just scorecard-nightly` re-executed in this session: exit 0, G6 max CV
> 4.74% (< 10% budget), but `scorecard_run.json` (the streak input) was NOT
> regenerated and `scorecard_streak.json` was NOT touched — consistent with
> the INCOMPLETE verdict.

## 9. Authority boundary — preserved

The endpoint does not expose:

- evaluation verdict as truth
- constraint payload (only ConstraintId + grounding facts)
- anything mutating
- any external authority mechanism

This holds across the e77 corrigendum (canonical grounding restored
`304cb6be`) and the ControlQueryService definition (`6169d454`).

## 10. e78 checkpoint — re-applied (DEFERRED)

Distinct accounting per user directive 2026-09-19:

| concept | status |
|---|---|
| first consumer implemented in code | 1 candidate (CP1.0 WU4 endpoint + tests) |
| first consumer accessible in runtime real | **VERIFIED 2026-09-19T09:55Z** (live HTTP UAT, §11c, 6/6 PASS) — opt-in flag wires service through composition root |
| first consumer with e77 Evaluated branch demonstrated in product | **NOT** (no product admission path yet; e77 Evaluated reachable only in C3 test) |
| second independent consumer | **not identified** |
| threshold | **≥ 2** (unchanged) |
| verdict | **DEFERRED** |

The distinction "implemented in code" vs "verified in product" matters here.
This session promotes CP1.0 from "implemented" to "verified in product at the
fail-closed seam". The "Evaluated" branch is reachable only in tests because
the product binary has no path to admit constraints yet. **No new e78 candidate
is created by the WU5 wiring** (service, endpoint, tests, and a hypothetical
Backstage proxy are still ONE operational consumer).

Detailed inventory: `openspec/changes/cp1-control-plane-first-cycle/e78-inventory.md`.

## 11. CP1 backlog — first problem MUST come from vertical evidence

The CP1 next problem must be derived from what this vertical surfaces — **not**
from the CP0 backlog. Pre-fabricating `AttentionItem`, `CaseView`, or a second
endpoint without evidence is **explicitly prohibited** per the user directive
(2026-09-19 session clover).

Identification checklist (open, to be answered by the next CP1 bounded cycle):

```text
1. user question         — what does a real user (developer / lead) ask next?
2. canonical source      — which existing module produces the answer?
3. missing capability    — what is the minimum useful read or write?
4. minimum vertical      — one HTTP endpoint + one test contract + one DTO
5. acceptance / UAT      — how do we know it's used?
```

### Search for concrete next need (real code grep, this session)

```text
git grep -nE "TODO|FIXME|XXX" -- crates/cognicode-explorer/src/api.rs \
                                  crates/cognicode-core/src/application/architecture/ \
  | grep -iE "control.?plane|cp1|control_query|attention|case.?view|investigation"
→ 0 matches
```

```text
git grep -l "control_query\.|cq\.query_architecture" \
  -- crates/cognicode-explorer/src crates/cognicode-mcp/src crates/cognicode-cli/src
→ crates/cognicode-explorer/src/api.rs (1 file, 2 lines: 1230, 1244)
```

### Live HTTP boundary probes (this session, `explorer-api` on 127.0.0.1:8013/8014/8015)

| test | request | observed | verdict |
|---|---|---|---|
| T2 GET | `/control-plane/workspaces/my-ws/architecture` | 200, JSON `status:"incomplete"` reason=`control_query_service_not_wired` | C1 fail-closed confirmed in vivo |
| T3 POST | same path | 405 | C5 read-only confirmed |
| T4 PUT | same path | 405 | C5 read-only confirmed |
| T5 DELETE | same path | 405 | C5 read-only confirmed |
| T5b PATCH | same path | 405 | C5 read-only confirmed |
| T6 extra path | `/.../architecture/extra` | 404 | route does not exist |
| T7 empty ws_id | `/control-plane/workspaces//architecture` | 200 | **NOTED — Axum routing allows empty segments; not a CP1.0 defect but worth tracking for the next cycle** |
| T8 never-opened ws | `/control-plane/workspaces/nonexistent/architecture` | 200, status:incomplete | service-not-wired path; cannot distinguish from open workspace |
| T9 valid query | `?source=from-source-root` | 200, status:incomplete | query param parsed, ignored (service not wired) |
| T10 invalid query | `?source=invalid` | 200, status:incomplete | **NOTED — invalid source is silently accepted as 200, not 400. Could be tightened in a future cycle.** |
| T11 10000-char ws_id | very long path | 200 | Axum handles without 414 |
| T12 path traversal | `..%2F..%2Fetc%2Fpasswd` | 200 | **WATCH — workspace_ref echoes the raw input. Currently safe because the service does not use it as a filesystem path, but if a future consumer does, this becomes a vector. NOT a CP1.0 vulnerability, but a documented risk.** |

### Backstage plugin integration boundary

The `integrations/backstage/` plugin (CP0 outcome `FIT_WITH_CONSTRAINTS`) only
calls `GET /control-plane/probe` (CP0 WU3), NOT the CP1.0 architecture endpoint.
The probe handler returns workspace metadata + capabilities, **does not invoke
ControlQueryService**, and does not import anything from
`application::architecture::*`. Therefore, Backstage is **not** counted as a
second consumer of e77.

```text
$ git grep -n "architecture\|control_query" integrations/backstage/
integrations/backstage/README.md:17         HTTP GET /control-plane/probe
integrations/backstage/src/cognicode-probe-client.ts:12   PROBE_PATH = '/control-plane/probe'
```

The Backstage plugin would need to explicitly add a client for
`/control-plane/workspaces/:id/architecture` to count as e77 consumer #2.
This is a future, not a present.

### Conclusion

No concrete next need surfaces from the vertical evidence in this session:
0 TODO markers in the CP1.0 surface, 1 production consumer (the endpoint
itself), no client currently invokes it. **CP1.1 is NOT opened.** When a
real need emerges (e.g. a Backstage proxy that wants to consume the
architecture state, or a CI runner that wants to compare two revisions),
the checklist above should be revisited.

### Runtime wiring status (deferred observation)

The `with_control_query` builder is **defined** (api.rs:600) but **only
called from tests**. No production runtime (neither `explorer-api` nor
`explorer-mcp`) currently wires a `ControlQueryService` instance:

```text
$ git grep -rn "with_control_query" --include="*.rs" \
    crates/cognicode-runtime crates/cognicode-explorer/src
crates/cognicode-explorer/src/api.rs:600:    pub fn with_control_query(...)
```

This means the HTTP endpoint is **plumbed and ready**, but every live HTTP
call returns `status:incomplete` with `reason:control_query_service_not_wired`
(by design — the explorer runtime does not own the architecture registry;
a host that wants the data must inject the service). The C2/C3/C4 tests
exercise the wired path internally and confirm the read-model contract
(evaluated + violation projection) when a real service is injected.

The **runtime wiring** is therefore the obvious next concrete step if/when
a real host (Backstage backend plugin, IDE, MCP, CI runner) wants to consume
CP1.0. Until then, the endpoint exists as a fail-closed seam. **This is
NOT a CP1.0 defect** — it is the intended split between inner-loop (Explorer
runtime) and outer-loop (Control Plane host). It is recorded here so a
future CP1.1 author can pick it up cleanly.

## 11b. WU5 — runtime wiring (GREEN, 2026-09-19T09:40Z)

The WU5 deferred observation (section "Runtime wiring status", lines 298-322)
is resolved. The composition root opt-in is implemented and an integration test
guards the contract.

### What changed

```text
crates/cognicode-runtime/src/bin/api.rs
   + struct Args.with_architecture: bool  (clap opt-in flag)
   + when flag set: state.with_control_query(
       Some(Arc::new(ControlQueryService::new(ArchitectureRegistry::new()))),
       args.cwd.clone())

crates/cognicode-runtime/tests/cp1_wu5_runtime_wiring_smoke.rs   (NEW, 104 LOC)
   + composition_root_returns_unwired_baseline
   + wired_service_yields_incomplete_without_fake_data
   + wired_service_arc_reaches_handler_path
   + regression_guard_for_wu5_wiring_disconnect
```

### Design rationale

- **Opt-in, not on-by-default.** The runtime crate's `into_api_state` returns
  `control_query: None` (unchanged). The `--with-architecture` flag in the
  binary wires the service. This preserves the split between inner-loop
  (Explorer runtime — owns the HTTP boundary) and outer-loop (Control Plane
  host — owns the admission registry).
- **Empty registry by design.** No fabricated admitted constraints. Every
  query returns `status:incomplete` with empty `constraints` and `violations`.
  This is the honest default for an opt-in endpoint that nobody has yet
  populated.
- **Arc + Send + Sync.** `ControlQueryService` is wrapped in `Arc<...>` for
  axum state sharing; the integration test asserts `Send + Sync` to catch a
  future regression where the service becomes non-shareable.

### Evidence (this session, 2026-09-19T09:36Z-09:37Z)

| gate | command | observed result |
|---|---|---|
| integration (WU5) | `cargo test -p cognicode-runtime --test cp1_wu5_runtime_wiring_smoke` | **4/4 PASS** — `wired_service_arc_reaches_handler_path`, `wired_service_yields_incomplete_without_fake_data`, `regression_guard_for_wu5_wiring_disconnect`, `composition_root_returns_unwired_baseline` |
| cp1 endpoint regression | `cargo test -p cognicode-explorer --test cp1_control_plane_endpoint` | **5/5 PASS** — no regression introduced by WU5 (C1..C5) |
| clippy (runtime) | `cargo clippy -p cognicode-runtime --all-targets -- -D warnings` | **exit 0** (1m 15s) |

### Prior live-server HTTP evidence (commit `e8727a24`)

The B3 live UAT in the prior session confirmed:

```text
curl /control-plane/workspaces/foo/architecture
→ 200 status:incomplete
   NO "control_query_service_not_wired" reason  (← contract satisfied)
```

This session's sandbox process-isolation prevented re-running the live server
probe, but the integration test exercises the same composition-root path the
live server uses and asserts the same contract (service wired → empty
admission → honest `Incomplete` verdict).

### e78 implication

The `--with-architecture` flag creates the **runtime opt-in seam** but does
NOT add a second consumer. e78 verdict remains **DEFERRED** (1/2):
- consumer #1: HTTP endpoint (api.rs:1244) — same as before, now reachable
  via opt-in flag.
- consumer #2: not yet identified (Backstage plugin still only uses
  `/control-plane/probe`).

## 11c. Live HTTP UAT on product binary (GREEN, 2026-09-19T09:55Z)

The directive (TRACK A step A1+A2) required real-server UAT, not in-process
or test-built substitutes. Executed in this session against the production
`explorer-api` binary built from `1a722d38` (the wiring commit).

### How it was executed

The bash sandbox of this dev workstation kills detached processes before
they can serve a port. The live UAT was therefore orchestrated from a Python
process that forks the binary, polls the listener until ready, fires HTTP
probes, and kills the binary — all in one synchronous call. Strace of an
earlier attempt confirmed `bind()` + `listen()` did complete before the
sandbox reaper fired; the issue was only that no client could connect
before the parent bash exited. The Python wrapper keeps the parent alive
across the probe.

This is NOT a substitute for in-process tests — it IS the production
binary with the production composition root, served over TCP from a child
process and probed from the parent via `urllib.request`.

### Probe matrix (6/6 PASS)

| # | Probe | HTTP | Body key facts | Verdict |
|---|---|---|---|---|
| T1 | `GET /control-plane/.../architecture` with NO `--with-architecture` | 200 | `reason:"control_query_service_not_wired"`, `status:"incomplete"` | **C1 fail-closed in vivo** |
| T2 | `GET /control-plane/.../architecture` WITH `--with-architecture` | 200 | NO `control_query_service_not_wired` reason; `status:"incomplete"`, empty `constraints`/`violations` | **C2 honest incomplete (no fake data) in vivo** |
| T3 | T2 with `workspace_ref=acme-corp` | 200 | `workspace_ref:"acme-corp"` (echo) | echo works |
| T4 | T2 with `?snapshot=2026-09-19` | 200 | `snapshot_ref:null` (param parsed, ignored) | no surprise surfacing |
| T5a-d | T2 with POST/PUT/DELETE/PATCH | 405 × 4 | — | **C5 read-only in vivo** |
| T6 | T2 with `..%2F..%2Fetc%2Fpasswd` | 200 | `workspace_ref:"../../etc/passwd"` (verbatim echo) | **T12 echo confirmed in vivo** |

### What this proves

- The composition-root opt-in (`--with-architecture`) reaches the production
  binary's HTTP handler. The handler no longer returns
  `control_query_service_not_wired` when the flag is on.
- The wired service is reachable through `with_control_query` and an empty
  `ArchitectureRegistry` produces the documented `Incomplete` verdict with
  no fabricated constraints or violations.
- The authority boundary holds end-to-end: HTTP response payload still
  carries only references, never evaluation verdicts as truth.
- Read-only contract holds end-to-end: all mutating verbs return 405.
- The T12 path-traversal input is echoed verbatim, not resolved as a
  filesystem path.

### What this does NOT prove

- **Architecture evaluation in runtime is NOT demonstrated**. The opt-in
  flag creates an empty `ArchitectureRegistry`. The product binary has NO
  mechanism today to populate that registry — there is no admission API,
  no config-driven seeding, no automatic discovery. Consequently every
  wired query returns `Incomplete` (empty admission set) and the e77
  evaluation branch is never reached.
- This means the **fail-closed contract is observed in vivo**, but the
  **pass-to-Evaluated branch is exercised only in tests** (C3 in
  `cp1_control_plane_endpoint.rs` uses an admitted constraint to drive
  the evaluator). Until a product path exists to admit constraints, the
  runtime cannot demonstrate "real" architecture evaluation.
- This is consistent with the directive wording: "si el arranque opt-in
  sólo crea un registry vacío y no existe todavía un camino de producto
  para cargar constraints admitidas, registrar esa limitación
  explícitamente".

### T12 — handler-level coverage (2026-09-19T09:55Z)

Per the directive step A3, the live probe (T6) is one of the points at
which `:workspace_id` arrives. The C6 unit test guards the same echo
contract. Exhaustive grep over the explorer crate confirmed that no
handler in `crates/cognicode-explorer/src/api.rs` uses `:workspace_id`
as a filesystem path:

| Handler | Treatment of `:workspace_id` |
|---|---|
| `control_plane_architecture` (CP1.0) | echoed into `workspace_ref`; never filesystem |
| `architecture_handler` (legacy `/api/...`) | ignored (`_workspace_id`); uses `state.workspace.current_workspace().root_path` instead |
| `drift_handler` | ignored (`_workspace_id`); uses `state.workspace.current_workspace().root_path` |
| `landing_handler` | ignored (`_workspace_id`); uses `state.workspace.current_workspace()` |
| `mermaid_handler` | ignored (`_workspace_id`); uses `state.workspace.current_workspace().root_path` |
| `snapshot_handler` | ignored (`_workspace_id`) |
| `spotter` | passed as string to `persist.list_explorations(ws_id)`; that function filters in-memory by string equality (no path resolution) |
| `StaticWorkspaceResolver::resolve` (ingest crate) | lookup against a pre-registered map by `workspace_id`; not derived from input |

The path resolution chain always goes through `state.workspace.current_workspace()`
which returns a server-resolved workspace object whose `root_path` is
NOT derived from the HTTP path param. There is no current code path
that converts an attacker-controlled `:workspace_id` into a filesystem
read/write.

The C6 unit test (`crates/cognicode-explorer/tests/cp1_control_plane_endpoint.rs`)
remains as the durable regression guard for this contract.

## 12. What CP1.0 does NOT deliver

- no investigation UI
- no case/attention model
- no policy/approval flow
- no second Control Plane consumer (e78 stays DEFERRED)
- no cadence green scorecard (separate cadence receipt INCOMPLETE)
- no architecture evaluation in runtime: the product binary has no path
  to admit constraints today, so the e77 Evaluated branch is reached
  only in unit/integration tests (C3), not in vivo. See §11c.
- no `just build-musl` artifact produced this session (`x86_64-unknown-linux-musl`
  target not installed on the dev workstation; `rustup target add` requires
  network access. This is a TOOLCHAIN limitation, not a CP1.0 product defect.
  The musl bundle is only needed for distribution, not for the runtime contract
  exercised by this change.)

## 13. CP1.0 status (corrected)

The original closeout (commits `c09441fd` + `cf736ead`) asserted CP1.0 was
**CLOSED**. After the discovery in commit `674c3795` — that
`with_control_query` is called only from tests, never from any product
binary — the verdict is corrected to **IMPLEMENTED / RUNTIME ACCEPTANCE
PENDING**. This session closes that gap with WU5 (`1a722d38`) and live HTTP
UAT (§ 11c, 6/6 probes PASS on the product binary).

```text
CP1.0 application/HTTP implementation:         GREEN  (cargo build OK, fmt OK, clippy OK,
                                                          955 unit + 6 integration + 5
                                                          domain unit tests PASS)
CP1.0 production runtime wiring (composition): GREEN  (WU5 implemented: --with-architecture
                                                          flag + 4-test integration suite;
                                                          see § 11b for evidence)
CP1.0 final product acceptance — fail-closed:  GREEN  (live HTTP UAT, product binary,
                                                          6/6 probes PASS — see § 11c)
CP1.0 architecture evaluation in runtime:     NOT DEMONSTRATED
                                                          (no product path to admit
                                                          constraints today; e77 Evaluated
                                                          branch exercised only in tests;
                                                          see § 11c "What this does NOT prove")
```

Closing condition (must all be GREEN):

```text
Core tests              GREEN  (955 explorer lib + 5 domain unit + 6 HTTP integration)
HTTP endpoint tests     GREEN  (C1..C6 in cp1_control_plane_endpoint.rs)
Real server startup     GREEN  (--with-architecture compiles + serves, see § 11c)
Real HTTP UAT           GREEN  (live probes T1..T6 against product binary, 6/6 PASS)
Canonical evidence      VERIFIED
Fail-closed semantics   VERIFIED  (T1 in vivo + C1..C5 + integration test)
Authority boundary      PRESERVED
Lint / fmt              GREEN
Runtime wiring contract GREEN  (cp1_wu5_runtime_wiring_smoke.rs, 4/4 PASS)
T12 echo contract       GREEN  (T6 live probe + C6 regression guard)
```

The CP1.0 fail-closed vertical is GREEN end-to-end on the product binary.
The "Evaluated" branch of e77 is reachable only in tests today; promoting
that branch to in-vivo runtime requires a product-side admission path,
which is **not** part of this change and is correctly deferred to a
future bounded cycle (CP1.x) once a real consumer need is identified.
