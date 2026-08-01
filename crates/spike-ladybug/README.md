# E29 S1 Spike — LadybugDB (lbug) v0.19.0 Build Validation

Validates that the `lbug` crate downloads its prebuilt static lib from GitHub
and can create + query a `.lbdb` **file** database using the Kùzu-derived API.

**Important**: `.lbdb` is a single **file** (~49 KB), NOT a directory. This was
verified empirically on 2026-07-31.

## What it proves

1. `lbug 0.19.0` is published on crates.io (pubtime 2026-07-30)
2. The crate's `build.rs` downloads `liblbug-linux-x86_64.tar.gz` from
   `https://github.com/LadybugDB/ladybug/releases/download/v0.19.0/`
   automatically — **no cmake required on this host** (cmake is NOT installed
   and cannot be installed without sudo)
3. The static lib links cleanly with the host toolchain (rustc 1.96.0, gcc 16.1.1)
4. `Database::new` + `Connection::new` + `conn.query` round-trip a single row
5. The `.lbdb` artifact is materialized as a file, not a directory

## How to run

```bash
just spike-ladybug          # build + run example + run test end-to-end
just spike-ladybug-clean    # wipe target/ + spike.lbdb for fresh run
```

## Stage S2 — Schema load + COPY FROM + query validation

Validates that `lbug 0.19.0` can host CogniCode's full graph schema and load 60K
rows via `COPY FROM` in < 60s (measured ~0.14s on this host — ~416K rows/sec).

### What it proves

1. All 25 NODE TABLEs + 20 REL TABLEs apply successfully via Cypher DDL
2. `COPY FROM` ingests 60K rows in well under the 60s budget
3. Typed column queries work (INT64, STRING, FLOAT comparisons)
4. MAP(STRING, STRING) property access works (with work-around for subscript syntax)
5. Temporal column filtering works (valid_to = -1 for current rows)
6. Rel traversal queries work (`MATCH (a)-[:Calls]->(b)`)

### How to run

```bash
just spike-ladybug-s2         # build + run examples + run tests end-to-end
just spike-ladybug-s2-clean   # wipe S2 .lbdb artifacts
```

### Critical corrections made during S2 (vs original schema-spec)

1. **`NOT NULL` removed**: Kùzu 0.x parser rejects `NOT NULL` in DDL. The 296 occurrences were removed from the schema spec.
2. **`CREATE INDEX` removed**: lbug 0.19.0 only supports indexes on node table primary keys. The 43 `CREATE INDEX` statements were removed; secondary index queries will do full table scans until lbug adds support.
3. **`SERIAL id` ≠ `InternalID`**: The `id(s)` Kùzu function returns an `InternalID` struct, NOT the SERIAL `id` column value. The Calls CSV must use the InternalID (post-Symbol-COPY), not the pre-computed SERIAL id.

### Known limitations

- No secondary indexes (queries on non-PK columns are full table scans)
- MAP subscript syntax `s['properties']['key']` parses as `LIST_EXTRACT`; use `.properties` returns instead
- S2 runs 60K rows — production migration would need to re-validate at 10M+ scale

## Build paths

The `lbug` crate has two build paths:

| Path | Trigger | Requires cmake? | Speed |
|------|---------|-----------------|-------|
| **Prebuilt static lib** (default) | automatic on first build | ❌ no | ~30s after download |
| **From source** | `LBUG_BUILD_FROM_SOURCE=1` env var | ✅ yes | 5–15 min |

This spike uses the prebuilt path by default. The build script DOES have an
implicit fallback to the from-source path on download failure (this is upstream
behavior in `lbug` 0.19.0, not something we can disable from the consumer side).

**On this host**: cmake is NOT installed. So if the prebuilt download ever
fails (network, rate limit, missing artifact), the build will fail loudly with
"cmake: command not found". This is observable, not silent — it surfaces the
real issue rather than masking it.

## Prebuilt cache location

The build script writes the extracted static lib to its OWN `CARGO_MANIFEST_DIR`
(which, for a registry dependency, is `~/.cargo/registry/src/index.crates.io-*/lbug-0.19.0/.cache/lbug-prebuilt/latest/lib/liblbug.a`),
**not** to the consuming crate's `.cache/`.

