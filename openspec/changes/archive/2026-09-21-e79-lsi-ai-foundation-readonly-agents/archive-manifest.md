# Archive Manifest — e79 LSI AI Foundation (read-only agents)

> Cycle: e79 (M11 first slice) | Phase: archive | Date: 2026-09-17
> Closure pattern: **D3-DEFER administrative** (same as e67/e68/e69/e70/e74/e75/e76/e77/e77.1)
> Reason: cycle never instantiated as formal SDDK record with full envelope;
> closure artifacts and durable record are the verification report
> (this manifest + `explore-report.md`) and the on-disk commits.

## Cycle summary

| Phase | Status | Artifact |
|-------|--------|----------|
| explore (ownership map) | DONE | `openspec/changes/e79-lsi-ai-foundation-readonly-agents/explore-report.md` |
| apply | DONE | commit `14a3a721` (impl, 14 files / 3383 insertions) |
| verify (surgical) | DONE | 46 AI-specific tests + 200 nearby subsystem tests pass |
| archive | DONE | this document + `state.yaml` umbrella update |

The cycle did not run a full SDDK ledger because the user's directive was to
ship a focused, read-only foundation under the governed degraded verification
mode (DEBT-SDDK-003 + DEBT-SDDK-005). The honest signal is recorded below.

## What e79 adds

### `domain::ai` (provider-neutral AI domain types)

* `InvestigationFrame` — immutable input (question + `InvestigationScope` +
  `InvestigationBudget` + `ExecutionContext`). FNV-1a content digest produces
  the canonical `InvestigationFrameId`. Rejecting empty questions, unanchored
  workspaces, `SnapshotId::NONE`, and invalid contexts is the constructor's
  job.
* `InvestigationRequest` — bounded `instruction` + declared `tool` surface +
  `RequestProvenance` (caller label echoed verbatim by the port).
* `LlmResponse` — carries `frame_id` + `request_digest` + `request_provenance`
  (echoed) + `response_provenance` (opaque provider/model strings) +
  `observed_read_set` + one of `Hypotheses(Vec<Hypothesis>)` /
  `Critiques(Vec<Critique>)` / `Advisory { summary }`.
* `Hypothesis` — advisory observation (`Suggestion(String)` /
  `Question(String)` / `Candidate { kind, summary }`). Carries NO
  authority fields (no `Admitter`, no `admitted_at`, no
  `DetectorAuthority`, no `DetectorDigest`).
* `Critique` — advisory disposition over an existing `Finding`
  (`Supported` / `WeaklySupported` / `Contradicted` / `MissingEvidence`
  / `NeedsInvestigation`). Carries NO authority fields.
* `LlmPort` — provider-neutral trait. `FakeLlmPort` is the only e79 impl.
  Real provider adapters (OpenAI/Anthropic/Ollama/…) are future work
  gated by **DEBT-SDDK-003**.

### `application::ai` (read-only orchestrators + tests)

* `FakeLlmPort` — deterministic in-memory adapter keyed by
  `(frame_id, request_digest)`. Optional fallback for unkeyed requests.
  The only `LlmPort` implementation in e79.
* `SemanticMiner` — read-only orchestrator. Returns
  `Vec<MinerOutput>` where `MinerOutput` is `Suggestion(Hypothesis) |
  ConstraintCandidate | DetectorCandidate`. **No canonical write
  surface** — the miner cannot commit facts, cannot admit architecture
  constraints, cannot promote detectors.
* `FindingCritic` — read-only orchestrator. Returns `Vec<Critique>`.
  **No canonical write surface** — the critic cannot mutate
  `Finding.status` / `Finding.severity` / `Finding.risk` /
  `DetectorAuthority` / `PolicyGate` result. Those mutations flow
  exclusively through existing deterministic authority paths
  (`Fact::new` constructor, `InMemoryFactStore::commit_batch`,
  `EvidenceStore::append_batch`).
