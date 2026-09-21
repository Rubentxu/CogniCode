# e80b Characterization — Fix Agent → ChangeProposal

> Cycle: e80b-lsi-fix-agent-change-proposal | Phase: WU0 (characterize) | Date: 2026-09-17
> Characterize before design. No production code changes in WU0.

## WU0.0 — completion-guard `exit 1` classification

**EXPECTED NONZERO. Not a governance failure. Continue.**

The e80a closure step ended with:

```console
$ git status --short | grep -v "^??"
$ echo "exit=$?"
exit=1
```

`grep -v "^??"` selected **zero** lines because the tracked working tree was
clean; the only entries were untracked (`??`, the local pre-activation SQLite
backups, filtered out). `grep` exits `1` when it selects nothing, so `exit 1`
was the correct signal for a negative check ("no tracked modifications").

Reproduced on demand:

```console
$ git status --short | grep -v "^??" ; echo "exit=$?"
exit=1
$ git status --short ; echo "exit=$?"
?? openspec/changes/e79-1-trust-baseline-hardening/ledger-migration/ledger.pre-migration.sqlite
?? openspec/changes/e79-1-trust-baseline-hardening/ledger-migration/live-pre-activation-v2/
?? openspec/changes/e79-1-trust-baseline-hardening/ledger-migration/live-pre-activation/
exit=0
```

No official SDDK lifecycle/completion guard was involved: e80a closed as an
administrative git-level archive (not an SDDK cycle record), so the step ran
no `sddk release` / `sddk cycle` command. The non-zero status came from the
shell pipeline, not from a failed guard.

## WU0 — inventory of existing surfaces

### AI layer (`domain::ai`, `application::ai`)

| Symbol | Location | Role |
|--------|----------|------|
| `InvestigationFrame` | `domain/ai/frame.rs:208` | immutable bounded input: `execution`, `question`, `scope`, `budget`; `content_digest()` |
| `InvestigationFrameId` | `domain/ai/frame.rs:49` | `u64` content digest id |
| `InvestigationScope` | `domain/ai/frame.rs:128` | `workspace`, `snapshot`, declared refs, `declared_read_set` |
| `InvestigationBudget` | `domain/ai/frame.rs:83` | `max_tool_calls`, `max_context_bytes`, `max_response_bytes` |
| `InvestigationRequest` | `domain/ai/request.rs:84` | `frame_id`, `instruction`, `tools`, `provenance`; `content_digest()` |
| `RequestProvenance` | `domain/ai/request.rs:49` | `frame_id`, `caller`, `issued_at` |
| `LlmResponse` | `domain/ai/response.rs:77` | private fields: `frame_id`, `request_provenance`, `request_digest`, `response_provenance`, `observed_read_set`, `output`. **Not serde** |
| `ResponseOutput` | `domain/ai/response.rs:95` | `Hypotheses \| Critiques \| Advisory`; **is serde** |
| `LlmPort` / `LlmPortError` | `domain/ai/port.rs:65,27` | one bounded call; `FakeLlmPort` is the deterministic adapter |
| `FakeLlmPort` / `ScriptKey` | `application/ai/fake.rs:54,35` | maps `(frame_id, request_digest)` → scripted `LlmResponse` |
| `SemanticMiner` / `MinerError` | `application/ai/semantic_miner.rs:82,57` | read-only orchestrator to mirror |
| `FindingCritic` | `application/ai/critic.rs` | read-only orchestrator |

### Proposal / world / trial surfaces

| Symbol | Location | Role |
|--------|----------|------|
| `ChangeProposal` | `application/change_proposal/proposal.rs:136` | `id`, `base_world`, `proposed_change`, `requested_by` |
| `ProposalKind::SourcePatch` | `.../proposal.rs:53` | `{ patch_ref: String }` — an **opaque** reference |
| `RequestedBy` | `.../proposal.rs:88` | `Human \| Plugin \| LlmAgent`; `is_automated()` |
| `TrialExecutor` / `DefaultTrialExecutor` | `.../change_proposal/executor.rs` | evaluates precomputed evidence; separate abstraction |
| `SoftwareWorld` | `application/software_world/world.rs` | `id`, `base_snapshot` |
| `WorldDiffReport` | `application/software_world/diff.rs:101` | a **`FactDelta`** world diff, not a source patch |
| `portable_execution` | `application/portable_execution/` | does NOT resolve `patch_ref` (only a test mentions it opaquely) |