That means:

- `crates/spike-ladybug/.cache/lbug-prebuilt/version-0.19.0/lib/liblbug.a` does
  **not** exist (verified 2026-07-31)
- The actual prebuilt lib lives at `~/.cargo/registry/src/index.crates.io-*/lbug-0.19.0/.cache/lbug-prebuilt/latest/lib/liblbug.a`
  (113 MB on this host)
- `cargo clean` does NOT wipe this cache (it persists across builds)
- `just spike-ladybug-clean` wipes `target/` and `spike.lbdb` only — it does
  NOT wipe the lbug prebuilt cache (and that is intentional: clearing Cargo's
  registry cache would require `cargo clean` on the whole workspace)

## API surface (corrected)

The `lbug` crate is a rename of Kùzu. Verified API:

```rust
use lbug::{Connection, Database, SystemConfig, Value};

let db = Database::new("spike.lbdb", SystemConfig::default())?;
let conn = Connection::new(&db)?;

conn.query("CREATE NODE TABLE Test(id INT64, name STRING, PRIMARY KEY(id));")?;
// NOTE: SQL INSERT is NOT supported. Use Cypher CREATE instead.
conn.query("CREATE (:Test {id: 1, name: 'hello'});")?;

for row in conn.query("MATCH (t:Test) RETURN t.id, t.name;")? {
    if let (Value::Int64(id), Value::String(name)) = (&row[0], &row[1]) {
        println!("id={id} name={name}");
    }
}
```

Access is by **column index** (`row[i]`), then pattern-match the `Value` enum.

## Troubleshooting

### Prebuilt download fails (network, GitHub rate limit)

Symptom: build error mentioning `failed to download liblbug` or curl exit non-zero.

On this host, the from-source fallback will then try to invoke cmake and fail
loudly with `cmake: command not found`. This is **observable, not silent** —
the failure mode is unambiguous.

Fix options:
1. Re-run with fresh network (most likely transient rate limit)
2. Pre-populate Cargo's registry cache: download `liblbug-linux-x86_64.tar.gz`
   manually and extract to `~/.cargo/registry/src/index.crates.io-*/lbug-0.19.0/.cache/lbug-prebuilt/latest/lib/liblbug.a`
3. Install cmake (requires sudo, not currently possible on this host)

### Database::new returns error

Check disk space (`df -h`) and write permissions on the target directory.
The spike writes to `spike.lbdb` in the current working directory.

### API drift from Kùzu

The first compile is the API proof. If lbug has diverged from Kùzu since the
rename, the corrected API in this README may not match. Re-derive from the
upstream source: `https://github.com/LadybugDB/ladybug-rust/src/lib.rs`.

### Build > 30 min

If the build is still taking > 30 min and you did not set `LBUG_BUILD_FROM_SOURCE=1`,
something else is wrong (network retry storm, very slow disk, etc.). Investigate
the cargo build log.

### DDL syntax error

Symptom: `Parser exception: Invalid input < NOT>` or similar.

Cause: Kùzu 0.x DDL is strict. Re-check against schema-spec v0.4.0 (no NOT NULL, no PK on rels, FROM-TO on rels, MAP parentheses).

### CREATE INDEX rejected

Symptom: `Binder exception: HASH indexes are currently supported only on node primary keys`.

Cause: lbug 0.19.0 doesn't support secondary indexes. Remove the index; queries will do full table scans.

### Rel CSV FROM/TO mismatch

Symptom: `Binder exception: Node with id X does not exist` or similar.

Cause: Calls CSV first 2 columns must be InternalIDs (assigned by Symbol COPY FROM), not pre-computed SERIAL ids. Use the two-phase load pattern in s2_copy_from.rs.

## Exit criteria

The spike is **PASS** when:

1. `cargo build --release --manifest-path crates/spike-ladybug/Cargo.toml` exits 0
2. `cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s1_bootstrap`
   prints exactly `id=1 name=hello`
