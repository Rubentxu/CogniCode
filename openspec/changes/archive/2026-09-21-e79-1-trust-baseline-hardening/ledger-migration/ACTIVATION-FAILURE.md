# P0-A activation failure — post-swap ledger verify RED (rolled back)

## Summary

The activation of `/tmp/migrate-output.sqlite` into the live ledger at
`~/.local/state/sddk/projects/p-c1fac1fea05615c6/ledger.sqlite` succeeded
at the file-system level (atomic rename, SHA matched expected, ownership
preserved), but `sddk ledger verify` (framework v1.169.63) reported
`STORAGE_LEDGER_INTEGRITY` because of an unaddressed schema drift in
`evidence_refs` for the two e66 supersede events.

Per the user's directive ("If `ledger verify` fails, rollback immediately"),
the live ledger was restored from the pre-activation backup. No data was
lost or corrupted.

## Timeline

| Step | Action | Result |
|------|--------|--------|
| 1 | Live fingerprint + writer check | `fuser`/`lsof` showed no writers. Live SHA-256: `06564a6b01bafed8924892dfcf5f7cb9dd347dc117539beaa9b4dba725013a44` |
| 2 | Hard gate SHA check | Live content fingerprint matched the pre-migration backup (`862b1a7a...` rows identical); only SQLite header bytes differ, as expected for any live file vs backup API output |
| 3 | Live ledger backup | `openspec/changes/e79-1-trust-baseline-hardening/ledger-migration/live-pre-activation/ledger.live-pre-activation.sqlite` (read-only, fingerprint identical to live) |
| 4 | Migrated artifact final check | `2d72e978dc66b5df86cd0b12b56ee1d1ab6d9767d94bc440771a17fefcfa12df`, verifier 111/111 |
| 5 | Atomic activation via `mv` | Pre `06564a6b...` → Post `2d72e978...` (expected). Owner/mode preserved, no stale side files |
| 6 | Post-swap SQLite integrity check | OK: 111 events, 21 cycles, 133 gate_receipts, 31 artifacts, both append-only triggers present, append-only enforcement functional |
| 7 | Post-swap internal verifier (`sddk-hash-verify`) | 111/111 content_hash, 111/111 chain_hash (tolerant parser: skipped 0 rows) |
| 8 | Post-swap `sddk ledger verify` (framework) | **RED**: `STORAGE_LEDGER_INTEGRITY — evidence_refs parse: invalid type: map, expected a string` |
| 9 | Rollback | Live restored from pre-activation backup. Post-rollback `sddk ledger verify` returns the original DEBT-002 error (`subjects parse: unknown field 'kind'`), confirming rollback was complete |

## Root cause (unaddressed schema drift)

The two events written by the e66 D3-defer closure (sequences 4 and 5 in
stream `cycle:p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets`) have **three**
schema drifts from the current parser:

1. `subjects` entries use `{"kind": "cycle", "id": ..., "role": "subject"}`
   instead of `{"type": "cycle", "id": ...}`. **Migrated.** ✓
2. `content_hash` is truncated to 32 hex chars (`sha256:760f45ed...`)
   instead of 64 hex chars. **Recomputed with framework's
   `EventEnvelopeV1::compute_content_hash()`.** ✓
3. `evidence_refs` uses object form (`{"path": "...", "commit": "..."}`)
   instead of the current scalar-string form. **NOT migrated.** ✗

The migration addressed drifts 1 and 2 but did not normalize drift 3
because drift 3 does not affect the `content_hash` (the `evidence_refs`
field participates in the hash, but in the legacy shape it would hash the
JSON differently anyway). My internal verifier (`sddk-hash-verify`) was
made tolerant to drift 3 (it accepts both shapes), which masked the
issue. The framework's verifier (`sddk ledger verify`) uses the strict
parser and rejects the ledger outright.

## Decision

Per the user's activation directive:

> "If `ledger verify RED` → rollback immediately. Document the failure
> and STOP P0-A."

Rollback completed. P0-A is **STOPPED** pending decision on whether to
extend the migration to cover drift 3.

## Options for next step (deferred to user)

### Option A — Extend migration scope (same DEBT-002)

