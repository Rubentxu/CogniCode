# issues-confidence-rules Specification (NEW)

## Purpose

`IssuesConfidenceRules` is the pure-function scoring module that maps a single `IssueExtractor` parse signal to a `f64` confidence in `[0.0, 1.0]` and a `Provenance` tag. It mirrors `DocsConfidenceRules` (`crates/cognicode-core/src/infrastructure/extraction/docs_confidence_rules.rs`) exactly: a `ConfidenceTier` enum with `confidence()` and `provenance()` accessors, gated behind `#[cfg(feature = "multimodal")]`. The 4-tier table is **frozen** — values are stable identifiers, not parameters.

## Requirements

### Requirement: Four confidence tiers

The module MUST export a `ConfidenceTier` enum with exactly 4 variants, each bound to a fixed confidence and provenance by the table below. Both `confidence()` and `provenance()` MUST be `const fn`.

| Rule | Trigger | Confidence | Provenance |
|------|---------|-----------:|-----------|
| `ExplicitLink` | Issue body contains a `Fixes #N` / `Closes #N` / `Resolves #N` style keyword, OR the issue explicitly cross-references a commit SHA | 0.9 | `Extracted` |
| `CommitFixes` | A commit subject or body matches `Fixes #N` / `Closes #N` / `Resolves #N` | 0.85 | `Extracted` |
| `CommitRefs` | A commit subject or body matches `Refs #N` / `Part of #N` / `See #N` | 0.7 | `Inferred` |
| `BodyMention` | Issue body mentions a code symbol via `file:name:line` shape (or markdown link to one) | 0.5 | `Inferred` |

#### Scenario: tier values are locked
- GIVEN the 4 variants
- WHEN `ConfidenceTier::ExplicitLink.confidence()` is called
- THEN it returns `0.9` exactly
- AND `ConfidenceTier::CommitFixes.confidence()` returns `0.85`
- AND `ConfidenceTier::CommitRefs.confidence()` returns `0.7`
- AND `ConfidenceTier::BodyMention.confidence()` returns `0.5`

#### Scenario: provenance matches the table
- GIVEN each variant
- WHEN `provenance()` is called
- THEN `ExplicitLink → Extracted`, `CommitFixes → Extracted`, `CommitRefs → Inferred`, `BodyMention → Inferred`

### Requirement: Pure scoring functions

The module MUST expose one pure function per signal. Each takes a parsed signal and returns the matching `ConfidenceTier` (the extractor materialises the `GraphEdge` with the tier's `confidence()` + `provenance()`).

| Function | Signature | Returns |
|----------|-----------|---------|
| `score_explicit_link(text: &str) -> ConfidenceTier` | Body / commit contains a "fixes" keyword | `ExplicitLink` if matched, else `Unresolved` |
| `score_commit_fixes(commit_msg: &str) -> ConfidenceTier` | Subject + body scanned for `Fixes/Closes/Resolves #N` | `CommitFixes` if matched, else `Unresolved` |
| `score_commit_refs(commit_msg: &str) -> ConfidenceTier` | Subject + body scanned for `Refs/Part of/See #N` | `CommitRefs` if matched, else `Unresolved` |
| `score_body_mention(line: &str) -> ConfidenceTier` | A line matches `file:name:line` or `[text](file:name:line)` | `BodyMention` if matched, else `Unresolved` |
| `Unresolved` | None of the above | 0.3 / `Ambiguous` (defensive fallback only) |

#### Scenario: CommitFixes wins over CommitRefs in the same message
- GIVEN a commit message `Fixes #10, Refs #11`
- WHEN `score_commit_fixes` is called on the same string
- THEN it returns `CommitFixes` (the higher-confidence tier)
- AND `score_commit_refs` returns `CommitRefs` for the secondary reference

#### Scenario: BodyMention is independent of the issue-tracker match
- GIVEN a body line `see [bar](src/foo.rs:bar:1)`
- WHEN `score_body_mention` is called
- THEN it returns `BodyMention` (0.5, `Inferred`)

### Requirement: Idempotency of pure functions

Every function MUST be a pure `fn` (no `&self`, no I/O, no global state). Repeated calls with the same input MUST return the same `ConfidenceTier`. The functions MUST be `#[inline]` to keep the call overhead at the floor.

#### Scenario: Repeated calls are stable
- GIVEN `score_commit_fixes("Fixes #1")` called 1000 times
- WHEN the results are collected
- THEN all 1000 results are `CommitFixes`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Commit message is empty string | `score_commit_fixes` / `score_commit_refs` return `Unresolved` (0.3) |
| Body line is whitespace only | `score_body_mention` returns `Unresolved` |
| Commit message has `fixes` (lowercase) | Case-insensitive match — still `CommitFixes` |
| Commit message references `#0` | `Unresolved` (issue `0` is invalid in GitHub) |
| Body line is a URL `https://...` | `score_body_mention` returns `Unresolved` (URL filter, same as `docs_extractor`) |
| `score_explicit_link` matches both `Fixes` and `Refs` | `ExplicitLink` wins (the more specific signal) |

## TDD RED Gate

1. 4 tier value tests (one per variant) — exact `f64` equality
2. 4 provenance tests (one per variant)
3. 5 scoring function tests — happy path for each
4. Case-insensitivity test for `score_commit_fixes`
5. Unresolved fallback test for each of the 5 functions
6. Idempotency test (1000 repeated calls)
7. BodyMention URL filter test
8. Commit-message `Fixes #0` → `Unresolved` test
9. Compile-gate test: module is absent under `--no-default-features`

## Dependencies

- `docs-source-adapter::docs_confidence_rules` (the structural template; copy its layout exactly)
- `multimodal` Cargo feature (gates the whole module)
- `Provenance` value object (existing, 3 variants: `Extracted` / `Inferred` / `Ambiguous`)
- `NodeKind` / `EdgeKind` enums (no new variants needed)

## Out of Scope

- Bayesian confidence calibration over time
- Per-repo confidence tuning (the 4 tiers are workspace-wide)
- Confidence fusion across multiple signals for the same edge (V1: one tier per edge; the extractor picks the highest-tier signal that matched)
