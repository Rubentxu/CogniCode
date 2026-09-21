# e69 — EvidenceBundle + PolicyGate

## Status

```text
e68 implementation   CLOSED
e68 verification     PASS
e69 implementation   CLOSED
e69 verification     PASS
e68 SDDK lifecycle   not formally instantiated
e69 SDDK lifecycle   not formally instantiated
```

## Goal

Convert existing work results (cargo test, just, cogh, analysis runs)
into structured evidence and take a policy decision over an
`EvidenceBundle`, **without yet building a new CI**.

```
WorkResult
  ├── cargo test
  ├── detector run
  ├── build result
  └── analysis result
        ↓
  EvidenceProducer (one per source)
        ↓
   EvidenceBundle
        ↓
    PolicyGate
        ↓
  Pass | Warn | Block | InsufficientEvidence
```

## Non-goals (deferred to e70+)

- GitHub Actions remote.
- Jenkins, distributed workers.
- A scheduler with retries.
- A persisted bundle (in-memory only; persistence is e70+).

## Architectural decisions

1. **`EvidenceBundle` is application-layer (shape B, not A or C)**.
   The bundle is a **derived operational aggregation**, not a canonical
   Fact. We do not write to the Evidence Kernel from the bundle.
   - A new type `application::evidence_bundle::EvidenceBundle` with
     `Vec<BundleEntry>`.
   - One `EvidenceBundleId` per bundle (opaque u64, allocated in
     process — persistence is e70+).

2. **`BundleEntry` distinguishes FAILED, MISSING, UNKNOWN, Evidence**
   as first-class citizens. Per the user's explicit emphasis: a failed
   producer is not the same as a missing one is not the same as an
   unknown/incomplete one.
   ```rust
   pub enum BundleEntry {
       ProducerFailed { source: ProducerSource, reason: String, raw: Option<String> },
       ProducerMissing { source: ProducerSource, why_unreachable: String },
       Evidence { source: ProducerSource, descriptor: EvidenceDescriptor, grade: EvidenceGrade },
       ProducerUnknown { source: ProducerSource, detail: String },
   }
   ```
   The bundle NEVER collapses MISSING/UNKNOWN into Evidence. The gate
   is the only authority that decides what to do with each kind.

3. **`EvidenceProducer` is a trait with one impl per source**.
   Errors are local to the producer; they translate to `BundleEntry`
   variants. The bundle aggregator never aborts because of one
   producer's failure — it logs the entry and moves on. This matches
   the fail-closed-but-not-fail-loud expectation.

4. **`PolicyGate` produces a structured `PolicyDecision`**:
   `Pass | Warn | Block | InsufficientEvidence`. The decision carries
   a structured `Reason` per BundleEntry (which rule fired, which
   evidence was missing, etc.) so downstream consumers (e70's
   `why_decided`) can render it without reconstructing.

5. **`absence is never success`**: any MISSING/UNKNOWN entry in a
   bundle where Evidence is required → `InsufficientEvidence` at
   minimum, never `Pass`. The gate has explicit rules; the user can
   pre-declare what each gate requires.

## Architecture budget

- Crates/modules touched:
  - `crates/cognicode-core/src/application/evidence_bundle/{mod,types,producer}.rs`
  - `crates/cognicode-core/src/application/policy_gate/{mod,gate}.rs`
- New domain types: **none**.
- New dependencies: **none**.
- APIs NOT modified:
  - `Fact`, `Evidence`, `Finding`
  - `DetectorAdmission`, `CanonicalEvidenceWriter`, `KernelEvidenceReadModel`
  - `FactStore`, `EvidenceStore`, `ReadSet`
  - `FactDelta`, `AffectedWorkPlan`, `WorkId`
  - All existing `application::*` modules.

## Risk budget

- Allowed uncertainties:
  - Exact rule set of the initial `PolicyGate` (user can refine
    in e70 once real workloads feed back).
- Mandatory fallbacks:
  - Any MISSING/UNKNOWN on a required-evidence slot → `InsufficientEvidence`.
  - Any FAILED on a required-evidence slot → `Block`.
- Tolerable regressions: none.
- Invariant metrics:
  - `just lsi-equivalence` 7/7.
  - e66/e67/e68 tests remain green.
  - Findings E2E suites remain green.
  - Intelligence event log E2E remains green.

## Decomposition (three WUs)

### WU1 — EvidenceBundle + BundleEntry

Pure data types. No I/O. Ordering stable per `(source, slot_id)`.

UAT:
- empty bundle is well-formed
- FAILED + MISSING + Evidence coexist without collapsing
- is_pass-ready() returns false if any entry is FAILED/MISSING/UNKNOWN
- bundle ordering is stable across reordering of entries
- EvidenceBundleId is allocated and unique per bundle within a process

### WU2 — EvidenceProducer trait + local producers

- One `EvidenceProducer` impl per source (cargo test, just, cogh,
  analysis).
- Aggregation runs every registered producer; errors translate to
  `BundleEntry` variants; no producer failure aborts the bundle.
- Deterministic producer order.

UAT:
- producer with Ok outcome → Evidence entry
- producer with Err (e.g. exit code) → ProducerFailed entry
- producer that cannot be reached (e.g. tool missing) → ProducerMissing
- producer that times out or returns garbled output → ProducerUnknown
- aggregation across N producers produces N entries (or fewer if
  some are deterministically skipped)
- ordering stable

### WU3 — PolicyGate adversarial + invariants

- `PolicyDecision` enum: `Pass | Warn | Block | InsufficientEvidence`.
- Gate has a `PolicySpec` declaring required-evidence slots.
- Per rule: required Evidence must be `Evidence`, not
  `ProducerFailed/Missing/Unknown`.
- Decision reasons are structured (per-entry, per-rule).

Adversarial matrix:
- All Evidence present and grade≥threshold → Pass
- Some Evidence present, some Missing for required slot → InsufficientEvidence (NEVER Pass)
- FAILED on required slot → Block
- UNKNOWN on required slot → InsufficientEvidence (NEVER Pass)
- Non-required slot MISSING/FAILED → may degrade Pass to Warn, never to Pass-from-Missing
- Empty bundle → InsufficientEvidence
- Determinism: same bundle + same spec → same decision, same reasons order

## Architecture invariants preserved

- Facts remain canonical (bundle is derived, not a Fact).
- Evidence carries proof (no Evidence producer mints authority).
- Derived operational decisions are not Facts (the bundle is derived).
- Producers propose; the gate decides.
- Unknown/incomplete never becomes "safe" (gate invariant).
