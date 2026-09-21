# Spec: cogh rollback --to <version> (e86.4)

> Cycle: `p-c1fac1fea05615c6/e86-4-cogh-rollback-select-version`
> Operational authority: `state.yaml`

## Purpose

The legacy `cogh rollback` (no-arg) reverses the LAST install
transaction by replaying the active journal. Users with multi-step
history (`install v0.95.0` → `update to v0.95.1` → `update to v0.95.2`)
who want to return to `v0.95.0` must currently chain two rollbacks
manually and reason about the journal sequence. This cycle adds
`cogh rollback --to <version>` so the user can express the intent
"return to v" in one command.

This is a UX feature with a one-step slice of the journal upgrade
proposed in the umbrella. Multi-step rollback chains (target that is
not the immediately-previous version) are explicitly out of scope
for this slice and are refused with a clear error that points the
user at `cogh install` or `cogh uninstall` instead.

## Requirements

### REQ-RB-01 — `cogh rollback --to <previous>` succeeds (one-step slice)

**When** the tracker pins version `v_current` and the journal for
`v_current` declares `previous_tracker = v_target`,
**then** `cogh rollback --to v_target` MUST roll back the current
install to `v_target` in a single command, restoring the tracker
pin to `v_target` and consuming the journal.
**And** the resulting state MUST be identical to running the legacy
`cogh rollback` twice (once to undo `v_current`, once to no-op on
`v_target`'s absence) — except the second no-op never runs because
the first rollback already reached the target.

#### Scenario: one-step rollback to previous_tracker

- GIVEN `cogh install mcp-server 0.95.0 --ide opencode --profile core` was run
- AND `cogh update mcp-server --ide opencode` upgraded to `0.95.1`
- AND `cogh update mcp-server --ide opencode` upgraded to `0.95.2`
- AND the journal for `0.95.2` carries `previous_tracker = "0.95.1"`
- WHEN `cogh rollback --to 0.95.1` runs
- THEN `mcp-server@0.95.2` is uninstalled
- AND `mcp-server@0.95.1` becomes the active install
- AND the tracker pins `0.95.1`

### REQ-RB-02 — multi-step target is refused with a clear error

**When** the user runs `cogh rollback --to v_target` and
`v_target` is not reachable in one step (i.e. the journal's
`previous_tracker` is some `v_prev` that does not equal `v_target`),
**then** the command MUST exit non-zero with an error message that
- names the actual `previous_tracker` so the user knows where the
  chain ends,
- explains that multi-step rollback is not implemented in this slice,
- suggests `cogh install <plugin> --version <v_target>` or
  `cogh uninstall mcp-server <v_current>` as alternatives.
**And** no filesystem mutation MUST occur (tracker unchanged, journal
preserved).

#### Scenario: multi-step target refused

- GIVEN tracker pins `0.95.0`, journal for `0.95.0` has
  `previous_tracker = "0.94.0"`
- WHEN `cogh rollback --to 0.92.0` runs
- THEN the command exits non-zero
- AND stderr names `0.94.0` (the actual previous_tracker)
- AND stderr contains "not reachable in one step" or "Multi-step"
- AND tracker still pins `0.95.0`
- AND the journal is preserved

### REQ-RB-03 — rollback past the first installation is refused

**When** the tracker pins version `v_current` and the journal for
`v_current` declares `previous_tracker = None` (i.e. `v_current` was
the user's first installation),
**then** `cogh rollback --to <anything>` MUST exit non-zero with a
message that contains `"past first installation"` and hints at
`cogh uninstall mcp-server <v_current>` as the right tool.

#### Scenario: rollback past first install refused

- GIVEN tracker pins `0.95.0`, journal for `0.95.0` has
  `previous_tracker = null`
- WHEN `cogh rollback --to 0.92.0` runs
- THEN the command exits non-zero
- AND stderr contains "past first installation"
- AND stderr mentions `uninstall`
- AND tracker still pins `0.95.0`

### REQ-RB-04 — `cogh rollback --to <current>` is a clean no-op

**When** the user runs `cogh rollback --to v_current` (the same
version the tracker currently pins),
**then** the command MUST exit 0 with a message indicating "already
at v_current; nothing to do".
**And** the journal MUST NOT be consumed.
**And** no filesystem mutation MUST occur (tracker unchanged, install
tree unchanged).

#### Scenario: rollback to current is no-op

- GIVEN tracker pins `0.95.0`, journal for `0.95.0` exists
- WHEN `cogh rollback --to 0.95.0` runs
- THEN the command exits 0
- AND stdout contains "already at 0.95.0" or "nothing to do"
- AND the journal is still on disk
- AND tracker still pins `0.95.0`

### REQ-RB-05 — `cogh rollback --to <unknown>` with no journal is refused

**When** the user runs `cogh rollback --to v_target` and the active
tracker pin has no journal file (e.g. install was GC'd or the journal
was removed manually),
**then** the command MUST exit non-zero with a message that
- explains "no journal for active version" (or equivalent),
- names the requested target so the user can confirm intent,
- suggests `cogh install` or `cogh uninstall` as alternatives.
**And** no filesystem mutation MUST occur.

#### Scenario: unknown target with no journal refused

- GIVEN tracker pins `0.95.0`
- AND no journal exists for `0.95.0`
- WHEN `cogh rollback --to 0.92.0` runs
- THEN the command exits non-zero
- AND stderr contains "no journal for active version" or "cannot reach"
- AND stderr mentions the target `0.92.0`

## Non-goals (out of scope for this slice)

- `cogh rollback --to <date>` (timestamp-based): requires a separate UX
  cycle; the journal schema is the same but the resolution contract
  differs.
- Replacing `cogh rollback` (no-arg): it stays as the shortcut for
  "undo last transaction".
- Multi-step chained rollback across multiple journals: requires the
  per-transaction-file journal upgrade proposed in the e86.4 proposal
  §Approach (Journal schema upgrade). Refused with a clear message in
  this slice.
- Rollback across plugins (e.g. "rollback everything"): separate UX.
- Clean uninstall + install = a separate semantic that lives in e86.3's
  uninstall coverage.

## Cross-references

- CLI surface: `crates/cognicode-cli/src/bin/cogh.rs` `Rollback { to }`
- Implementation: `crates/cognicode-cli/src/cmd/layout.rs::cmd_rollback`
- Pinning tests: `crates/cognicode-cli/src/cmd/layout.rs::tests::t_e86_4_*`
  (5 unit tests, all green at HEAD)
- Predecessor: `openspec/changes/archive/2026-09-21-e86-3-cogh-uninstall-coverage/`
- Sequencer (next): this completes the e86 family; e87 (channels) and
  e88 (real-PC UAT) remain as future cycles per the umbrella
  e84-cognicode-distribution-artifact-contract state.