* `boundary_tests` (WU6) — **executable proof** of:
  1. no canonical write surface in miner or critic (return-type IS
     the boundary);
  2. `RequestProvenance` round-trips verbatim into the response
     (lineage pairing invariant);
  3. tool surface is bounded by the declared request (no implicit
     tool promotion);
  4. no provider name (`openai` / `anthropic` / `ollama`) is
     referenced by name from any public type;
  5. wrong `(frame_id, request_digest)` keys error, not silently
     match;
  6. `LlmPort::complete` takes `&self` (immutable borrow — the
     miner/critic cannot mutate the port);
  7. responses whose `frame_id` does not match the request are
     rejected.

### Module exports

* `crates/cognicode-core/src/domain/mod.rs` adds `pub mod ai;`.
* `crates/cognicode-core/src/application/mod.rs` adds `pub mod ai;`.

## Authority boundary (audit results, 2026-09-17)

| Invariant | Status |
|-----------|--------|
| No `LlmResponse -> Fact` path in e79 code | ✅ confirmed |
| No `LlmResponse -> canonical Evidence authority` path | ✅ confirmed |
| No `LlmResponse -> Gated Finding` path | ✅ confirmed |
| No `LlmResponse -> ArchitectureConstraint admitted` path | ✅ confirmed |
| No `LlmResponse -> PromotionPermit` path | ✅ confirmed |
| No `LlmResponse -> source/config apply` path | ✅ confirmed |
| No provider SDK / HTTP client / DTO reference in `domain::ai` or `application::ai` | ✅ confirmed (provider/model are opaque `String`) |
| `SemanticMiner` returns `Hypothesis` / candidate only | ✅ confirmed |
| `FindingCritic` returns `Critique` only | ✅ confirmed |
| No import of `domain::ai` / `application::ai` from `application::architecture/admission.rs` / `application/change_tracking/` / `application/fact_bridge/` / `application/promotion_authority/` | ✅ confirmed |
| The three existing LLM-rejection boundaries from e77.1 (`Fact::new` / `InMemoryFactStore::commit_batch` / `EvidenceStore::append_batch`) are **reused**, not duplicated | ✅ confirmed |

## Lineage audit

The final UAT demonstrates a traceable path:

```
InvestigationFrame (immutable, FNV-1a id)
  → build_request(frame)
  → InvestigationRequest { frame_id, instruction, tools, provenance }
  → request_digest = fnv1a64(instruction || tools || provenance)
  → LlmResponse { frame_id, request_provenance (echoed),
                  request_digest, response_provenance { provider, model },
                  observed_read_set, output }
  → MinerOutput::Suggestion(Hypothesis) | ConstraintCandidate | DetectorCandidate
  → Critique
```

* `HypothesisRef::Fact(FactId)` / `Entity(EntityId)` / `Evidence(EvidenceId)` /
  `Finding(FindingId)` / `ArchitectureConstraint(ArchitectureConstraintId)` are
  the **only** ways an AI observation references canonical state.
* A `Critique` carries a `finding_id: FindingId` and an optional
  `grounding: Vec<HypothesisRef>` — no invented `FactId` / `EvidenceId` is
  trusted merely because the model returned it.
* Unknown / unresolvable references remain unknown / unresolved
  (`CritiqueDisposition::MissingEvidence`).

## Verification

| Suite | Tests | Status |
|-------|-------|--------|
| `domain::ai::` (frame/request/response/hypothesis/port) | 21/21 | PASS |
| `application::ai::` (fake/semantic_miner/critic/boundary_tests) | 25/25 | PASS |
| `domain::architecture::` (no regression) | 7/7 | PASS |
| `application::architecture::` (no regression) | 16/16 | PASS |
| `domain::findings::` (no regression) | 112/112 | PASS |
| `application::findings::` (no regression) | 21/21 | PASS |
| `domain::readset::` (no regression — lineage foundation) | 12/12 | PASS |
| **Total scoped** | **214** | **PASS** |
| `cargo build -p cognicode-core --lib` | 0 errors | GREEN |
| `cargo build --workspace` | 0 errors | GREEN |
| **Full `cargo test --lib`** | **NOT EXECUTED** | **DEBT-SDDK-005 (futex stall)** |

**Honest wording:** the surgical subsystem suites are sufficient for e79
closure under the current governed degraded verification mode (DEBT-SDDK-005).
The full `cargo test --lib` regression sweep was NOT executed; it MUST NOT
be claimed as PASS.

## Umbrella AI task mapping (12.1–12.6)

