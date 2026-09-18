# Proposal: `arch/debt4-rollback-journal` (CLOSED GREEN)

> DEBT-4 — rollback journal / uninstall retention policy.
> Final cycle of the identity-taxonomy debt chain
> (DEBT-3 -> DEBT-3.f -> DEBT-2 -> DEBT-1 -> DEBT-4).

## Why

`cmd_rollback` resolved its target with a heuristic: with no tracker
pin, it picked the highest-semver journal on disk. "Unknown" became
"guessed", and the journal was treated as navigable history when its
sole responsibility is undoing ONE committed lifecycle transition.

## Decision (implementation at ebe2157d..1c9c6c24)

Journal = one-shot operational rollback capability:

- Applicability is explicit: journal applies only when its version
  equals the tracker pin. No tracker -> "nothing applicable" (no
  heuristic selection, acceptance = 0 sites).
- Stale journal (envelope version != tracker) fails closed.
- Consumption: journal removed only AFTER a successful reversal;
  second rollback is a harmless no-op.
- Uninstall: invalidates the journal of the removed version only; if
  the version was active, the tracker pin is cleared (post-state is
  explicitly "no current version"; no automatic restore).
- Uninstall is idempotent (version-tree existence is the source of
  truth for "installed").
- Drop rule pinned: a deserialized RollbackJournal MUST NOT retain
  armed Drop rollback behaviour; neutralized at the source
  (`lifecycle_journal::load`, `RollbackJournal::from_json`),
  tripwired by `t_debt4_loaded_journal_is_drop_neutralized`.

## Spec

Delta added to `openspec/specs/cognicode-lifecycle/spec.md`
(Requirement: "Rollback journal is a one-shot capability (DEBT-4)").
Architecture record: `docs/adr/ADR-DEBT-4-rollback-journal-one-shot.md`
(local-only, ephemeral per project convention).

## Verification

- Transition matrix T1-T6 (9 t_debt4 tests incl. Drop tripwire): GREEN
- UAT disposable-home round-trips (install/rollback, install/uninstall):
  GREEN, zero real-HOME pollution (before/after file snapshot diff)
- Full cogh suite: 269 passed / 0 failed / 1 ignored
- `check_known_failures.py`: 41 unchanged; fmt scoped clean;
  clippy: 0 errors; `cargo check --tests` clean
- Heuristic target selection scan: 0 runtime matches

## Out of scope (explicit)

Audit/history (future `LifecycleReceipt`), multi-level undo, GC of
stale journals, TOML zero-touch writes (optional DEBT-1b).
