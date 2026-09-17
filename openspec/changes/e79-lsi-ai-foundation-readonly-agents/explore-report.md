# WU0 — AI Ownership Map (e79 input)

> Cycle: A-lite | Phase: explore → ownership map | Date: 2026-09-17
> **No code is written in WU0. This document is the only artifact of WU0.**

## Purpose

Before adding types for `InvestigationFrame`, `LlmPort`, `Hypothesis`,
`SemanticMiner`, or `FindingCritic`, inventory every existing AI/LLM-related
surface so the new abstractions either (a) reuse them, (b) adapt them, or
(c) are justified as separate.

## Method

Targeted `agentgrep` searches over `crates/`, `docs/adr/`, `docs/`, and the
openspec specs. Each candidate surface is classified into one of:

```text
REUSE            → e79 uses this surface as-is
ADAPT            → e79 wraps or extends this surface
KEEP SEPARATE    → intentionally distinct, e79 will not duplicate
OBSOLETE         → exists but should be removed in this cycle (none expected in WU0)
```

If a classification would be `OBSOLETE`, e79 must record it as DEBT.

## Scope of this ownership map

The user directive (`e79 authorization 2026-09-17T09:46Z`) listed eight
candidates. I inventory each one in turn. I also add a ninth section for
anything surfaced by the search that should be recorded but was not on the
list.

## 1. Explorer `brain_session` — KEEP SEPARATE

**Search**: `grep -r brain_session` returned **0 matches** in the workspace.

The user's directive said:

> `brain_session` ya gestiona contexto conversacional en Explorer

This is **not corroborated by the current source tree**. There is no
`brain_session` module, function, struct, or constant in any crate under
`crates/cognicode-explorer/src/` or anywhere else in the workspace.

**Classification**: KEEP SEPARATE — but with a documented honesty signal.

**Action**:
- e79 does **not** invent a `brain_session` (that would be fabrication).
- e79 creates `InvestigationFrame` and `LlmPort` as provider-neutral
  capabilities that any future conversation layer (Explorer or otherwise)
  may later consume.
- The explorer's hypothetical conversational surface, if/when it exists,
  will be a *consumer* of `LlmPort`, not its origin.

## 2. Explorer lenses (hypothesis-style) — REUSE framing; KEEP SEPARATE concrete

**Found**:
- `crates/cognicode-explorer/src/domain/lens.rs` — `Lens` trait,
  `LensContext`, `LensRegistry`. Doc comment explicitly says:
  *"Lenses are **hypotheses, not verdicts** — they surface observations
  the human reader can interpret, never declarative claims."*
- `crates/cognicode-explorer/src/domain/lenses/{hotspots,architecture,dependencies}.rs`
  — three lenses that emit `DesignFinding` objects framed as hypothesis.
- `crates/cognicode-explorer/src/domain/dto.rs` — `DesignFinding` DTO
  (Explorer-side; distinct from cognicode-core's `Finding`).

**Classification**:
- The **hypothesis framing** is REUSE: Explorer's lens model is already the
  canonical "hypothesis, not verdict" pattern in the codebase. e79's
  `Hypothesis` and `FindingCritic` types will use the same vocabulary
  (hypothesis = observation the reader interprets, not authority).
- The concrete Explorer `DesignFinding` is KEEP SEPARATE: it is a
  read-side projection for the human UI, not a canonical fact in the
  evidence kernel. e79 does **not** rewrite Explorer's lenses to "unify
  names" — the user directive explicitly forbids that.

**Action**:
- e79's `Hypothesis` type lives in `domain::ai` (cognicode-core), is
  **not** a `DesignFinding`, and is consumed only by `SemanticMiner`,
  `FindingCritic`, and the lineage log.
- e79 does not touch Explorer's lens module.

## 3. `ProducerKind::LlmAgent` — REUSE (the rejection boundary is the load-bearing piece)

**Found**:
- `crates/cognicode-core/src/domain/evidence_kernel/fact.rs:51` —
  `ProducerKind::LlmAgent` variant.
- `fact.rs:134` — `Fact::new` rejects `ProducerKind::LlmAgent` with a
  typed `FactError::LlmProvenanceRejected`.
- `fact.rs:9-10` (module doc) and `fact.rs:235-265` — invariant documented:
  *"LLM output stays a Hypothesis or AgentEvidence and is never persisted as
  an extracted Fact."*
- `crates/cognicode-core/src/infrastructure/evidence_kernel/in_memory.rs:97-111`
  — the in-memory adapter ALSO rejects `LlmAgent` provenance at
  `commit()`. The rejection is enforced at TWO boundaries (the
  domain constructor and the store adapter).
- `crates/cognicode-core/src/application/fact_bridge/batch_builder.rs:131-193`
  — `add_observation` and `add_tiered_observation` reject `LlmAgent`
  provenance at the bridge boundary too. THREE boundaries.

**Classification**: REUSE — this is the load-bearing piece of the
read-only boundary. e79 cannot and should not weaken it.

**Action**:
- e79's `LlmPort` response carries provenance metadata (model id,
  request digest, tool reads) that downstream code may convert into a
  `ProvenanceRecord` with `ProducerKind::LlmAgent` — but only to attach
  to a `Hypothesis` or to `RecordHypothesis` behaviour effect, never to
  a `Fact`.