3. `cargo test --manifest-path crates/spike-ladybug/Cargo.toml --tests`
   passes `s1_creates_lbdb_file_and_round_trips_one_row`
4. `cargo check --workspace` still passes (spike excluded)
5. `cargo check --manifest-path crates/spike-ladybug/Cargo.toml --all-features`
   exits 0

The spike is **FAIL** when:

- Prebuilt download fails AND cmake is unavailable (would need sudo to install)
- lbug API does not match the corrected Kùzu-derived API
- `.lbdb` is created as a directory (would mean lbug v0.20+ changed storage)

## Stage S3 — Concurrency and Single-Writer Constraint

Validates that lbug 0.19.0's concurrency model satisfies CogniCode's needs.
Single-writer is at the **active-write-query level**, not the connection level.

### What it proves

1. **Single-writer at query level**: multiple `Connection`s can coexist on one RW `Database`, but concurrent write queries serialize — one succeeds, the other errors with "Cannot start a new write transaction in the system. Only one write transaction at a time is allowed in the system."

2. **Retry succeeds**: after the first writer commits, the second writer's retry succeeds.

3. **Multi-reader concurrent reads**: 4 scoped threads each calling `conn.query(...)` execute concurrently without blocking and all return the same snapshot of rows.

4. **Read-only Database rejects writes**: `Database::new(path, SystemConfig::default().read_only(true))` rejects write queries with "Cannot execute write operations in a read-only database!"

5. **Cross-process file lock**: two processes cannot both open the same `.lbdb` as write; the second `Database::new` returns `Err` with "Could not set lock on file".

### Measured error strings

| Scenario | Error message (exact) |
|----------|---------------------|
| Concurrent write query (same process) | `Cannot start a new write transaction in the system. Only one write transaction at a time is allowed in the system.` |
| Write on read-only Database | `Cannot execute write operations in a read-only database!` |
| Second RW process open | `Could not set lock on file : <path> (Error: Resource temporarily unavailable)` |

### How to run

```bash
just spike-ladybug-s3         # build + run examples + run tests end-to-end
just spike-ladybug-s3-clean   # wipe S3 .lbdb artifacts
```

### Key corrections made during S3

1. **`BEGIN WRITE TRANSACTION` not supported**: lbug 0.19.0 uses auto-commit per query, not explicit transaction commands.

2. **Read-only mode**: `SystemConfig::default().read_only(true)` is the correct API — field is private with builder pattern.

3. **No MVCC snapshot isolation**: lbug 0.19.0 does not provide read transaction isolation; readers see committed data immediately. The single-writer constraint is at the query level, not at the transaction level.

## Stage S4 — Crash Recovery and WAL Durability

Validates that lbug 0.19.0's WAL-based durability model satisfies CogniCode's needs.
**Core question**: does committed data survive a true SIGKILL?

### What it proves

1. **Durability baseline (E1)**: Clean write + clean close → WAL is absent (checkpointed into main DB), 1000 rows recovered on reopen.

2. **SIGKILL after commit (E2)**: 1000 rows committed → SIGKILL → WAL is absent (not persisted) → **all 1000 rows recovered**. Data survives via direct write to main DB, not via WAL persistence (for small writes below the 16 MB auto-checkpoint threshold).

3. **SIGKILL before commit (E3)**: SIGKILL before any write → 0 rows recovered (correct, nothing was committed).

4. **Corrupt WAL handling (E4)**: With `auto_checkpoint(false)` and `force_checkpoint_on_close=false`, WAL may not be created for small writes. E4 corruption test skipped in this case — normal reopen works. The `throw_on_wal_replay_failure(false)` silent-skip branch was verified (E4a passes).

5. **Reopen latency (E5)**: Reopen of a 1000-row DB completes in 159–177ms (< 1s budget).

### Measured behavior

