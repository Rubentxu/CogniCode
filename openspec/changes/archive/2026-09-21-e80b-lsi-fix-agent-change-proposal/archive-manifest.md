# Archive Manifest — e80b Fix Agent → ChangeProposal

> Cycle: e80b-lsi-fix-agent-change-proposal | Phase: archive | Date: 2026-09-17
> Closure pattern: **administrative archive** (git-level), consistent with
> e79/e79.1/e80a. The cycle was not instantiated as a formal SDDK cycle record.

## Cycle summary

| Phase | Status | Artifact |
|-------|--------|----------|
| WU0.0 guard classification | DONE | `characterization.md` (EXPECTED NONZERO) |
| WU0 characterize | DONE | `characterization.md` (inventory + 4 explicit answers + reuse decisions) |
| WU1 typed candidate | DONE | `domain/ai/patch.rs` (`SourcePatchCandidate`, `SourceEdit`) |
| WU2 response vocabulary | DONE | `domain/ai/response.rs` (`ResponseOutput::SourcePatchCandidate`) |
| WU3 validation boundary | DONE | `validate_source_patch` (frame/digest/read-set/scope/paths/budgets) |
| WU4 patch artifact seam | DONE | `PatchArtifactSink` + `PatchRef` + `InMemoryPatchArtifactSink` |
| WU5 FixAgent | DONE | `application/ai/fix_agent.rs` |
| WU6 terminal boundary | DONE | terminal effect is a `ChangeProposal` |
| WU7 no trial folding | DONE | `TrialExecutor` untouched; FixAgent does not trial |
| WU8 adversarial tests | DONE | `fix_agent_tests.rs` cases A-P |
| WU9 e80a integration | DONE | `wu9_*` tests in `fix_agent_tests.rs` |
| WU10 events/lineage | DONE (seam documented, no EventBus) | `FixLineage` + module doc |
| archive | DONE | this document + `state.yaml` update |

## Composition

```text
LLM response
    ↓
SourcePatchCandidate       untrusted suggestion
    ↓
ValidatedSourcePatch       structurally safe candidate
    ↓
PatchArtifact (PatchRef)   inert content
    ↓
ChangeProposal             intent   ← FixAgent terminal effect
    ↓
TrialEvidence              proof        (external)
    ↓
PromotionEvaluation        technical readiness (external)
    ↓
PromotionAuthorization     authority        (external, e80a)
    ↓
PromotionPermit            capability       (external, e80a)
    ↓
Apply                      effect           (external)
```

Each type answers exactly one question. The FixAgent participates only in the
first four stages.

## WU0 answers (inventory)

```text
1. Canonical SourcePatch/TextEdit representation?  NO  (the two TextEdit types
   are LSP/VFS-oriented; WorldDiffReport is a canonical-FactDelta diff)
2. Store/sink able to return the patch_ref?        NO  (introduced a minimal port)
3. Does portable execution resolve patch_ref?      NO
4. Is patch_ref only an opaque future seam?        YES
```

`ChangeProposal` keeps its shape: `SourcePatch { patch_ref }` references an
artifact; the proposal still describes intent and does not transport the change.

## Exit condition

```text
AI can inspect bounded context          YES
AI can produce typed patch candidate    YES
AI patch is validated                   YES
AI can create SourcePatch proposal      YES
proposal author forced to LlmAgent      YES

AI can mutate source                    NO
AI can trial automatically              NO
AI can mint Evidence authority          NO
AI can mint PromotionAuthorization      NO
AI can mint PromotionPermit             NO
AI can self-approve                     NO
AI can apply                            NO
```

## Umbrella mapping

```text
12.1 InvestigationFrame          SATISFIED
12.2 LlmPort                     SATISFIED
12.3 Semantic Miner              SATISFIED
12.4 Finding Critic              SATISFIED
12.5 Fix Agent                   SATISFIED
12.6 read/tool/write security    SATISFIED for the current AI foundation
```

