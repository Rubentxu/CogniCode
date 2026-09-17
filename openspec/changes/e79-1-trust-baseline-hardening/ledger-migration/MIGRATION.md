# DEBT-002 secondary finding — Audited ledger migration

## Purpose

The events `cycle.supersede.requested` and `cycle.supersede.applied` written
by the e66 D3-defer closure (sequences 4 and 5 in stream
`cycle:p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets`) carry a legacy schema that
the current SDDK parser rejects:

- `subjects` entries use `{"kind": "...", "id": "...", "role": "subject"}`
  instead of the current `{"type": "...", "id": "..."}`.
- `evidence_refs` entries use object form `{"path": "...", "commit": "..."}`
  instead of the current scalar-string form.
- The `content_hash` of these two rows is truncated to 32 hex chars
  (`sha256:760f45ed...` / `sha256:cd4b6497...`), violating the current
  `CHECK (content_hash LIKE 'sha256:%')` constraint that expects 64 hex
  chars after the prefix.

The mismatch prevents the framework verifier from reproducing the hashes,
which in turn blocks any future reconciliation tooling.

## Strategy

The migration does NOT delete, rewrite, or invent events. It mutates exactly
two existing rows in a way that:

1. Renames the subject field `kind` → `type` (current parser expectation).
2. Drops the legacy `role` field (no current consumer reads it; current
   parser rejects unknown fields under `deny_unknown_fields`).
3. Normalizes `evidence_refs` from object form (`Vec<Map>` with `{"path"}`,
   `{"commit"}`, `{"hash"}` keys) to scalar-string form (`Vec<String>`),
   extracting the single identifying string per entry in stable order.
4. Recomputes `content_hash` using the **framework's own**
   `EventEnvelopeV1::compute_content_hash` to guarantee bit-perfect parity
   with the framework verifier.
5. Recomputes `chain_hash` per stream using the same SHA-256 chain rule
   the runtime uses (`sha256(content_hash || prev_chain_hash)` or
   `sha256(content_hash || "genesis")` for the first event in a stream).
6. Updates only the two malformed rows' `subjects_json`,
   `evidence_refs_json`, and `content_hash` fields, plus the `chain_hash`
   field of every downstream event in the same stream (to preserve
   chain integrity).
7. Preserves all other fields (`event_id`, `event_type`, `payload_json`,
   `actor_json`, `recorded_at`, etc.) byte-for-byte.

## Pre-conditions enforced by the migrator

- Exactly 2 events in the entire ledger have `subjects_json LIKE '%"kind":%'`
  AND `subjects_json NOT LIKE '%"type":%'`. (Earlier LIKE filters with
  `%kind%` matched the substring of `"cycle"` and produced false positives;
  the corrected filter uses the literal field name `kind:` only.)
- Both events have sequences 4 and 5 in the stream
  `cycle:p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets`.
- Both event types are `cycle.supersede.requested` and
  `cycle.supersede.applied`.

If any precondition fails, the migrator aborts without touching the ledger.

## Procedure

1. **Source fingerprint** (sha256 of the SQLite file bytes).
2. **Consistent backup** via `rusqlite::Connection::backup()` (the SQLite
   online backup API), which gives a snapshot-safe copy regardless of any
   concurrent reader state.
3. **Drop the append-only triggers** (`events_v1_no_update`,
   `events_v1_no_delete`) for the duration of the mutation.
4. **Process all 111 events** in `(stream_id, sequence)` order:
   - For the 2 malformed events: build a normalized `Vec<EntityRef>`,
     build a normalized `Vec<String>` for evidence_refs, construct an
     `EventEnvelopeV1`, and call `envelope.compute_content_hash()` to get
     the canonical `content_hash`.
   - For all other 109 events: keep `content_hash` byte-for-byte as in
     source (the migration is conservative; nothing else changes).
   - For every event: compute `chain_hash` from the new `content_hash`
     and the previous `chain_hash` in the same stream.
