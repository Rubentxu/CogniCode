# Exploration Report — cycle e54 — QualityIssue compatibility projection (M6.3)

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

M6 umbrella deliverables include "legacy QualityIssue projection" and the
exit gate "QualityStore compatibility". With the Detector IR (e52) and the
Finding model (e53) in place, the next bounded slice lifts the existing
legacy quality rows into the new model so the quality surface does not fork
onto a second finding type.

## Landscape

- `crates/cognicode-core/src/domain/ports/quality_store.rs` — `QualityIssue
  { id: i64, rule_id, severity, category, file_path, line, message, status }`,
  ungated, in core. Adapters (`LadybugStore`, explorer quality repository)
  already populate it.
- `domain::findings` (e52/e53) — `Finding`, `EvidenceClass`, `FindingGate`.

## Strategy

Add `domain/findings/quality_projection.rs` with a pure
`project_quality_issue(&QualityIssue) -> Result<Finding, FindingError>`.

Conservative, documented policy (safe compatibility direction):

- severity string → `FindingSeverity`; risk derived from severity;
- category → sanitized `quality.<segment>` kind;
- status string → `FindingStatus` (unknown ⇒ `Open`);
- `evidence_class = C`, `evidence` empty, causal chain = single `location`
  step → projected findings are **not explainable** and **cannot block**
  the new gates until re-evidenced.

## Impact model

| Artifact | Impact | Confidence |
|----------|--------|-----------|
| `domain/findings/quality_projection.rs` (new) | additive pure domain | KNOWN |
| `finding.rs` | +1 `FindingError` variant + `From<DetectorIrError>` | KNOWN |
| consumers | none yet (wiring is a later cycle) | LIKELY |

## Out of scope

- Wiring the projection into Explorer/MCP quality responses.
- A re-evidence pass that would upgrade projected findings.
