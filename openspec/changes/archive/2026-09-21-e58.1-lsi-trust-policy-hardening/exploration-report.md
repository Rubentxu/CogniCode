# Exploration Report — cycle e58.1 — trust/policy hardening

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

Review of e57 found three boundary gaps to close before copying the
executor pattern into Graph/Dataflow.

## Gaps and resolutions

### G-1 (P0) — `AdmittedDetector` could still be forged

`DetectorAdmission::admit` normalised the authority to `Candidate`, but
`AdmittedDetector` remained a public, `Serialize`/`Deserialize` struct, and
`PromotionApproval` was a public string wrapper. Any Rust consumer (or a
deserialized value) could therefore present `authority: Gated` directly.

**Resolution — a real capability boundary:**
- [`AdmittedDetector`] is plain data: no serde, not executable.
- [`ExecutionPermit`] carries private fields plus a private `AdmissionSeal`
  and is **not** serializable, so it can only be minted by
  `DetectorAdmission::{admit, promote, restore}`.
- The executor takes `&ExecutionPermit`, never a bare `AdmittedDetector`.
- Persistence goes through `AdmittedDetectorRecord` (serializable); recovery
  must re-enter via `restore`, which re-validates the definition and
  downgrades an unapproved `Gated` claim to `Candidate`.

### G-2 (P0/P1) — the backend could still influence the gate

Although a backend could not build a `Finding`, it supplied
`DetectorMatch.severity/risk` and `ProducedEvidence.kind`, and the assembler
copied the former and derived `EvidenceClass` from the latter. A faulty AST
backend could emit `RuntimeTrace` + `Critical/Critical` and produce an
A/Critical finding.

**Resolution:**
- `DetectorMatch` no longer carries severity/risk; both come from the
  detector's new `DetectorFindingPolicy` (definition-owned, part of the
  instance digest).
- `DetectorBackend::evidence_ceiling()` declares the strongest class a
  backend may claim; the executor rejects a run whose evidence exceeds the
  ceiling (`BackendContractViolation::EvidenceCeilingExceeded`).

### G-3 — `AstBackend` over-advertised its capabilities

It advertised `SemanticResolution` while only comparing already-classified
subjects. Now it advertises `AstPattern` only, so a detector requiring
semantic resolution cannot be planned onto it.

Also added IR validation rule V9: a detector with executable steps must
declare at least one capability (an empty `requires` is legal only for a
PRODUCE-only aggregator), so the planner cannot hand it to any backend.

## Explicitly NOT closed

**U42 (full causal grounding) stays open.** The verifier proves *referential
existence* (evidence resolves; causal evidence belongs to the finding), not
the full chain `CausalStep → EvidenceId → Evidence → FactId → Fact →
Entity/Snapshot/Provenance`. That needs the kernel evidence adapter; the
`EvidenceLookup` port is the seam. `state.yaml` continues to mark U42
incomplete.

## e58 checker bug

`check_known_failures.py` ignored the `cargo test` return code, so a
compile failure yielded `observed = {}` and `--update` could wipe the
baseline. Fixed: harness failures exit 2 and refuse `--update`; `--update`
preserves each existing entry's `category`/`first_seen`/`reason`.