### Candidate edit / artifact representations

| Symbol | Location | Fit for e80b? |
|--------|----------|---------------|
| `TextEdit` (`FileSystem`) | `domain/traits/file_system.rs:29` | **No** — `lsp_types::Url` + byte offsets; VFS/LSP-oriented, no workspace-relative or budget semantics |
| `TextEdit` (`refactor`) | `domain/aggregates/refactor.rs:273` | **No** — `SourceRange` + `new_text`, LSP-oriented |
| `sandbox_core::artifacts` | `sandbox_core/artifacts.rs` | **No** — scenario result/scoring JSON, not a content store |
| `EvidenceSink` | `domain/findings/ports.rs:48` | **No** — canonical evidence, not a patch artifact |
| `BehaviorEffectSink` | `domain/behaviors/runtime.rs:181` | **No** — behavior effects |
| `ReadSet` | `domain/readset.rs:58` | **Partial** — has `iter` / `contains` / `len`; **no subset API** |

### Explicit answers

```text
1. Is there already a canonical SourcePatch/TextEdit representation?
   NO. The two `TextEdit` types are LSP/VFS-oriented and carry no
   workspace-relative or patch-budget semantics; `WorldDiffReport` is a
   canonical-fact diff, not a source patch.

2. Is there already a store/sink capable of storing a candidate patch and
   returning the opaque patch_ref?
   NO. No artifact/patch store exists. `sandbox_core::artifacts` is
   scenario scoring data.

3. Does portable execution already resolve patch_ref?
   NO.

4. Is patch_ref currently only an opaque future seam?
   YES. `patch_ref` appears only as a `String` field and in one test as
   the literal "wu5-test".
```

## Design decisions derived from WU0

1. **Introduce a minimal typed candidate** — no generalised artifact platform.
   `SourcePatchCandidate { base_scope, edits: Vec<SourceEdit>, rationale }`
   where each `SourceEdit { path, new_content }` is a **full-file replacement**.
   Full-file replacement is chosen deliberately: it removes line-offset
   ambiguity entirely, so "overlapping edits" reduces to "duplicate path",
   which is a clean typed rejection rather than a fuzzy overlap computation.

2. **Add `ResponseOutput::SourcePatchCandidate(...)`** rather than overloading
   `Advisory`. The candidate is serde (it is untrusted plain data), which the
   existing `ResponseOutput` derive already requires.

3. **A separate validation boundary.** The candidate carries raw `String`
   paths (so malformed model output is representable and can be rejected with
   a typed error). Only `ValidatedSourcePatch` carries validated
   workspace-relative paths. Malformed output is never silently repaired.

4. **A minimal `PatchArtifactSink` port** with a deterministic, content-addressed
   in-memory adapter. `PatchRef` is `sha256:<hex>` over the canonicalised patch,
   so identical patches collapse to the same ref (determinism, WU8-N).

5. **`ReadSet` subset check implemented locally** (`observed ⊆ declared`), since
   the type offers no subset API.

6. **The `patch_ref` seam is now resolved for AI proposals only.** `ChangeProposal`
   keeps its shape (`SourcePatch { patch_ref }`); e80b supplies a legitimate way
   to obtain that ref without turning the proposal into the patch container.

## Final composition

```text
LLM response
    ↓
SourcePatchCandidate       untrusted suggestion
    ↓
ValidatedSourcePatch       structurally safe candidate
    ↓
PatchArtifact              inert content (PatchRef)
    ↓
ChangeProposal             intent   ← FixAgent terminal effect
    ↓
TrialEvidence              proof        (external, not FixAgent)
    ↓
PromotionEvaluation        technical readiness (external)
    ↓
PromotionAuthorization     authority        (external, e80a)
    ↓
PromotionPermit            capability       (external, e80a)
    ↓
Apply                      effect           (external)
```

One type answers exactly one question.

## WU0 exit

* Completion-guard `exit 1` classified as EXPECTED NONZERO.
* All four inventory questions answered explicitly.
* Reuse decisions recorded; no existing contract is a semantic fit.
* No production code changed yet.
