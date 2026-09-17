# Spec: lifecycle-journal

> e86 WU1. Operational authority is `state.yaml`.

## Purpose

`cogh rollback` must reverse the last install deterministically. Today
`RollbackJournal` records side-effects in memory and reverses them in LIFO
order, but the journal is gone once the transaction commits; there is no
way for a later command to read it. e86 persists the journal to disk at
commit time and replays its reversal on `cogh rollback`.

## Scope

In scope:

- Persisting the journal to `~/.cognicode/journal/<version>.json` at
  commit time.
- A new `WroteTracker` side-effect so the rollback can restore the
  previous tracker version.
- A new `cogh rollback [plugin]` command that reads the latest journal
  and reverses it.
- Behaviour when no journal exists: `cogh rollback` exits cleanly with a
  "nothing to roll back" message and exit code 0.

Out of scope:

- A journal present for an install that was not produced by `cogh`. If the
  JSON does not parse, the command fails loudly.
- Restoring the install's `manifest.yaml` to its previous bytes (we keep
  the WroteManifest reversal that already exists).
- Time-bounded rollback (e.g. "rollback to 24 hours ago"). Rollback
  reverses exactly the latest commit; older versions are not touched.

## Requirements

### REQ-LJ-01 — journal on commit

**When** `InstallerTransaction::commit` writes the install manifest,
**then** the journal MUST also be written to
`~/.cognicode/journal/<version>.json`. The journal MUST contain every
`SideEffect` in commit order (not LIFO — the file is forward-ordered;
`rollback` reverses it).

### REQ-LJ-02 — journal shape

The journal JSON MUST be:

```json
{
  "version": "0.95.0",
  "committed_at_unix": 1734567890,
  "effects": [
    { "type": "CreatedDir", "path": "..." },
    { "type": "Downloaded", "path": "..." },
    { "type": "WroteManifest", "path": "..." },
    { "type": "WroteTracker", "path": "...", "previous": "0.94.0" }
  ]
}
```

The `WroteTracker` variant is new in e86. Its reverse writes `previous`
back to `path`. If `previous` is `null`, the tracker file is removed.

### REQ-LJ-03 — `cogh rollback` semantics

**When** `cogh rollback` runs and
`~/.cognicode/journal/<tracker_version>.json` exists,
**then** it reads the journal, calls `RollbackJournal::rollback()` on its
contents, removes the install dir at `~/.cognicode/install/<version>/`,
and writes the previous tracker version back.
**When** no journal exists,
**then** the command prints "nothing to roll back" and exits 0.
**When** the journal exists but `rollback()` returns an error,
**then** the command prints the error chain and exits non-zero; the
install dir is NOT removed (best-effort safety: leave the broken state
visible so the user can intervene).

### REQ-LJ-04 — atomicity on rollback

`cogh rollback` MUST acquire the same advisory install lock as
`run_install`. A rollback in progress prevents concurrent installs and
vice versa.

### REQ-LJ-05 — idempotence

Re-running `cogh rollback` after a successful rollback MUST be a no-op:
the journal for the rolled-back version is gone, so REQ-LJ-03's
"nothing to roll back" branch fires.

## Scenarios

| ID | When | Then |
|---|---|---|
| SC-LJ-01 | After a successful install, `~/.cognicode/journal/<v>.json` exists with the install's effects | File is readable, JSON-valid, and contains `WroteTracker` with the previous tracker value |
| SC-LJ-02 | `cogh rollback` with no journal | Prints "nothing to roll back", exit 0 |
| SC-LJ-03 | `cogh rollback` after a successful install | Install dir removed, tracker restored to previous version, shims removed |
| SC-LJ-04 | `cogh rollback` twice in a row | Second call is a no-op (no journal) |
| SC-LJ-05 | A concurrent `cogh install` is running | `cogh rollback` blocks until the lock is released, then proceeds |
| SC-LJ-06 | The journal's JSON is corrupt | `cogh rollback` prints a clear error and exits non-zero; install dir untouched |

## Exit gates

This spec is satisfied when:

- `cargo test -p cognicode-cli --bin cogh lifecycle::tests::test_rollback_*`
  passes all of SC-LJ-01..06.
- The existing `RollbackJournal` tests still pass (no regression).
- `cargo check --workspace --all-targets` exits 0.
- `cargo fmt -p cognicode-cli --check` exits 0.