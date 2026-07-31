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

## See also

- `sddk/e29-s1-build/proposal.md` — full proposal
- `sddk/e29-s1-build/spec.md` — Given/When/Then acceptance criteria
- `sddk/e29-s1-build/design.md` — technical design + risk register
- `sddk/e29-s1-build/tasks.md` — task breakdown
- `sddk/e29-s3-concurrency/proposal.md` — S3 proposal
- `sddk/e29-s3-concurrency/spec.md` — S3 delta spec
- `sddk/e29-s3-concurrency/design.md` — S3 technical design
- `openspec/specs/ladybug-spike-validation/spec.md` — full 6-stage spike spec
- `docs/adr/ADR-026-ladybugdb-canonical-migration.md` — migration decision

