# e86.4 — cogh Rollback-to-Version (proposal)

> Program: cognicode-distribution (umbrella: e84 contract + e85 release + e86 lifecycle)
> Milestone: M2.4 — selective rollback by target version, not only last transaction
> Phase: propose | Date: 2026-09-18
> Delivery: bounded cycle, single slice

## Intent

Today `cogh rollback` reverses the LAST install transaction. If a user did:

  1. `cogh install mcp-server 0.95.0 --ide opencode --profile core`
  2. `cogh update mcp-server --ide opencode`  →  installed 0.95.1

...and now wants to return to 0.95.0, the workflow is:

  - `cogh rollback`  (rolls back 0.95.1 → pre-update state, i.e. 0.95.0)
  - if they also want to undo 0.95.0, run `cogh rollback` AGAIN

That's a fragile ux because the user has to know "rollback" means
"undo last", not "go to N". A real selection parameter
`cogh rollback --to <version>` lets the user say "I want mcp-server 0.95.0
to be the current install" in one command, with the system figuring out
which transactions to play forward or reverse.

This is a UX feature + a journal-design upgrade (selective journal
walking, not just last entry).

## Scope

IN scope:
- New CLI surface: `cogh rollback --to <version>` (e86.4 REQ-RB-01..05).
- Journal upgrade: every committed transaction is keyed by
  `(transaction_id, plugin, version)`; rollback walks transactions in
  reverse-chronological order UNTIL the target version is reached for the
  target plugin. Forward-only transactions skipped.
- Idempotency: if the target version IS the current installed version,
  rollback is a no-op (clean exit, info message).
- Block: rollback past the FIRST installation (i.e. you cannot rollback
  to "no version") is refused with a clear error and hint to use
  `cogh uninstall` instead.
