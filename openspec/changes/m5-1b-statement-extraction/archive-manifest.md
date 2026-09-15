# M5.1b Archive Manifest

> Cycle: `p-c1fac1fea05615c6/m5-1b-statement-extraction`
> Phase: archive (closes cycle)
> Date: 2026-09-14

## Cycle summary

| Property | Value |
|---|---|
| Cycle ID | `p-c1fac1fea05615c6/m5-1b-statement-extraction` |
| Path | A-lite |
| Phases completed | specify → design → tasks → apply (3 WUs) → verify → release → archive |
| Final status | RELEASED → **CLOSED** (after `archive.complete` transition sequence 8) |
| Head SHA | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` |
| Tag | `m5-1b-statement-extraction` (annotated) |
| Tag peel SHA | `6bd72e78` (= HEAD, verified remotely via `git ls-remote origin`) |
| Merge receipt SHA | `6bd72e78` |
| Release receipt SHA | `6bd72e78` |
| **Closed at** | `2026-09-14T21:45:26Z` (post-transition `cycle.status.updated_at`) |
| **Closing transition** | `archive.complete` (sequence 8 for this cycle; ledger event `evt-a68d4e22-429d-4aa4-a437-59f556e58b71`; outcome `succeeded`; `status=CLOSED`, `phase=archive`) |
| **Post-transition ledger** | `event_count=73` (`sddk ledger verify --root . --scope . --format json` after transition; pre-transition count was 72) |
| Verify verdict | `PASS_WITH_WARNINGS` (8 / 8 REQ-STMT-* COMPLIANT, 5 / 5 commands exit 0, 1 warning, 0 critical) |
| Diff stat | **+2031 / -36** across **15 files** (7 commits) |
| Diff digest (sha256 of `git diff --stat 90edff1f..HEAD`) | `ac2ad5b9ae2cd4b95a70d24433d72f161e0d88a14e40c9dcdc58af4b1838e8ac` |
| HEAD digest (sha256 of full SHA) | `06731126030f04e3f80e65f92bfa9a6571414fd44bedc5abf33a15d2d5635a6b` |

## Deliverables shipped

### Production code (5 files)

| File | LOC delta | Role |
|---|---:|---|
| `crates/cognicode-core/src/application/ingest/extractor.rs` | +524 / -7 | Walker: `extract_statements_from_node` + `walk_block` + `find_body_node` (DFS-order Statement emission per Rust tree-sitter node kind, `catch_unwind` per ADR-023) |
| `crates/cognicode-core/src/application/ingest/types.rs` | +24 / -9 | `ExtractionResult.statements_by_function: BTreeMap<NodeId, Vec<Statement>>` with `#[serde(default)]` |
| `crates/cognicode-core/src/application/ingest/extract_stage.rs` | +2 / -1 | Wire-through: `extract_file` populates the new map |
| `crates/cognicode-core/src/application/program_analysis/ast_lift.rs` | +350 / -34 | Lift Pass 4: inject `result.statements_by_function.get(&node.id).cloned()` into each `FunctionLocalView`; fallback `Vec::new()` when missing |
| `crates/cognicode-core/src/application/program_analysis/conformance.rs` | +1 / -1 | Fixture variable alignment (`diamond_backward_slice`: `x` → `a`) |

### Test coverage (delta)

| Surface | M5.1 close | M5.1b close | Δ |
|---|---:|---:|---:|
| `program_analysis::ast_lift` lib tests | 16 | 21 | +5 |
| `ingest::extractor::tests` lib tests | 4 | 9 | +5 |
| Real-source algorithm coverage | 1 of 6 | 6 of 6 | +5 |
| Forbidden I/O imports | 0 | 0 | 0 |

All 5 conformance tests assert a SHA-256 hex digest of the lifted real-source algorithm output against the synthetic corpus baseline (`digest_hex` helper). The 5 conformance tests cover `cfg_per_function`, `dominators_cfg`, `slice_forward`, `slice_backward`, `taint_flow`; the M5.1 `real_source_digest_for_interproc_matches_synthetic` test covers `interproc_summary` — total **6 / 6 algorithm shapes proven end-to-end from real Rust source**.