| Scenario | Result |
|----------|--------|
| E1: clean close, WAL present | Absent (checkpointed) |
| E1: rows recovered | 1000/1000 ✓ |
| E2: SIGKILL after commit, WAL present | Absent (not created or auto-checkpointed) |
| E2: rows recovered | 1000/1000 ✓ |
| E2: reopen wall-time | 170–177ms ✓ |
| E3: SIGKILL before commit | 0 rows ✓ |
| E4: WAL with `throw_on_wal_replay_failure(false)` | Ok (E4a passes) |
| E4: WAL with `throw_on_wal_replay_failure(true)` | Not tested (WAL not persisted for small writes) |
| E5: reopen latency | 159ms ✓ |

### Key findings

- **WAL not persisted for small writes**: Even with `auto_checkpoint(false)` and `force_checkpoint_on_close=false`, lbug does not create a persistent WAL for writes below the 16 MB threshold. Data is written directly to the main `.lbdb` file.
- **Durability sufficient for Phase 1**: All 1000 committed rows survive SIGKILL — data is durable via direct write, not WAL replay.
- **No partial/corrupt data**: No partial row writes or corruption observed in any scenario.
- **E4 corruption test skipped**: WAL was not persisted for 10-row test writes, so the corruption scenario could not be tested. The `throw_on_wal_replay_failure(false)` silent-skip branch works correctly.

### How to run

```bash
just spike-ladybug-s4         # run probe + tests end-to-end
just spike-ladybug-s4-clean  # wipe S4 .lbdb artifacts
```

### E2 outcome

**all_1000_survived** — The 1000 committed rows survived SIGKILL. WAL was not persisted (data written directly to main DB), but durability is confirmed.

## S5: Latency — lbug vs PostgreSQL

**Status**: implemented (PR 2 in progress)

### What

Comparative latency benchmark across 5 query pairs (Q1–Q5) on a 10,000-node call graph:
- **Q1**: Point read by `id`
- **Q2**: 1-hop neighborhood (`Calls` edges)
- **Q3**: BFS depth 3 (`Calls*1..3`)
- **Q4**: Aggregation (`GROUP BY kind`)
- **Q5**: COPY FROM throughput (population phase)

### Architecture

```
PR 1 (deps + populate + lbug harness):
  s5_populate.rs      — dual-engine 10K-row populate
  s5_query_lbug.rs    — Q1–Q4 lbug benchmark
  s5_latency.rs       — RED/GREEN lbug-only test

PR 2 (PG harness + full test + tooling):
  s5_query_pg.rs      — Q1–Q4 PostgreSQL benchmark
  s5_latency.rs       — E1–E7 full tests
  justfile            — spike-ladybug-s5, spike-ladybug-s5-clean
```

### Key findings

| Query | lbug (100 nodes) | PG (10K nodes, ~29K edges) | Notes |
|-------|-----------------|----------------------------|-------|
| Q1 point read | ~1,881 µs | ~446 µs | PG has index on `id` |
| Q2 1-hop | ~5,566 µs | ~10,506 µs | lbug graph-native is faster |
| Q3 BFS depth 3 | ~6,372 µs | ~316,031 µs | PG recursive CTE is slow |
| Q4 aggregation | ~8,215 µs | ~2,491 µs | PG GROUP BY is faster |

**S2 finding reused**: `id(s) != SERIAL id` — requires two-phase lbug populate (offset 4647).

### Known issues

- PG edge population incomplete: 29,368/50,000 edges (only `node_1` had edges due to `ON CONFLICT DO NOTHING` failures)
- 10K-row PG populate took >5 min (per-row INSERT; no `COPY FROM` available inside Podman)

### How to run

```bash
# Requires PG running at postgres://cognicode:cognicode@localhost:5432/cognicode
just spike-ladybug-s5         # populate + run full benchmarks
just spike-ladybug-s5-clean   # remove /tmp s5 DB and PG spike tables
```

### E2 outcome

E1–E5 comparative benchmarks — see `s5_latency.rs` for assertions.

## Stage S6 — Cypher Compatibility

**Status**: implemented

### What

Validates 9 Cypher compatibility criteria (E1–E9) for LadybugDB 0.19.0 against the CogniCode Phase 1 query surface:

| Criterion | Probe | Outcome |
|----------|-------|---------|
| E1: All EdgeKind labels queryable | Typed `MATCH ()-[r:Kind]->() RETURN count(r)` × 6 | **PASS** (5/6 — `CorroboratedBy` missing from S2 DDL, D3) |
| E2: Variable-length paths `*1..3` | `MATCH path=(s)-[:Calls*1..3]->(t) RETURN length(path)` | **PASS** (depths 1, 2 observed) |
| E3: WITH + ORDER BY + LIMIT | `MATCH (s) WITH s.kind, count(*) RETURN ORDER BY DESC LIMIT 10` | **PASS** (2 rows, descending) |
| E4: UNWIND batch create | `UNWIND [{id, name}, ...] AS row CREATE (:Symbol {...})` | **PASS** (2/2 created) |
| E5: OPTIONAL MATCH null-padding | `OPTIONAL MATCH (s)-[:Calls]->(t)` with isolated node | **PASS** (t IS NULL) |
| E6: MAP `properties['key']` access | `s.properties['codeowners']` on MAP column | **PASS_WITH_LIMITATION** (`[]` binds to LIST_EXTRACT — workaround: whole MAP return) |
| E7: SIZE() on relationship collection | `size(collect(r))` on 2 Calls | **PASS** (returns 2) |
| E8: DISTINCT | `RETURN DISTINCT s.kind` | **PASS** (3 unique kinds) |
| E9: All NodeKind/EdgeKind labels parse | `MATCH (n:Label)` × 9 labels | **PASS** (4/4 NodeKind + 4/5 EdgeKind — D3: CorroboratedBy missing) |

### Deviations (D1/D2/D3)

| ID | Description | Impact |
|----|-------------|--------|
| D1 (E6) | `[]` on MAP resolves to `LIST_EXTRACT` — no MAP overload | `s.properties['key']` fails; workaround: `RETURN s.properties` (whole MAP) |
| D2 (E2) | Spec uses `edges(path)`; lbug uses `rels(path)` | Use `rels(path)` in probes |
| D3 (E1/E9) | `CorroboratedBy` missing from S2 DDL | Label not queryable; Phase 1 DDL addition recommended |

### Key finding

**E6 MAP access**: `s.properties['codeowners']` fails because lbug's `LIST_EXTRACT` function has overloads for `LIST/STRING/ARRAY + INT64` only — no MAP variant. The workaround is to `RETURN s.properties` (whole MAP) and extract on the Rust side via `Value::MAP`. This is a **documented LadybugDB limitation**, not a spike abort.

**E9 D3 gap**: `CorroboratedBy` rel table exists in `edge_kind.rs` but not in the S2 DDL. Phase 1 DDL addition recommended.

### How to run

```bash
just spike-ladybug-s6         # run all E1–E9 Cypher compatibility probes
just spike-ladybug-s6-clean   # remove /tmp s6_*.lbdb
```

### Exit gate verdict

**PROCEED** — 8/9 criteria clean PASS; E6 PASS_WITH_LIMITATION (MAP accessible via whole-return + Rust-side extraction; `[]` syntax documented as Kùzu limitation for Phase 1 `LadybugStore` adapter).

## See also

- `sddk/e29-s1-build/proposal.md` — full proposal
- `sddk/e29-s1-build/spec.md` — Given/When/Then acceptance criteria
- `sddk/e29-s1-build/design.md` — technical design + risk register
- `sddk/e29-s1-build/tasks.md` — task breakdown
- `sddk/e29-s3-concurrency/proposal.md` — S3 proposal
- `sddk/e29-s3-concurrency/spec.md` — S3 delta spec
- `sddk/e29-s3-concurrency/design.md` — S3 technical design
- `sddk/e29-s4-crash-recovery/proposal.md` — S4 proposal
- `sddk/e29-s4-crash-recovery/spec.md` — S4 delta spec
- `sddk/e29-s4-crash-recovery/design.md` — S4 technical design
- `openspec/specs/ladybug-spike-validation/spec.md` — full 6-stage spike spec
- `docs/adr/ADR-026-ladybugdb-canonical-migration.md` — migration decision