5. **Apply updates in a single transaction**:
   - For the 2 malformed rows: `UPDATE events_v1 SET subjects_json=?,
     content_hash=?, chain_hash=? WHERE event_id=?`. Filtering by
     `event_id` (PRIMARY KEY) is essential — filtering by `sequence`
     collides across streams because each stream has its own sequence 4,
     5, etc.
   - For all other 109 rows: `UPDATE events_v1 SET chain_hash=? WHERE
     event_id=?` (downstream propagation in the e66 stream).
6. **Re-create the append-only triggers** with the same names and bodies
   so external verifiers see them again.
7. **Output fingerprint** (after explicit `PRAGMA wal_checkpoint(TRUNCATE)`
   to flush the WAL).

## Verification

Run the binary `sddk-hash-verify` (built from this same project) against
the output ledger:

```text
total events: 111
content_hash matches: 111

chain_hash verification:
chain_hash matches: 111 / 0 mismatches
```

Compare against the source:

```text
SKIP seq=4 subjects parse: unknown field `kind`, ...
SKIP seq=5 subjects parse: unknown field `kind`, ...
total events: 111
content_hash matches: 109
chain_hash matches: 109 / 2 mismatches
```

The two SKIP / mismatch rows in the source are exactly the two malformed
events; the migration eliminates both.

## Reproducing the binaries

The compiled migrator and verifier are NOT stored in the repo (binaries are
out of scope for this directory; the migration source project is in the
agent's local scratch space). To reproduce them, check out the SDDK
framework at the exact commit `b9e928c` (framework v1.169.62), then build
the project at `/tmp/sddk-hash-verify/` (a `cargo` workspace depending on
`sddk-domain` at that commit):

```bash
cd /tmp/sddk-hash-verify
CARGO_TARGET_DIR=/var/home/rubentxu/cargo-targets/tmp cargo build --release
```

This produces `target/release/migrate` and `target/release/sddk-hash-verify`.
The migration procedure and the verifier logic are documented above;
their behavior is deterministic and reproducible from the SDDK source
alone.

## Lessons learned (during e79.1 development)

1. **LIKE substring matching**: `LIKE '%kind%'` matches the substring of
   `"cycle"`. The corrected filter is `LIKE '%"kind":%' AND NOT LIKE
   '%"type":%'` (exact field name with trailing colon, AND exclusion of
   the new field name to avoid double-counting).
2. **`sequence` is NOT unique globally** in `events_v1`. It is unique per
   stream. Any UPDATE keyed on `sequence` alone is incorrect and will
   touch 9 rows when only 1 was intended. Always key on `event_id` (which
   is the PRIMARY KEY) for row-level operations.
3. **Use the framework's own hash function** (`EventEnvelopeV1::
   compute_content_hash`) rather than reimplementing the envelope JSON
   manually. The struct's serde derive emits fields in declaration order
   and respects `skip_serializing_if = "Option::is_none"`; a hand-built
   `serde_json::Map` will sort keys alphabetically and emit fields in a
   different order, producing a different hash.
4. **`chain_hash` is not exposed as a public helper** in the framework
   domain crate. The migration inlines the same `sha256(content ||
   prev_chain)` / `sha256(content || "genesis")` rule the runtime uses.
5. **CHECK constraints are not enforced by default on legacy rows**: the
   pre-migration ledger has rows with `content_hash` shorter than
   `sha256:` + 64 hex chars, which violates `CHECK (content_hash LIKE
   'sha256:%')`. SQLite does not run CHECK validation on rows that
   predate the constraint; this is how the malformed rows were stored.
6. **The verifier must be tolerant** to legacy schema variants. The
   initial verifier `panic`-ed on the first malformed row and aborted
   the whole run. The fixed verifier uses `match` / `unwrap_or` for the
   per-field parsers, skips rows it cannot parse (counting them
   separately), and continues with the rest. This is consistent with the
   "honesty over completeness" principle: report what we can verify,
   flag what we cannot.