### Commit chain (7 stacked-to-main commits)

| # | SHA | Subject |
|---:|---|---|
| 1 | `28dce397` | WU1 — `ExtractionResult.statement_by_function` + Rust statement walker |
| 2 | `6b8bee7f` | WU2 — `ast_lift` injects statements from map + 2 module tests |
| 3 | `23d7839e` | WU3 — 5 conformance tests for M5.1b statement extraction |
| 4 | `47b25c86` | docs: M5.1b — implementation-receipt + verification-report + openspec artifacts |
| 5 | `448d81bd` | fix: remove unused let-bindings in 2 conformance tests |
| 6 | `2f1f638a` | docs: update implementation-receipt with orchestrator re-check |
| 7 | `6bd72e78` | docs: SDDK verify report + findings (PASS_WITH_WARNINGS, 4/4 gates passed) |

## Acceptance contract — final state

| Requirement | Status | Evidence |
|---|---|---|
| REQ-STMT-01 — `ExtractionResult.statements_by_function` (serde-default BTreeMap) | satisfied | `types.rs:82`; 9 extractor tests reading the map |
| REQ-STMT-02 — `extract_statements_from_node` walker (all required kinds) | satisfied | 5 walker unit tests cover let, assignment, expression, return, if/match/for/while/block |
| REQ-STMT-03 — Per-function `Statement.id` is 0..N-1 | satisfied | `statement_extraction_id_monotonic` test |
| REQ-STMT-04 — Determinism (same source → same `Vec<Statement>`) | satisfied | `statement_extraction_deterministic` (byte-equal after JSON) |
| REQ-STMT-05 — Lift populates `FunctionLocalView.statements` | satisfied | `lift_injects_statements_from_map` + `lift_falls_back_to_empty_statements` + `real_source_chain_shape_yields_single_function` |
| REQ-STMT-06 — Failure isolation per ADR-023 | satisfied | `statement_extraction_error_isolation` (panic in body → empty Vec, file proceeds) |
| REQ-STMT-07 — No domain I/O, no new ports | satisfied | `grep -rn 'tokio\|sqlx\|reqwest'` returns 0 matches across changed production paths |
| REQ-STMT-08 — All 6 algorithm shapes covered by real source | satisfied | 5 / 5 conformance tests (cfg / dominators / slice-forward / slice-backward / taint) + M5.1 interproc test |

**Result: 8 / 8 REQ-STMT-* requirements are COMPLIANT.**

## Spec delta sync

