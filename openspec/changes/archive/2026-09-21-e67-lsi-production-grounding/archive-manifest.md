# Archive Manifest — cycle e67 — LSI Production Grounding Activation (Rust)

> Cycle: A-lite (degraded-but-governed) | Milestone: M7.6 — first production vertical | Phase: archive | Date: 2026-09-17

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e67 |
| Milestone | M7.6 — first production producer → canonical FactStore vertical slice |
| Requirement | closes the "no production producer emits canonical facts yet" gap documented in the e62.4 archive (M6 honest caveat) |
| Path | A-lite — degraded-but-governed (DEBT-SDDK-003) |
| Base HEAD | `a48674d9` (post-e66 archive-evidence update) |
| Spec delta | none (the cycle refines M6's `REQ-GRD-001` / `REQ-GRD-002` at the producer + gate boundaries; the M6 kernel itself is unchanged) |

## Lifecycle note (honest)

**e67 was never instantiated as a formal SDDK cycle in the ledger.** The three work units
(`86cc0c8c`, `0c874a9c`, `781dc50e`) were authored and committed in sequence with deterministic
checkpoints (`cargo test`, `cargo fmt`, `cargo clippy`, equivalence harness), but no cycle
record exists in `~/.local/state/sddk/projects/p-c1fac1fea05615c6/ledger.sqlite`. The verification
report (already committed at `b364413f`) is the on-disk honest record:

> "e67 was implemented as a bounded vertical slice without a formal SDDK cycle
> (proposal / spec / tasks / verify / archive). This document records the
> honest lifecycle state without fabricating artifacts that were never produced."

This archive commit (`382a0c10` + this manifest) lands the missing proposal / design / spec
artifacts on disk and is the durable record. The SDDK ledger has no e67 row — by design, since
the lifecycle was real but not formalized through `sddk cycle start`.

## Commits

| Commit | Kind | Summary |
|--------|------|---------|
| `86cc0c8c` | `feat(e67)` | WU1 — `application/fact_bridge/production_grounding.rs` (async end-to-end ingestion seam) |
| `0c874a9c` | `feat(e67)` | WU2 — `application/findings/grounded_ast_projection.rs` (async fail-closed join over `core:defines`) |
| `781dc50e` | `feat(findings)` | WU3 — `application/findings/grounded_finding_flow.rs` (adversarial gate via composed grounded finding flow) |
| `b364413f` | `docs(e67)` | verification report and honest lifecycle closure |
| `382a0c10` | `docs(e66,e67)` | land final SDDK cycle artifacts for archive (this commit's sibling) |

## Delivered

### WU1 — async end-to-end production ingestion

- `cognicode_core::application::fact_bridge::production_grounding::ingest_rust_facts`
  (`pub async fn`, signature per `design.md`): `extract_file` → `tree_sitter_facts::collect`
  → `FactBatchBuilder::finish` → `FactStore::commit(ws, snap, batch).await`.
- Returns `GroundedIngestReceipt { snapshot, fact_ids, fact_count }`.
- **First production call-site of `FactStore::commit` in the codebase** (confirmed by grep on
  the cycle's plan phase).
- Async end-to-end invariant (`REQ-DGN-001`): no `Handle::current().block_on`,
  no `Runtime::new().block_on`, no nested executor. `commit(...).await` propagates to the
  caller. Enforced by `rg` static check in the WU1 checkpoint.
- 6 unit tests cover the WU1 invariants (predicates bounded, async propagation, idempotency).

### WU2 — grounded AST projection

- `cognicode_core::application::findings::grounded_ast_projection::project_grounded_ast`
  (`pub async fn`, fail-closed join).
- Joins `ExtractionResult` (syntax-tree detector IR) with `core:defines` facts pinned to the
  same `(workspace, snapshot)`.
- Fail-closed: exactly one match yields `GroundingRef::entity`; zero or multiple matches
  yield `grounding = None` (no fabrication).
- Subject vocabulary is `syntax.function_definition` (detector IR), not `core:defines`
  (kernel vocabulary) — avoids conflation, surfaces the seam explicitly.
- 5 / 5 unit tests pass; equivalence harness 7 / 7 stable; no `Handle::current().block_on`.

### WU3 — adversarial gate via composed grounded finding flow

- End-to-end async composition: `ingest` (WU1) → `grounded projection` (WU2) →
  `admission + promotion` → `detector execution (AstBackend)` → `canonical evidence persistence`
  → `execution finalization` → `finding assembly` → `read-model load` → `verifier gate check`.
- **No new authority path introduced** — WU3 composes the existing authorities (M6
  detector admission, M7.3 behavior authority, M7.4 budget check).
- 10 / 10 tests pass:
  - Positive UAT: full vertical grounded → gateable.
  - Negative UAT: same detector/source with `grounding=None` → refused.
  - 6-case adversarial matrix: ungrounded, wrong-fact, cross-snapshot, subject-mismatch,
    ambiguous canonical source (fail-closed `grounding=None`), refuting evidence.
  - Receipt carries committed fact ids only (sanity guard).

## Acceptance (U67.1 / U67.2 analog)

| Test (U67 analog) | Outcome |
|-------------------|---------|
| `positive_uat_full_vertical_grounded_gateable` | GREEN |
| `negative_uat_ungrounded_finding_refused` | GREEN |
| `adversarial_ungrounded_refused` | GREEN |
| `adversarial_wrong_fact_subject_refused` | GREEN |
| `adversarial_cross_snapshot_fact_id_refused` | GREEN |
| `adversarial_subject_mismatch_finding_refused` | GREEN |
| `adversarial_ambiguous_canonical_source_grounding_none_fail_closed` | GREEN |
| `adversarial_refuting_evidence_refused` | GREEN |
| `receipt_carries_committed_fact_ids_only` | GREEN |

A real Rust source file (`sandbox/fixtures/lsi-grounding/sample.rs`) flows end-to-end through
the kernel: `extract_file` → `tree_sitter_facts::collect` → `FactBatchBuilder::finish` →
`FactStore::commit` → `core:defines` is non-empty → `Finding` carries `GroundingRef::fact(...)` →
verifier accepts → gate is allowed to be blocked.

## Static gates (all clean)

- `rg 'Handle::current\(\)\.block_on' crates/cognicode-core/src/application/fact_bridge/production_grounding.rs crates/cognicode-core/src/application/findings/grounded_ast_projection.rs crates/cognicode-core/src/application/findings/grounded_finding_flow.rs` → no match.
- `cargo fmt --check -p cognicode-core` → clean.
- `cargo clippy -p cognicode-core --all-targets --features evidence-kernel -- -D warnings` → clean on touched paths.

## Related debt

- `docs/debts/DEBT-SDDK-003.md` — why this cycle ran in degraded-but-governed mode (worker
  delegation unavailable; orchestrator executed with deterministic checkpoints).
- `docs/debts/DEBT-SDDK-002.md` — the formal release path remains blocked; this archive
  commit is the durable on-disk record.
- The M6 "honest caveat" ("no production producer emits canonical facts yet") is now closed
  by this cycle for the Rust source path. Other languages and runtimes remain to be wired.