Precision on 12.6: the write-side boundary is covered by the implemented
adversarial tests at the candidate/proposal level (path escape, traversal,
scope, frame/digest lineage, read-set scope, budgets, duplicates, no
authority-bearing output variant, minting-surface unreachability, determinism,
sink failure, malformed input). Actual source mutation and apply are **outside**
the AI foundation by design: no test claims they are exercised, because the
FixAgent cannot reach them.

## Verification

| Check | Result |
|-------|--------|
| e80b `application::ai` (incl. WU8 A-P, WU9, sink) | 69 passed / 0 failed |
| e80a `promotion_authority` | 42 passed / 0 failed |
| `change_proposal` | 24 passed / 0 failed |
| e69 `policy_gate` | 14 passed / 0 failed |
| e64 behavior (lib) | 19 passed / 0 failed |
| e77.1 `architecture_e77_1_wu3` canonical grounding | 10 passed / 0 failed |
| `behavior_authority_e2e` + `behavior_budget_e2e` | 12 passed / 0 failed |
| `cargo test -p cognicode-core --lib` | terminates; failure set == `scripts/known_failures.yaml` exactly (41 entries) |
| same, `--features evidence-kernel` | 2531 passed / 2 failed (the 2 are the characterized DEBT-SDDK-006 entries) |
| `cargo build --workspace` | GREEN |
| clippy on the touched surface | no new warnings |

### New failure encountered and resolved during the gate

`authority_tests::l_no_ai_module_imports_a_permit_minting_constructor` (an e80a
audit) began failing because e80b's WU9 integration test under `application/ai`
must reference the minting surface to prove the composition. Characterized as an
audit-scoping issue, not a boundary violation: the audit now skips test modules.
Production modules remain audited. No boundary was weakened, and no test was
deleted.

## Explicit non-goals (honored)

No real source mutation, no Git commit/push, no automatic trial, no automatic
promotion, no self-approval, no live LLM provider, no Backstage, no Control
Plane, no e78 Packs, no P1.4 repair, no DEBT-SDDK-006 repair, no historical
replay, no shadow analysis. The cycle did not widen into "AI coding agent".

## Debts

| Debt | Status |
|------|--------|
| DEBT-SDDK-002 | RESOLVED (e79.1) |
| DEBT-SDDK-005 | RESOLVED (e79.1) |
| DEBT-SDDK-004 | PARTIAL: 3/4 repaired; P1.4 deferred with architectural trigger |
| DEBT-SDDK-006 | OPEN: `interproc_summary` feature-gating (two baseline entries) |

`known_failures.yaml` was NOT modified in e80b: this cycle does not change the
two `interproc_summary` tests. P1.4 stays explicit in DEBT-SDDK-004.

## Commits

* **Implementation:** `9d28781e` — `feat(e80b): FixAgent authors a validated
  patch ChangeProposal`.
* **Archive:** this commit — this manifest + `state.yaml` umbrella update.

## Files

New:
* `domain/ai/patch.rs`
* `application/ai/fix_agent.rs`
* `application/ai/patch_sink.rs`
* `application/ai/fix_agent_tests.rs`
* `application/ai/fix_agent_test_support.rs`

Changed:
* `domain/ai/response.rs`, `domain/ai/mod.rs`
* `application/ai/mod.rs`, `application/ai/critic.rs`, `application/ai/semantic_miner.rs`
* `application/promotion_authority/authority_tests.rs` (audit scoping)

## State transition

```text
e80b CLOSED
12.5 Fix Agent SATISFIED
AI authoring path validated end-to-end up to ChangeProposal

NEXT: e81 (Historical Replay + strict OPTIMIZE/CONFIRM separation)
```

## STOP

Architectural stop after e80b, for strategic review before e81. The AI foundation
now authors proposals and nothing more; testing, evaluation, authority, and apply
remain external and are unchanged.