- `openspec/specs/statement-extraction/spec.md`: **CREATED** (no prior canonical spec existed; the only spec referencing this domain was M5.1's REQ-LIFT-03 deferral note).
- Requirements transferred: **REQ-STMT-01..08** (8 requirements, 9 scenarios SCN-STMT-01..09).
- Source: `openspec/changes/m5-1b-statement-extraction/specs/statement-extraction/spec.md` (sha256 `ea60afeb48f7b9a045facc28391b287cf2414aff5e82e5116933dbd45caf4045`, 154 LOC).
- Destination: `openspec/specs/statement-extraction/spec.md` (sha256 `ea60afeb48f7b9a045facc28391b287cf2414aff5e82e5116933dbd45caf4045`, byte-equal copy — verified by `sha256sum`).
- Modified canonical specs: **none** (no prior `statement-extraction/` directory under `openspec/specs/`).
- Note: M5.1's `openspec/changes/m5-1-ast-lifting/specs/ast-lifting/spec.md` is the only predecessor; its REQ-LIFT-03 was a deferral marker for M5.1b and is now satisfied. No canonical merge was needed for M5.1 because no `openspec/specs/ast-lifting/` directory exists and the M5.1 archive manifest did not perform a delta sync — that is a pre-existing project convention.

## Pre-existing unrelated failures documented (out of M5.1b scope)

| Symbol / file | Status | Owner | Notes |
|---|---|---|---|
| `crates/cognicode-core/src/interface/mcp/mcp_roundtrip_tests.rs:997` (test submodule imports feature-gated `handle_interproc_summary`) | Pre-existing build-config mismatch | not M5.1b | File unchanged in `git diff 90edff1f..HEAD`. With feature enabled (`--features program-analysis-server`) the 8 / 8 `m5_program_analysis_roundtrip` tests pass. |
| `crates/cognicode-core/src/application/program_analysis/conformance.rs:30` (`INTERPROC_SUMMARY` import) | Pre-existing `unused_imports` warning when feature is OFF | not M5.1b | The only modified line in conformance.rs in this cycle is line 182 (fixture variable fix), not line 30. With feature on, the symbol is used. |

Both are documented here per the launch prompt's pre-existing-failures contract; neither blocks the M5.1b verdict.

## Warnings (non-blocking)

- **`ast-lift-loc-cap`** (low, owner: `debt-verify`): `ast_lift.rs` is 845 LOC vs the M5.1 cap of ~545 LOC. Cap breach driven by WU3's 5 conformance tests + 2 helpers (~330 LOC) inside `#[cfg(test)]` submodules `tests::real_source` and `tests::statement_lift`. Production code (lift + node_to_function_view + is_function_kind + lift_rust_source) is unchanged in responsibility. Tracked as a follow-up split into `ast_lift_tests.rs` if test count grows further.

## Receipts linkage

```
implementation-receipt.md (sha256 2d847472…)
       ↓
verify-report.md (sha256 794a2265…) + verify-findings.json (sha256 110151b3…)
       ↓
release-report.md (sha256 ef7fe9ab…)
       ↓
merge-receipt.md (sha256 0c27da2b…)  ← push evidence (HEAD == origin/main)
release-receipt.md (sha256 cd901f8f…) ← annotated tag evidence (remote-verified)
       ↓
archive-manifest.md (this file) ← bound to release-receipt per chain contract
```

## Files inventory (`git diff --stat 90edff1f..HEAD`)

| Status | Bucket | Path | LOC delta |
|---|---|---|---:|
| Modified | `crates/cognicode-core/src/application/ingest/` | `extract_stage.rs` | +2 / -1 |
| Modified | `crates/cognicode-core/src/application/ingest/` | `extractor.rs` | +524 / -7 |
| Modified | `crates/cognicode-core/src/application/ingest/` | `types.rs` | +24 / -9 |
| Modified | `crates/cognicode-core/src/application/program_analysis/` | `ast_lift.rs` | +350 / -34 |
| Modified | `crates/cognicode-core/src/application/program_analysis/` | `conformance.rs` | +1 / -1 |
| Modified | `crates/cognicode-core/src/infrastructure/parser/` | `ansible_handler.rs` | +1 |
| Modified | `crates/cognicode-core/src/infrastructure/parser/` | `terraform_handler.rs` | +1 |
| Added | `openspec/changes/m5-1b-statement-extraction/` | `design.md` | +209 |
| Added | `openspec/changes/m5-1b-statement-extraction/` | `exploration-report.md` | +133 |
| Modified | `openspec/changes/m5-1b-statement-extraction/` | `implementation-receipt.md` | +74 / -3 |
| Added | `openspec/changes/m5-1b-statement-extraction/specs/statement-extraction/` | `spec.md` | +154 |
| Added | `openspec/changes/m5-1b-statement-extraction/` | `tasks.md` | +104 |
| Modified | `openspec/changes/m5-1b-statement-extraction/` | `verification.md` | +71 / -7 |
| Added | `openspec/changes/m5-1b-statement-extraction/` | `verify-findings.json` | +124 |
| Added | `openspec/changes/m5-1b-statement-extraction/` | `verify-report.md` | +244 |

Total: **15 files changed, +2031 / -36 LOC**. The 1-line additions to `ansible_handler.rs` / `terraform_handler.rs` are scoped to the release-cycle meta-context and are not part of the production extraction pipeline.

## Follow-ups (out of M5.1b scope)

1. **ast_lift.rs file split (debt-verify follow-up):** 845 LOC vs M5.1 cap ~545. Owner: `debt-verify`. Forecasted in `tasks.md`. Production responsibility unchanged; the growth is concentrated in `#[cfg(test)]` submodules (`tests::real_source`, `tests::statement_lift`).
2. **mcp_roundtrip_tests.rs build-config mismatch:** pre-existing at `90edff1f`. Owner: not M5.1b. Test submodule imports feature-gated symbols without cfg-gating the submodule.
3. **M5.1 canonical spec delta sync (pre-existing gap):** `openspec/specs/ast-lifting/spec.md` does not exist (M5.1's archive manifest did not perform a delta sync). No spec exists for that domain in the canonical tree. Out of M5.1b scope; surfaced here as a follow-up signal for a future housekeeping cycle if one is planned.

## Cycle closure

This cycle is **CLOSED**. The `archive.complete` transition applied
successfully (sequence 8 for cycle `p-c1fac1fea05615c6/m5-1b-statement-extraction`,
ledger event `evt-a68d4e22-429d-4aa4-a437-59f556e58b71`, outcome `succeeded`,
`status=CLOSED`, `phase=archive`). Post-transition `sddk ledger verify` reports
`event_count=73` (was 72 pre-transition; closing append verified).

**Gate receipts bound to this closure:**

| Gate | Receipt ID | Outcome |
|---|---|---|
| `ledger-valid` | `gate-ledger-valid-0bb2ef6bf2211156-1` | `passed` |
| `vault-index-current` | `gate-vault-index-current-0bb2ef6bf2211156-1` | `passed` |

The M5.1b REQ-LIFT-03 deferral from M5.1 is now **satisfied**: statement-level
def/use extraction is emitted by the tree-sitter walker and consumed by the
`ast_lift` Pass 4, with all 6 algorithm shapes proven end-to-end from real Rust
source.

**Artifact digests (post-finalization):**

| Artifact | Path | sha256 |
|---|---|---|
| archive-manifest | `openspec/changes/m5-1b-statement-extraction/archive-manifest.md` | `86f9c9fe1fe18c6c68d17c01dbf0ef9a942ffc5b11b95722823faa659b92af90` (finalized 178 LOC) |
| canonical synced spec | `openspec/specs/statement-extraction/spec.md` | `ea60afeb48f7b9a045facc28391b287cf2414aff5e82e5116933dbd45caf4045` |
| release-receipt | `openspec/changes/m5-1b-statement-extraction/release-receipt.md` | `cd901f8f41c6b0120f5eff093d88c8f45d75852148ececc6b68979ca0f486a6c` |
| merge-receipt | `openspec/changes/m5-1b-statement-extraction/merge-receipt.md` | `0c27da2b07addb66d2e79b4c697da953de63fe6e2fe61e08eecf3732529fe5c3` |
| release-report | `openspec/changes/m5-1b-statement-extraction/release-report.md` | `ef7fe9ab578cbb4a62ac686f91c49850bab35146d117fd09be0aeb45e5ed22dc` |
| verify-report | `openspec/changes/m5-1b-statement-extraction/verify-report.md` | `794a22658039c156a070823345627e2789b99012152901067f8a56c3a9fc1a50` |
| verify-findings | `openspec/changes/m5-1b-statement-extraction/verify-findings.json` | `110151b338bd9ed0693bc4a5978b2ce63fac681c799169a4b93c4a7b1e144fc0` |
| implementation-receipt | `openspec/changes/m5-1b-statement-extraction/implementation-receipt.md` | `2d847472eb29ef1b7de4a02310707e46655278f6f6682452ae86bc03fe4f38dd` |

## Next

(none — no successor cycle planned in the A-lite sequence. M5.1b is the closure of the M5.1 / M5.1b pair; future M6 work will be opened as a new cycle by the orchestrator.)