- No change to `Fact::new` rejection logic. No new code path that
  bypasses it.

## 4. `EvidenceKind::Hypothesis` — REUSE (existing canonical mapping)

**Found**:
- `crates/cognicode-core/src/domain/findings/outcome.rs:33` —
  `EvidenceKind::Hypothesis` variant.
- `outcome.rs:51` — maps to `EvidenceClass::D` (lowest).
- `crates/cognicode-core/src/domain/findings/finding.rs:35` —
  `EvidenceClass::D` doc: *"Hypothesis only (heuristic / LLM-suggested)."*

**Classification**: REUSE — the canonical mapping for hypothesis-class
evidence already exists. e79 must route every AI-derived piece of evidence
through `EvidenceKind::Hypothesis` (and only that).

**Action**:
- e79 does **not** create a new evidence kind for AI output.
- If `SemanticMiner` produces an evidence-bearing claim, it must be
  `ProducedEvidence { kind: EvidenceKind::Hypothesis, ... }` and the
  resulting finding is `EvidenceClass::D` (the canonical verifier already
  treats class `D` as "can never gate").
- The e77.1 `ArchitectureSource` variant (added for static architecture)
  stays distinct from `Hypothesis`.

## 5. `BehaviorEffect::RecordHypothesis` — REUSE (existing read-only effect)

**Found**:
- `crates/cognicode-core/src/domain/behaviors/runtime.rs:125` —
  `BehaviorEffect::RecordHypothesis { summary }`.
- `class.rs:88-103` — `BehaviorEffectKind::RecordHypothesis` with the
  authority table at `class.rs:143-160`:
  - `PureDerivation`: **may not** record hypothesis.
  - `ReactiveAnalysis`: may record hypothesis.
  - `AgentBehavior`: may record hypothesis.
- `behavior_budget_e2e.rs:578` — an integration test exercising the
  ReactiveAnalysis → RecordHypothesis path with a budget.

**Classification**: REUSE — this is the canonical "behavior says
something that is not canonical truth" surface.

**Action**:
- e79's `SemanticMiner` runs as a `Behavior` (deterministic, synchronous
  as the user directive requires) that emits `RecordHypothesis` effects.
- e79 does **not** need to mint canonical facts, gate findings, or apply
  patches — it needs only `RecordHypothesis` (and possibly `RecordEvidence`
  with `EvidenceKind::Hypothesis` if the miner emits grounding references).

## 6. `RequestedBy::LlmAgent` (ChangeProposal author) — KEEP SEPARATE (M9 seam)

**Found**:
- `crates/cognicode-core/src/application/change_proposal/proposal.rs:88-107`
  — `enum RequestedBy { Human { .. } | Plugin { .. } | LlmAgent { .. } }`.
- `proposal.rs:115-135` — `is_automated` and `class_tag` distinguish
  automated authors (Plugin, LlmAgent) from human authors.

**Classification**: KEEP SEPARATE — this is the M9 (ChangeProposal / Trial)
seam. e79 does not create proposals (proposals are e80 / Fix Agent territory
per the user directive). e79 does **not** rewrite `RequestedBy`.

**Action**:
- e79 records its author as `ActorRef::agent(...)` in any lineage event
  (Intelligence Log) — not as `RequestedBy::LlmAgent`, because e79 does
  not create proposals.
