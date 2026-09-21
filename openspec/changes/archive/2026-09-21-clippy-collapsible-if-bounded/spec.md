# Specification: clippy-collapsible-if-bounded

## Capabilities (REQ)

### REQ-COLLAPSIBLE-IF-01

Each of the 18 `collapsible_if` warnings identified by clippy on
HEAD must be resolved by applying the suggested collapse into a single
`if` statement using `let_chains` syntax.

**Acceptance**:
- `cargo clippy -p cognicode-cli --bin cognicode-release --tests`
  reports 0 occurrences of `clippy::collapsible_if`.
- The same `cargo clippy` command reports no NEW warnings in any
  other category introduced by the change.

### REQ-COLLAPSIBLE-IF-02

The change MUST be behavior-preserving (no API surface changes,
no test changes).

**Acceptance**:
- No public function signature changes in
  `crates/cognicode-cli/src/cmd/*.rs`.
- No new `#[allow(clippy::*)]` attributes introduced.
- All pre-existing tests that pass on the parent commit MUST continue
  to pass on the new commit, EXCEPT the acknowledged pre-existing
  failure `layout::tests::f3_t4_broken_same_version_install_is_
  repaired_not_hidden` which is independent of this change.

### REQ-COLLAPSIBLE-IF-03

The change MUST be documented in a verify-report that confirms each
of the 18 collapses is mapped to its original location.

**Acceptance**:
- Verify report exists at
  `openspec/changes/clippy-collapsible-if-bounded/verify-report.md`.
- The report lists, for each warning, the file, line range, original
  code, and collapsed code.

## Invariants

- **let_chains stability**: this project compiles on rustc 1.96 with
  edition 2024 where `let_chains` is stable. No `#![feature(...)]`
  attribute is added.
- **Conventional Commits**: the change is committed with the message
  `chore(lint): collapse collapsible_if (H5 phase 1)` per the project's
  AGENTS.md directive.
- **No AI attribution**: the commit message MUST NOT contain
  `Co-Authored-By: AI` or any equivalent.

## Domain language

- **collapsible_if**: a clippy lint that identifies nested
  if-statements which can be combined into a single boolean
  expression via `let_chains`.
- **let_chains**: a Rust language feature allowing `&&` to chain
  `if let` expressions in a single boolean condition. Stable in
  edition 2024 + rustc 1.88+.
- **bounded cycle**: a scoped SDDK cycle that addresses one specific
  category of work, with explicit acceptance criteria, reviewer
  approval, and verifiable closure.

## Out of scope

- Other clippy lints (dead_code, needless_borrow, doc_overindented,
  etc.) — addressed in future cycles.
- Carry-over test failures (`f3_t4_*`, `test_clean_home_install`).
- API surface changes.
- Release engineering (the change will be released in the next
  release slot, v0.97.2 if applicable, but release tagging is NOT
  part of this cycle).

## Coherence check (propose→spec)

This specification maps to the proposal's three bullets of scope:

| Proposal bullet | Specification REQ |
|---|---|
| Apply clippy's suggested fix to 18 warnings | REQ-COLLAPSIBLE-IF-01 |
| Cargo verification (fmt, check, clippy) | REQ-COLLAPSIBLE-IF-01 + REQ-COLLAPSIBLE-IF-02 |
| One bounded commit + one verify report | REQ-COLLAPSIBLE-IF-03 |

No scope creep. Invariants are within the proposal's stated approach.