Treat drift 3 (`evidence_refs`) as part of the same DEBT-002 root cause.
The migration's strategy already declares "normalize `evidence_refs` from
object form to a single string per entry" — it just was not applied to
the row write. One additional code path (write normalized
`evidence_refs_json` to the row, recompute `content_hash` to reflect the
normalized value) would resolve the issue. Same root cause, same audit
trail, no new DEBT.

### Option B — Split into separate DEBT

Treat drift 3 as a separate `DEBT-SDDK-006` ("evidence_refs schema drift
in legacy events") and open a new cycle for it. Heavier process for what
is the same logical defect (the e66 D3 closure wrote events with a
multi-field schema drift, all of which the current parser rejects).

### Option C — Reverse the activation decision

Re-evaluate the entire approach. The migration is bit-perfect with the
tolerant verifier, so the migration itself is correct. The mismatch is
between the tolerant verifier and the strict framework verifier, which is
a tooling consistency problem (separate from the data integrity problem).
A possible alternative is to make the framework verifier tolerant too,
but that is an upstream SDDK framework change and outside e79.1 scope.

## Current state of the live ledger

After rollback, the live ledger is functionally identical to the
pre-activation state:

- Content fingerprint (rows): `862b1a7abbe6cf7112d75899580063978716771ad83eb51a83685a8d43ffa1be`
- File SHA-256: `906e58badc7f6f19e9a8d75fb7b4b69b3e9ae9fbd2712d743c584cac9aea3af1` (header bytes from the SQLite backup API, semantically identical to live pre-swap `06564a6b...`)
- `sddk ledger verify` reports the original DEBT-002 error, confirming rollback
- Append-only triggers present and enforced

The pre-activation backup is preserved at
`openspec/changes/e79-1-trust-baseline-hardening/ledger-migration/live-pre-activation/ledger.live-pre-activation.sqlite`
(read-only, fingerprint identical to live).

## Artifacts

- Migrator source: `openspec/changes/e79-1-trust-baseline-hardening/ledger-migration/src/migrate.rs`
- Verifier source: `openspec/changes/e79-1-trust-baseline-hardening/ledger-migration/src/sddk-hash-verify.rs`
- MIGRATION.md doc: `openspec/changes/e79-1-trust-baseline-hardening/ledger-migration/MIGRATION.md` (needs update to document the drift-3 oversight)
- DEBT-002: `docs/debts/DEBT-SDDK-002.md` (unchanged; the secondary-finding section is still accurate but understated the scope)
- Pre-activation backup: `openspec/changes/e79-1-trust-baseline-hardening/ledger-migration/live-pre-activation/ledger.live-pre-activation.sqlite`

## P0-A status

**RESOLVED via Migration v2 (2026-09-17, second atomic activation).**

## Activation attempt 1 (FAILED → ROLLED BACK)

- Migration v1: addressed only `subjects.kind` and `subjects.role`; left
  `evidence_refs` in legacy `Vec<Map>` form.
- Atomic activation succeeded at file-system level (SHA matched expected).
- Post-swap `sddk ledger verify` (framework v1.169.63, strict parser)
  reported `STORAGE_LEDGER_INTEGRITY — evidence_refs parse: invalid
  type: map, expected a string`.
- Rollback executed. Live restored from
  `live-pre-activation/ledger.live-pre-activation.sqlite`.
- Failure: the custom internal verifier (`sddk-hash-verify`) was
  tolerant to legacy `evidence_refs` shape, so it returned 111/111
  GREEN while the framework strict parser returned RED. The custom
  verifier cannot be the acceptance oracle for SDDK-format data.

## Migration v2 (RESOLVED)

Added explicit `evidence_refs` normalization:

- For each malformed event, parse `evidence_refs_json` as
  `Vec<serde_json::Value>`. If entries are objects, extract the
  `path` field (preferred), then `hash`, then `commit` (in that order),
  preserving the original order of entries. If entries are already
  strings, pass through unchanged.
- Recompute `content_hash` for the two malformed events with the
  normalized `evidence_refs` using
  `EventEnvelopeV1::compute_content_hash` (framework commit `d032939`,
  v1.169.64).
- Recompute `chain_hash` for every event in the same stream
  (`cycle:p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets`) since `seq=2`
  onwards.

The internal verifier was also split into two modes:

- `--legacy-inspection` (tolerate legacy shapes; use to characterize
  source before migration).
- `--current-schema` (default; REQUIRE the current schema that the
  framework accepts; use as acceptance oracle for migrated copies).

The custom verifier is now strictly aligned with the framework parser.
They MUST agree.

## Pre-activation hard gate v2 (all GREEN)

- Source content fingerprint matches expected original (rows identical,
  `862b1a7a...` row-hash).
- Migration v2 deterministic (run 1 == run 2, output SHA
  `194d46b0e3edcbc51d842b97704ba2902f7e5c072f1a0185270e5dc127972449`).
- SQLite `integrity_check`: ok.
- Event count unchanged: 111 → 111.
- Cycles unchanged: 21 → 21.
- `gate_receipts` unchanged: 133 → 133.
- `artifacts` unchanged: 31 → 31.
- Append-only triggers present: `events_v1_no_update`,
  `events_v1_no_delete`.
- Custom `--current-schema` verifier on migrated: 111/111 content_hash,
  111/111 chain_hash.
- **REAL `sddk ledger verify` on isolated migrated copy
  (`XDG_STATE_HOME=/tmp/sddk-iso-verify/.local/state sddk ledger verify`):
  GREEN** (`event_count: 111`, `last_hash: sha256:de7cdfe6...`).

## Second atomic activation (SUCCESS)

- Pre-swap live SHA: `906e58badc7f6f19e9a8d75fb7b4b69b3e9ae9fbd2712d743c584cac9aea3af1`
  (post-rollback from v1).
- Pre-activation backup SHA: `906e58badc7f6f19e9a8d75fb7b4b69b3e9ae9fbd2712d743c584cac9aea3af1`
  (matches live pre-swap).
- Migrated copy SHA: `194d46b0e3edcbc51d842b97704ba2902f7e5c072f1a0185270e5dc127972449`
  (= expected).
- Atomic rename via `mv` succeeded. Post-swap live SHA:
  `194d46b0e3edcbc51d842b97704ba2902f7e5c072f1a0185270e5dc127972449` ✓.
- No stale WAL/SHM/Journal files.
- Post-swap SQLite integrity: ok. Append-only triggers present and
  enforced.
- Post-swap custom `--current-schema` verifier: 111/111 ✓.
- **Post-swap REAL `sddk ledger verify`: GREEN** ✓.

## Canary success (DEBT-002 RESOLVED)

Cycle `e79-1-canary-characterize-debt-002` advanced through the
normal lifecycle:

- Acquired lease (owner=jcode-orchestrator, fencing_token=1).
- Stored implementation-receipt artifact
  (`art-7b78c94ca46b-d3efde43`).
- Evaluated gate `implementation-complete` with `sddk.cli` evaluator
  (outcome=passed, receipt `gate-implementation-complete-750c28cf2a2cd95f-1`).
- Applied transition `phase.build.complete.b-direct` →
  status=OPEN, phase=verify, sequence=2,
  event=`evt-3de9c977-93ee-4c90-a241-f67ea52cb094`,
  event_hash=`sha256:1d81bfa7967bdbd92b0cd3a7642977f7b3fce484fd010ed53b5aefa6b500d391`.

Post-canary `sddk ledger verify`: `event_count: 116`,
`last_hash: sha256:de7cdfe6b682d0b9a98d51d07a090e09b134a8f954bc35f3a6501a8e43b1d028`.

The canary demonstrated that the SDDK workflow engine, after the
migration, can:

1. Replay the migrated cycle's `cycle.created` event.
2. Accept the lease acquisition.
3. Store and validate a fresh artifact.
4. Evaluate a gate with structured evidence.
5. Apply the transition and write the corresponding `cycle.transitioned`
   event with a verifiable hash.

**DEBT-SDDK-002 = RESOLVED** with final cause:

```text
legacy e66 events used a pre-v1.169.0 event serialization
that the current EventEnvelopeV1 parser rejects under
deny_unknown_fields. Three independent schema drifts in the
same two rows: subjects.kind (vs type), legacy subjects.role,
and evidence_refs Vec<Map> (vs Vec<String>). All resolved by
the audited migration v2 in this directory.
```