- If a future e80 cycle derives a `ChangeProposal` from a hypothesis, that
  derivative will mark itself `RequestedBy::LlmAgent`, **not** e79.

## 7. `ReadSet` and `ReadSetRecorder` — REUSE (lineage foundation)

**Found**:
- `crates/cognicode-core/src/domain/readset.rs` — `ReadSet` bounded,
  ordered, deduped, truncated set of `FactId`s a behavior actually read.
- `crates/cognicode-core/src/domain/ports/read_set_recorder.rs` —
  `ReadSetRecorder` and `InvalidationQuery` ports.
- `crates/cognicode-core/src/application/change_tracking/planner.rs` —
  already used by the affected-work planner.

**Classification**: REUSE — the lineage foundation e79 needs. AI reads
canonical knowledge, so its `ReadSet` is exactly the same shape as any
other behavior's.

**Action**:
- `InvestigationFrame` records the `ReadSet` the agent was *allowed* to
  read (declared intent).
- The `LlmPort` records the `ReadSet` the agent *actually* read during
  the response (observed lineage). The intersection is auditable.
- e79 does **not** introduce a second lineage model. The Intelligence Log
  (M7.1) is the destination of read events.

## 8. MCP tools / provider seams — KEEP SEPARATE (out of scope for e79)

**Search**: `grep -r openai|anthropic|ollama` returned **0 matches**. There
is no provider-specific code in the workspace today.

**Classification**: KEEP SEPARATE — e79 does not ship a real provider
adapter. A real provider adapter (if ever) is a spike/smoke test, not a
gate (per user directive: *"A real provider adapter may be a smoke
test/spike, not a gate. DEBT-SDDK-003 provider/worker outage must not
block deterministic verification."*).

**Action**:
- e79 ships an in-memory deterministic `FakeLlmPort` only.
- The deterministic fake is what the acceptance suite uses.
- No new external SDK dependency in `cognicode-core/Cargo.toml`.
- No HTTP client code in `domain::ai`.

## 9. Find-pass extras — observations not on the original list

### 9.1 `Lenses consume existing ports; they do not introduce new ones`

`LensContext` (`crates/cognicode-explorer/src/domain/lens.rs:33-44`) takes
`SymbolRepository`, `QualityStore`, `SourceReader`, and optionally
`GraphQueryPort`. This is exactly the pattern e79's `InvestigationFrame`
should follow: it should reference existing ports (or wrappers around them)
rather than introduce new ones.

### 9.2 `FactStore::commit` and `EvidenceStore::append_batch` are the canonical write paths

e79 must not introduce a parallel write path. The e77.1 corrigendum just
closed a synthetic-minting path; e79 must not reopen it under a different
name.

### 9.3 The Intelligence Log already records *behavior* events with lineage

`crates/cognicode-core/src/domain/intelligence_log/` has `IntelligenceEvent`
with `caused_by`, `correlation`, `actor`, and inline-or-artifact payload.
e79's lineage events (`investigation.started`, `llm.requested`,
`llm.responded`, `hypothesis.recorded`, `finding.criticised`) fit this
shape and should be emitted through the existing `IntelligenceEventStore`,
not a new lineage store.

### 9.4 The change-tracking planner already consumes `ReadSet`

`change_tracking/planner.rs` decides whether a work item is Affected,
Unaffected, or Unknown based on its read set. e79's `SemanticMiner` and
`FindingCritic` could be modelled as read-only work items in the future
planner (out of scope for this cycle, but the substrate exists).

### 9.5 ADRs and roadmap references

There is no ADR-`llm` / `ai-foundation` / `e79` in `docs/adr/` today. The
roadmap entry for `e79` in `state.yaml` was authored in the e77.1 archive
commit and references only the cycle goal — no prior design.

## Summary classification table

