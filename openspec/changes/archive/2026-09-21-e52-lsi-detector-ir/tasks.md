# Tasks — cycle e52 — Detector IR (M6.1)

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15
> Change ID: `e52-lsi-detector-ir`
> Umbrella task: **7.2 Define Detector IR schema/parser/validator.**

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~450–600 (implementation + tests) |
| 400-line budget risk | Medium |
| Chained PRs recommended | No (single coherent domain module) |

## Work units

### WU-1 — Register the `findings` domain module

- Add `pub mod findings;` to `crates/cognicode-core/src/domain/mod.rs`.
- Create `crates/cognicode-core/src/domain/findings/mod.rs` re-exporting
  the Detector IR surface.

### WU-2 — Escalation ladder

- `AnalysisLevel` with 8 variants, `rank()`, `Display`, `Ord`.
- Tests: strict ascending rank; stable Display strings.

### WU-3 — Namespaced patterns

- `SubjectPattern` and `FindingKind` newtypes with a shared
  `ns:name` grammar validator (`parse_namespaced`), rejecting empty
  namespace, empty name and multiple `:`.
- Tests: accept `security.user_input`; reject `""`, `"ns:"`, `":name"`,
  `"a:b:c"`.

### WU-4 — IR nodes + `DetectorIr`

- `DetectorStep::{Match, Flow, Exclude, Verify, Produce}`.
- `DetectorId`, `DetectorIr { id, name, required_level, authority, steps }`.
- `DetectorAuthority::{Candidate, Gated}` + `can_block()`.
- Serde derives with stable names.

### WU-5 — Validator

- `DetectorIr::validate()` implementing rules V1–V8 from the design,
  returning `DetectorIrError` with structured payloads.
- Tests for every rule, including the "unsupported construct fails loud"
  scenario (FLOW/VERIFY below the required level) and the
  "candidate cannot block" authority scenario.

## Sequencing

WU-1 → WU-2 → WU-3 → WU-4 → WU-5 in one commit.

## Acceptance gate

- `cargo test -p cognicode-core --lib domain::findings` green.
- `cargo check -p cognicode-core` green (default build).
- Domain module imports no `sqlx`/`tokio`/I/O crate.
- `cargo fmt --all --check` green.
- Conventional commit, no AI trailers.