| Task | Description | Status |
|------|-------------|--------|
| 12.1 | `InvestigationFrame` | **SATISFIED** (domain::ai::frame — immutable, FNV-1a id, anchored scope, bounded budget, 7 unit tests) |
| 12.2 | `LlmPort` | **SATISFIED** (domain::ai::port — provider-neutral trait; `FakeLlmPort` deterministic impl) |
| 12.3 | Semantic Miner prototype | **SATISFIED** (application::ai::semantic_miner — read-only orchestrator; `Vec<MinerOutput>`; 5 unit tests + 2 boundary tests + 1 UAT) |
| 12.4 | Finding Critic prototype | **SATISFIED** (application::ai::critic — read-only orchestrator; `Vec<Critique>`; 6 unit tests + 3 boundary tests) |
| 12.5 | Fix Agent | **NOT STARTED** — e80 |
| 12.6 | Read/tool lineage + security | **PARTIALLY SATISFIED by read-only surface** — the read path (frame → request → response → MinerOutput / Critique) is bounded and proven (WU6 boundary tests). The write-side half — patches, `ChangeProposal`, automated authorship, and `PromotionPermit` minting — **belongs to e80 and is NOT satisfied here.** |

**Important:** 12.6 is NOT claimed complete. The honest split is: read
security + lineage ✅; write security (patches / automated promotion)
remains for e80.

## New debts introduced by e79

* **None structural.** e79 is additive; no canonical write path was
  modified.
* **DEBT-SDDK-003** (live provider adapter) remains the gating debt for
  any real `LlmPort` impl beyond `FakeLlmPort`.
* **DEBT-SDDK-005** (full `cargo test --lib` futex stall) remains
  unresolved; e79 follows the same surgical-subsets contract established
  by e66–e77.1.

## Commits

* **Implementation:** `14a3a721` — `feat(e79): add read-only AI
  investigation foundation` (14 files, 3383 insertions).
  - `crates/cognicode-core/src/domain/ai/{mod,frame,request,response,hypothesis,port}.rs`
  - `crates/cognicode-core/src/application/ai/{mod,fake,semantic_miner,critic,boundary_tests}.rs`
  - `crates/cognicode-core/src/domain/mod.rs` (added `pub mod ai;`)
  - `crates/cognicode-core/src/application/mod.rs` (added `pub mod ai;`)
  - `openspec/changes/e79-lsi-ai-foundation-readonly-agents/explore-report.md`
* **Archive:** (this commit) — archive manifest + `state.yaml` umbrella
  update.

## State of M11 (AI Foundation)

| Sub-area | Status |
|----------|--------|
| 12.1 InvestigationFrame | SATISFIED |
| 12.2 LlmPort | SATISFIED |
| 12.3 Semantic Miner (read-only) | SATISFIED |
| 12.4 Finding Critic (read-only) | SATISFIED |
| 12.5 Fix Agent | NOT STARTED (e80) |
| 12.6 Read lineage + security | PARTIALLY SATISFIED (read-only) |
| Write security (patches / automated authorship / promotion) | NOT STARTED (e80) |

**e79 CLOSED. M11 read-only AI foundation GREEN.**

## State transition

```text
e79 CLOSED
M11 read-only AI foundation GREEN

NEXT: e80 (AutomatedAuthorPolicy + Fix Agent authority review)
e80 implementation BLOCKED pending explicit strategic authorization
```

Retained:

```text
e78 (Pack Ecosystem) DEFERRED (consumer-count checkpoint)
```

`e78` MUST NOT be reopened.

## STOP

Architectural stop after e79. The next review must answer **one** question
before e80 implementation begins:

> **What additional authority is required for a proposal whose author is
> `LlmAgent` or `Plugin`, even when all its tests and gates have passed?**

This is the strategic gate. Real provider adapters and the Fix Agent are
out of scope until that question is settled.

## Files in this archive

* `openspec/changes/e79-lsi-ai-foundation-readonly-agents/explore-report.md`
  — ownership map, classification of 13 candidate surfaces, decision matrix.
* `openspec/changes/e79-lsi-ai-foundation-readonly-agents/archive-manifest.md`
  — this document.
* `openspec/changes/cognicode-living-software-intelligence/state.yaml`
  — umbrella update.
