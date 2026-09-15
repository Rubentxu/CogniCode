# Archive Manifest — cycle e62.4 — canonical grounding, coherence verification (U42, part 2b)

> Cycle: A-lite | Milestone: M6 | Phase: archive | Date: 2026-09-15

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e62.4 |
| Milestone | M6 — Findings & Detector IR (**closed by this cycle**) |
| Requirement | U42 (umbrella `cognicode-living-software-intelligence`) — **closed** |
| Path | A-lite |
| Base HEAD | `b6e97c88` (e62.3 archive) |
| Spec delta | none |

## Commits

| Commit | Kind | Summary |
|--------|------|---------|
| `5f9bb6cf` | `feat(cognicode-core)` | WU2 atomic causal evidence + indexed bindings; WU3 prepare/finalize seam |
| `f86cee55` | `feat(cognicode-core)` | WU4 canonical write bridge; WU5 kernel read model + `EvidenceLookup::resolve`; WU6 coherence verifier + acceptance |
| (archive) | `docs(openspec)` | e62.4 artifacts + state |

## Delivered

### WU2 — atomic causal evidence

- `EvidenceBindings` index-aligned with `produced_evidence`; `EvidenceBinding`
  `Grounded { id, fact }` / `Ungrounded { reason }`; `GroundingFailure`.
- `ProducedEvidence.fact` → `grounding: Option<GroundingRef>`;
  `CausalObservation.fact` removed, so a step's fact is derived from its
  evidence and cannot be declared.
- AST: one atom per construct. Graph: source node + one atom per traversed
  relation + sink node, with parallel relations naming different facts left
  ungrounded (`ambiguous_relation_grounding`). Dataflow: one atom per statement.
- `FindingAssembler` claims only grounded ids.

### WU3 — prepare / finalize

- `DetectorExecutor::prepare` runs admission, validation, planning, the backend
  and both contract checks; `PreparedExecution` has private fields.
- `prepare` + async persist + `finalize` is the only path; `execute()` is that
  path with a sync sink.
- `EvidenceSink::persist(&[ProducedEvidence])` persists a run in one call.

### WU4 — canonical write bridge

- `CanonicalEvidenceWriter::persist`: `FactStore::get` first, entity hint
  checked against the canonical subject, one atomic `append_batch`, ids
  allocated by the store, store errors preserved as errors.

### WU5 — read model

- `EvidenceGrade` lifted to ungated `domain::kernel_ids` (shim in
  `evidence_kernel::evidence`).
- `FactDescriptor` / `FactSlot` / `EvidenceDescriptor` / `EvidenceResolution`;
  `EvidenceLookup::resolve`.
- `KernelEvidenceReadModel::load` performs the only I/O and records missing ids
  and dangling facts rather than raising, keeping `FindingVerifier` sync.

### WU6 — coherence verification

- Evidence: resolves, does not refute, is not dangling, fact snapshot matches
  the execution scope.
- Causal step: grounded in both evidence and fact, in the finding's evidence
  set, evidence grades exactly the claimed fact, subject agrees when named.

## Evidence

| Check | Result |
|-------|--------|
| `domain::findings` | 119 passed (ungated and gated) |
| `application::findings` (gated) | 21 passed |
| `evidence_kernel` (gated) | 88 passed |
| AST / Graph / Dataflow / Axiom E2E | 8 / 4 / 7 / 3 passed |
| **`findings_canonical_grounding_e2e`** (gated, acceptance) | **10 passed** |
| `cargo check --workspace --all-targets` | 0 errors (gated and ungated) |
| `cargo fmt --all --check` | clean |
| `scripts/check_known_failures.py` | exit 0 — 41 entries, unchanged |

## Durable knowledge learned

- A causal chain cannot be verified if its evidence is a single blob per path:
  grounding forces one atom per element of the witness, and the graph paradigm
  needs the *relation* grounded, not just the nodes.
- The binding list must stay index-aligned with what the backend produced;
  compacting ungrounded entries silently repoints matches.
- A fact stated by a backend is a second source of truth for the same claim and
  must be removed, not merely overridden.
- Evidence ids must be allocated where snapshot-wide uniqueness is visible (the
  store), never by the producer or per execution.
- Storage failures and "nothing found" must be distinguishable; a bridge must
  never degrade a store error into an ungrounded item.
- `Refutes` is not a weaker `Supports`: accepting evidence because it exists is
  wrong.
- Ungrounded is a legitimate value, not an error: the analysis result is still
  real and worth explaining, it just cannot open a gate.

## Deferred (open work, recorded)

- Grounding from the production producers (M5, graph/DAG lift, real tree-sitter
  extractor) so real findings can gate. Today they are ungrounded by
  construction and cannot block: strictly more conservative than before.
- `ExecutionPlan<Vec<Stage>>`; `LegacyRuleProvenance` into `Finding` (needs an
  execution-context carrier; the IR cannot carry it without corrupting the
  semantic digest).

## Milestone status

M6 (Findings & Detector IR) tasks 7.1–7.7 complete; U40, U41, U42, U43, U47 now
all satisfied. M6 is **done**; the next work belongs to the following milestone.