- Tests: 5 new unit tests (mirror E86.3's structure) + 1 subprocess e2e
  mirroring `cmd_rollback_after_live_install`.

OUT of scope:
- Adding `cogh rollback --to <date>` (timestamp-based) — defer to a
  later cycle, the journal schema is the same but UX needs design.
- Replacing `cogh rollback` (no-arg) — it stays as a shortcut for
  "undo last transaction".
- Rollback across plugins (e.g. "rollback everything") — defer.
- Clean uninstall -> then install new = a separate semantic that
  belongs to E86.3's uninstall coverage.

## Approach

Read first:
  - `crates/cognicode-cli/src/cmd/rollback_journal.rs` — current
    single-journal-on-disk model. The journal is already JSON-
    serialisable (e86 set this up in T2).
  - `crates/cognicode-cli/src/cmd/installer_transaction.rs::commit`
    where the per-transaction id is generated.
  - `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs` — the
    resolution contract for `--to`.

Journal schema upgrade (additive, gated by serde default+fallback):
  - New field `transaction_id: Uuid` per journal entry (regenerated
    on commit; default if missing for compatibility).
  - New field `committed_at: UnixSeconds` (already there? verify).
  - `~/.cognicode/journal/` becomes a directory of per-transaction
    JSON files (one file per transaction) instead of a single journal
    file. Old single-file journal migrates lazily on first commit.

CLI surface (additive):
  - `cogh rollback --to <version> --plugin <plugin>` — REQUIRED new
    arg `--to` when --to is passed.
  - Without `--to`, current behaviour is preserved.
  - `<version>` accepts semver, including pinned or `latest`.

Spec delta: REQ-LJ-16..22 added to the lifecycle spec, plus a new spec
file `rollback-target-version/spec.md` with Given/When/Then.

## Acceptance contract

| REQ | Observable |
|---|---|
| REQ-RB-01 | `cogh rollback --to 0.95.0 --plugin mcp-server` after install v0.95.0 + update to v0.95.1 + another-update-to-v0.95.2: rolls back v0.95.2→v0.95.1 (transaction reverse), then no-op for v0.95.1 (already there) → manifest reports 0.95.1 + observed equals target? No, target was 0.95.0 so reverse continues. Final state: mcp-server@0.95.0 |
| REQ-RB-02 | `cogh rollback --to 0.95.2 --plugin mcp-server` after the same sequence: re-installs 0.95.2 from cache; no new transaction needed if artifacts cached; manifest reports 0.95.2 |
| REQ-RB-03 | `cogh rollback --to 0.95.0 --plugin mcp-server` when 0.95.0 is the FIRST installation: refuses with "cannot rollback past first installation; use `cogh uninstall mcp-server 0.95.0` instead" |
| REQ-RB-04 | `cogh rollback --to 0.95.0 --plugin mcp-server` when target = current: exits 0 with "already at 0.95.0" message; no side effects |
| REQ-RB-05 | `cogh rollback --to nonexistent --plugin mcp-server`: refuses with `InstallerError::ResolveFailed("version nonexistent not in known history; known: [0.95.0, 0.95.1, 0.95.2]")` |
| REQ-RB-06 | All 184 (E86.1 baseline) + 6 (E86.3 expected) = 190 expected cogh tests still pass + 5 (REQ-RB-01..05) + 1 e2e = 196 expected |
| REQ-RB-07 | Spec delta REQ-LJ-16..22 added; new spec file `rollback-target-version/spec.md` consumed by tests |
| REQ-RB-08 | Workspace lint, fmt, known-failures checker stay green |

## Risks

- **Journal migration** — must not break existing installs. Migration
  is on first-commit (lazy): old single-file journal is read once,
  per-transaction files written on first new commit. A flag
  `journal_mode = "v2"` in `~/.cognicode/state.json` distinguishes
  the two; if a user upgrades and then downgrades the binary,
  downgrade path is unsupported (similar to most package managers).
- **Disk I/O** — per-transaction files are small (a few KB each) but a
  long-lived install (months of updates) accumulates them. Mitigation:
  journal keeps only the most recent 20 transactions by default, older
  ones archived into `~/.cognicode/journal/archive/<transaction_id>.json.gz`
  (compression optional, gated by a future cycle).
- **Concurrent rollback** — two `cogh rollback` invocations in
  parallel could race on the journal. Mitigation: file lock on the
  journal dir; if held >5s, second invocation fails with
  `InstallerError::LockTimeout("another cogh is modifying the journal")`.
- **Network dependency** — rollback-to-version may need to RE-DOWNLOAD
  artifacts if the cache has been GC'd. Mitigation: `cogh rollback --to`
  refuses offline re-download (clear error); user can either be online
  or accept that artifacts are purged (matches existing `--offline`
  conventions in `--channel stable` resolution).

## Deliverables

1. `crates/cognicode-cli/src/cmd/rollback_journal.rs` — per-transaction
   file model + lazy migration.
2. `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs` — `--to`
   resolution against known journal history.
3. `crates/cognicode-cli/src/bin/cogh.rs` — `Rollback { ..., to:
   Option<String>, plugin: Option<String> }` struct.
4. 5 new unit tests + 1 subprocess e2e test in
   `crates/cognicode-cli/src/cmd/lifecycle.rs` (parallel to E86.3's).
5. Spec delta REQ-LJ-16..22 + new `rollback-target-version/spec.md`.
6. Apply receipt + verification report.

## Non-goals

- A unified `cogh history` command (would expose journal reads to
  humans — desirable but not in scope).
- A TUI / web UI for browsing versions (a long stretch).
- Refactoring `cogh rollback` (no-arg) to be implemented in terms of
  `cogh rollback --to <previous>` internally (defer; risk is changing
  semantics for the existing shortcut).

## Sequencer

E86.2 → E86.3 → E86.4, in that order. E86.3's regression suite is the
safety net for E86.4's journal redesign — if E86.4 breaks uninstall
state, E86.3 catches it on the next CI run.

## Open questions (defer until explore phase)

1. Should `cogh rollback --to <version>` be allowed across plugins
   (rollback mcp-server and core simultaneously to v0.95.0), or
   constrained to one plugin at a time? **Lean**: one plugin, semver
   must match both; keep simple, design later.
2. Should the per-transaction journal entries include a digest of the
   installed artifacts (for tamper detection on a shared PC)? **Lean**:
   yes, add `installed_digests: BTreeMap<PathBuf, Sha256>` per entry —
   closure-time cost is one sha256 per file added to the journal; you
   pay on rollback only if you trust the journal (otherwise verify
   re-walk).
3. Should `cogh rollback` (no-arg) preserve "rollback last transaction"
   or become a synonym for "rollback to the version BEFORE the current
   one"? **Lean**: keep the no-arg semantics; `--to` is the explicit
   selector.