| # | Surface | Class | Where it lives |
|---|---------|-------|----------------|
| 1 | Explorer `brain_session` | KEEP SEPARATE | (does not exist in source — honest signal) |
| 2 | Explorer lenses (`Lens`, `DesignFinding`) | REUSE framing, KEEP SEPARATE concrete | `cognicode-explorer/src/domain/lens.rs`, `lenses/*` |
| 3 | `ProducerKind::LlmAgent` rejection | REUSE | `domain/evidence_kernel/fact.rs` (3 boundaries) |
| 4 | `EvidenceKind::Hypothesis` (class D) | REUSE | `domain/findings/outcome.rs` |
| 5 | `BehaviorEffect::RecordHypothesis` | REUSE | `domain/behaviors/runtime.rs`, `class.rs` |
| 6 | `RequestedBy::LlmAgent` | KEEP SEPARATE | `application/change_proposal/proposal.rs` (e80 substrate) |
| 7 | `ReadSet` / `ReadSetRecorder` | REUSE | `domain/readset.rs`, `domain/ports/read_set_recorder.rs` |
| 8 | Real provider SDKs (OpenAI/Anthropic/Ollama) | KEEP SEPARATE | not present in source; out of scope for this cycle |
| 9.1 | `LensContext` pattern (existing ports, no new ones) | REUSE | as a *pattern*, not as code |
| 9.2 | Canonical write paths (FactStore.commit, EvidenceStore.append_batch) | REUSE | do not bypass |
| 9.3 | Intelligence Log lineage substrate | REUSE | `domain/intelligence_log/` |
| 9.4 | Change-tracking planner + ReadSet | REUSE | `application/change_tracking/planner.rs` (out of scope for this cycle) |
| 9.5 | ADRs and roadmap | n/a (none exist) | honest signal |

## What this map tells e79

1. **The "hypothesis" semantic already has two homes**: Explorer lenses
   (UI hypothesis) and `EvidenceKind::Hypothesis` (canonical evidence
   class). e79 introduces a **third** home (`domain::ai::Hypothesis`),
   but it must be reconciled with the canonical mapping: every
   `Hypothesis` carries a derived `EvidenceKind::Hypothesis` (class D)
   if it bears references, and is otherwise a pure advisory observation.

2. **The LLM rejection boundary is already enforced at three points**.
   e79 must not create a fourth point; instead, e79's `LlmPort` records
   provenance such that the three existing points (Fact::new, in-memory
   commit, batch_builder.add_observation) all reject any attempt to
   persist LLM-derived data as canonical truth.

3. **The Intelligence Log and `ReadSet` are the lineage substrate**.
   e79's lineage model = existing log + existing read set + new
   request/response digests. No second lineage model.

4. **Behaviour-driven execution is the test contract**. `SemanticMiner`
   and `FindingCritic` are behaviours that emit `RecordHypothesis`
   effects (and optionally `RecordEvidence` with hypothesis-class
   evidence). They never emit `CommitCanonicalFact` — that is
   structurally impossible because they run as `AgentBehavior` or
   `ReactiveAnalysis`, both of which the authority table forbids
   from committing facts.

5. **No provider SDKs enter `cognicode-core`**. `LlmPort` is a port;
   the only implementation shipped in e79 is an in-memory deterministic
   fake. Real providers are a future spike, gated by DEBT-SDDK-003.

6. **The explorer's hypothetical `brain_session` does not exist**. e79
   does not invent it; e79 builds `InvestigationFrame` and `LlmPort`
   as provider-neutral capabilities a future conversation layer may
   consume.

## Honest signals (the things that will be debts if we ignore them)

- **The user's mental model includes a `brain_session`**. It is not
  in the current source. If the user wants `brain_session` to exist as
  a real Explorer's conversational surface, that is a separate cycle
  (and a real e79 consumer of `LlmPort`).
- **Real provider adapters are out of scope**. If the user later wants
  OpenAI/Anthropic/Ollama adapters, that is a future cycle, not e79.
- **`SemanticMiner` cannot produce admitted `ArchitectureConstraint`**
  per the user directive. If the miner produces a constraint candidate,
  it must be a `ConstraintCandidate` (e77 domain type), not an admitted
  `ArchitectureConstraint`. The same applies to detector ideas — they
  must be candidate definitions, never admitted detectors.

## WU0 closure

WU0 is DONE when this document is committed under
`openspec/changes/e79-lsi-ai-foundation-readonly-agents/`.

After WU0, the next phases can begin:
- WU1: `InvestigationFrame` (bounded immutable input).
- WU2: `LlmPort` (provider-neutral port + in-memory fake).
- WU3: `Hypothesis` model (reconcile with existing).
- WU4: `SemanticMiner` (read-only prototype).
- WU5: `FindingCritic` (critique dispositions).
- WU6: lineage + security.
- WU7: read-only vertical UAT.
