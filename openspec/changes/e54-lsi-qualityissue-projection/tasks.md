# Tasks — cycle e54 — QualityIssue compatibility projection (M6.3)

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15
> Umbrella task: **7.6 Implement QualityIssue compatibility projection.**

## Work units

### WU-1 — Severity/status/kind mapping helpers
- `severity_from_str`, `risk_for`, `status_from_str`, `sanitize_segment`.

### WU-2 — Projection
- `project_quality_issue(&QualityIssue) -> Result<Finding, FindingError>`:
  identity `quality-<id>`, kind `quality.<category>`, detector
  `legacy.quality@legacy`, `evidence_class = C`, empty evidence,
  `location` causal step, message normalized when empty.

### WU-3 — Error plumbing
- `FindingError::InvalidIdentifier(DetectorIrError)` +
  `From<DetectorIrError> for FindingError`.

### WU-4 — Tests
- identity/kind, sanitization, empty-category fallback, severity→display+risk,
  unknown severity, status variants, location step, non-blocking policy,
  empty-message normalization, round-trip.

## Acceptance gate
- `cargo test -p cognicode-core --lib domain::findings` green (36 tests).
- `cargo check -p cognicode-core` green; fmt clean; domain pure.
- Conventional commit, no AI trailers.
