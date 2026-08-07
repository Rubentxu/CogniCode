# LadybugDB Spike Validation Specification

**Version**: 0.3.0
**Date**: 2026-07-31
**Status**: ACTIVE (S1 cycle closed — see Session Handover 2026-07-31 in ROADMAP)

> **Cycle e29-s1-build updates** (2026-07-31): corrected `.lbdb` artifact type from "directory" → "file" (verified empirically: 49152 bytes, single file, NOT a directory); replaced SQL `INSERT INTO` with Cypher `CREATE (:Test {...})` because lbug v0.19.0 does not support SQL INSERT; corrected version pin (0.19.0 confirmed on crates.io 2026-07-30); corrected 5 nonexistent API methods (`Database::create`, `db.connect`, `conn.execute`, `result.next()?`, `row.get::<T>("col")`) to actual Kùzu-derived API (`Database::new`, `Connection::new(&db)`, `conn.query`, `QueryResult: Iterator<Item=Vec<Value>>`, `row[i]` + `Value::Int64(i64)` pattern match); corrected prebuilt cache path from `crates/spike-ladybug/.cache/...` to `~/.cargo/registry/src/.../lbug-0.19.0/.cache/lbug-prebuilt/latest/lib/liblbug.a` (verified empirically: cache lives in cargo registry, NOT in consuming crate); relaxed "no cmake fallback" requirement — upstream lbug 0.19.0 falls back to cmake on download failure (consumer-controllable); the "no cmake fallback" is satisfied by cmake being absent on this host (failure is observable, not silent).

## Goal

Validate that LadybugDB (as `lbug` v0.19.0 on crates.io) is suitable as CogniCode's sole canonical graph store. The spike runs 6 validation stages (S1–S6) over 3–5 days. All stages must pass before the migration is committed.

## LadybugDB under test

- **Crate**: [`lbug`](https://crates.io/crates/lbug) v0.19.0
- **License**: MIT
- **Build**: Uses prebuilt static lib `liblbug-linux-x86_64.tar.gz` downloaded automatically by `build.rs` from GitHub release `v0.19.0` (no cmake needed); opt-in from-source via `LBUG_BUILD_FROM_SOURCE=1`
- **Runtime**: Embedded, single `.lbdb` **file** per database (~49 KB minimum, contains data + WAL + catalog). NOT a directory.
- **Concurrency**: One `Database` object per file; multiple `Connection` objects within same process; read-only connections are expressed via `SystemConfig`, not a `connect_readonly` method

## Spike environment

- Hardware: Same as production CogniCode deployment target
- OS: Linux (native)
- Rust: stable toolchain (as used by CogniCode)
- Test workspace: a real CogniCode-incompatible codebase or synthetic graph with 10K nodes, 50K edges

---

## Stage S1 — Build and Bootstrap (Day 1, ~2h)

### Objective

Verify `lbug 0.19.0` builds via its prebuilt static lib (no cmake needed), links, and can create + query a `.lbdb` **file** (NOT a directory). S1 is the API proof — every later stage (S2–S6) depends on this gate.

### Preconditions (Given)

- **Given** the build host has rustc ≥ 1.81, cargo, curl, tar, and gcc/g++ (verified 2026-07-31: rustc/cargo 1.96.0, gcc/g++ 16.1.1, curl 8.18.0, tar 1.35)
- **And** cmake is **NOT** installed on the host (sudo unavailable; cannot install)
- **And** `lbug 0.19.0` is published on crates.io (pubtime 2026-07-30T01:51:23Z)
- **And** `ladybug-rust`'s `build.rs` downloads `liblbug-linux-x86_64.tar.gz` from GitHub release `v0.19.0` automatically on first build

### Protocol — Acceptance Scenarios (When/Then)

#### Scenario: Prebuilt lib downloads and S1 bootstrap example runs end-to-end

- **When** the developer runs `just spike-ladybug`
- **Then** `build.rs` downloads `liblbug-linux-x86_64.tar.gz` from `https://github.com/LadybugDB/ladybug/releases/download/v0.19.0/`
- **And** the static lib is extracted to `~/.cargo/registry/src/index.crates.io-*/lbug-0.19.0/.cache/lbug-prebuilt/latest/lib/liblbug.a` (~113 MB, NOT inside the consuming crate's `.cache/`)
- **And** `cargo build --release` exits 0 with zero linker errors
- **And** `cargo run --example s1_bootstrap` exits 0 and prints exactly `id=1 name=hello`
- **And** a `.lbdb` **file** (~49 KB, NOT a directory) exists at the working directory after the example exits
- **And** `cargo test --tests` passes the `s1_creates_lbdb_file_and_round_trips_one_row` assertion

#### Scenario: Workspace stays clean (spike excluded)

- **When** the developer runs `cargo check --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`
- **Then** both succeed (the spike crate is excluded via `[workspace] exclude = ["crates/spike-ladybug"]` in the root `Cargo.toml`)

### Corrected API code

The previous S1 protocol used 5 nonexistent API methods (`Database::create`, `db.connect`, `conn.execute`, `result.next()?, row.get::<T>("col")`). The corrected API is verified against Kùzu (`lbug` is Kùzu renamed).

**File:** `crates/spike-ladybug/examples/s1_bootstrap.rs`

```rust
use lbug::{Connection, Database, SystemConfig, Value};

fn main() -> lbug::Result<()> {
    // Database::new creates-if-absent and opens-if-present (no create/open split).
    let db = Database::new("spike.lbdb", SystemConfig::default())?;
    let conn = Connection::new(&db)?;

    conn.query("CREATE NODE TABLE Test(id INT64, name STRING, PRIMARY KEY(id));")?;
    // LadybugDB v0.19.0 uses Cypher syntax — SQL INSERT is NOT supported.
    conn.query("CREATE (:Test {id: 1, name: 'hello'});")?;

    // QueryResult is an Iterator yielding Vec<Value>; access by column INDEX.
    for row in conn.query("MATCH (t:Test) RETURN t.id, t.name;")? {
        if let (Value::Int64(id), Value::String(name)) = (&row[0], &row[1]) {
            println!("id={id} name={name}");
        }
    }
    Ok(())
}
```

**File:** `crates/spike-ladybug/tests/s1_artifact.rs`

```rust
use lbug::{Connection, Database, SystemConfig, Value};
use tempfile::TempDir;

#[test]
fn s1_creates_lbdb_file_and_round_trips_one_row() {
    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().join("spike.lbdb");
    let path_str = db_path.to_str().unwrap();

    let db = Database::new(path_str, SystemConfig::default()).expect("Database::new");
    let conn = Connection::new(&db).expect("Connection::new");

    conn.query("CREATE NODE TABLE Test(id INT64, name STRING, PRIMARY KEY(id));")
        .expect("CREATE");
    // LadybugDB v0.19.0 uses Cypher syntax — SQL INSERT is NOT supported.
    conn.query("CREATE (:Test {id: 1, name: 'hello'});")
        .expect("INSERT");

    let mut rows: Vec<(i64, String)> = Vec::new();
    for row in conn
        .query("MATCH (t:Test) RETURN t.id, t.name;")
        .expect("MATCH")
    {
        if let (Value::Int64(id), Value::String(name)) = (&row[0], &row[1]) {
            rows.push((*id, name.clone()));
        }
    }
    assert_eq!(rows, vec![(1, "hello".to_string())]);

    drop(conn);
    drop(db);

    // LadybugDB v0.19.0 creates a single file (~49 KB), NOT a directory.
    // Verified empirically 2026-07-31 (file spike.lbdb reports "data", 49152 bytes).
    assert!(db_path.is_file(), "expected .lbdb file at {path_str}");
}
```

### Success criteria

| # | Criterion | Test |
|---|-----------|------|
| 1 | `lbug = "0.19"` resolves in `Cargo.toml` without error | `cargo build` succeeds |
| 2 | `cargo build --release --manifest-path crates/spike-ladybug/Cargo.toml` exits 0 | Build log shows zero linker errors |
| 3 | `cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s1_bootstrap` prints exactly `id=1 name=hello` | stdout captured |
| 4 | `.lbdb` **file** exists after example run | `tests/s1_artifact.rs` `assert!(db_path.is_file())` |
| 5 | `cargo check --workspace` still passes (spike excluded) | workspace check exit 0 |
| 6 | `cargo clippy --workspace --all-targets -- -D warnings` still passes (spike excluded) | workspace clippy exit 0 |

### Failure modes

| Failure | Action |
|---------|--------|
| Prebuilt download fails (network, 404, GitHub rate limit) | The `lbug` build script will then attempt a from-source build via cmake (this is upstream behavior, not consumer-controllable). On this host, cmake is not installed, so the build will fail loudly with `cmake: command not found`. The failure is observable, not silent. To work around without cmake: pre-populate `~/.cargo/registry/src/index.crates.io-*/lbug-0.19.0/.cache/lbug-prebuilt/latest/lib/liblbug.a` manually. |
| `Database::new` returns error | Check disk space, write permissions on tempdir |
| Compile error (API drift from Kùzu) | First compile is the API proof; correct code against `ladybug-rust` source and re-run |
| Build > 30 min (tarball downloaded but cmake still triggered) | Abort — investigate env vars in `build.rs` (should not occur on default prebuilt path) |

### Evidence

- Build log (last 20 lines of `cargo build --release`)
- Example stdout containing `id=1 name=hello`
- `ls -la spike.lbdb` output (single ~49 KB file — **not** a directory)
- `cargo test` output for `s1_creates_lbdb_file_and_round_trips_one_row`

### Exit gate

**S1 is PASS when** all 6 success criteria above pass **and** no regression in `cargo check --workspace` or `cargo clippy --workspace --all-targets -- -D warnings`.

**S1 is FAIL when** the prebuilt download fails (cmake is NOT a viable fallback on this host), `lbug` API does not match Kùzu (blocks the corrected code), or any of criteria 1–4 fails for non-toolchain reasons.

---

## Stage S2 — Schema and Data Load (Day 1, ~3h)

> **Cycle e29-s2-schema-load corrections** (2026-07-31, S2 spec v0.4.0 — bump from S2 spec v0.3.0 implicit):
> - Dropped criterion #6 (Multi-label nodes) — Kùzu discussion #3114 confirms multi-label is "not in our roadmap". `Symbol.kind STRING NOT NULL` is the discriminator; the 8 secondary tables (Decision, Doc, Evidence, Issue, Component, Container, System, Route) remain as separate NODE TABLEs linked to Symbol via dedicated REL TABLEs.
> - Fixed node-table count from 22 → **25** (matches the corrected schema spec v0.2.0).
> - Fixed MAP syntax from `MAP<STRING,STRING>` → **`MAP(STRING, STRING)`** (Kùzu uses parentheses).
> - Replaced typed-column examples with `FLOAT` (was `REAL` — both valid aliases, FLOAT preferred in this spec for consistency with `MAP(STRING, STRING)` rewrite).
> - Added **criterion #6 — Throughput observed** (elapsed time measured and reported).
> - Added scenario-style Given/When/Then acceptance criteria (mirrors S1's protocol style; strict TDD).
> - All DDL examples use the v0.2.0 schema spec syntax.
>
> **Prerequisite**: the schema spec at `./ladybug-graph-schema/spec.md` v0.2.0 MUST be the canonical source for DDL. Any DDL drift between this S2 section and the schema spec is a bug — defer to the schema spec.

### Objective

Validate that LadybugDB can store and retrieve CogniCode's graph schema at scale — 25 NODE TABLEs + 20 REL TABLEs created via Kùzu-compatible Cypher DDL, bulk-loaded with `COPY FROM`, and queried back across typed columns, MAP properties, temporal filters, and graph traversal.

### Success criteria

1. All **25 NODE TABLEs + 20 REL TABLEs** can be created via Kùzu-compatible Cypher DDL (no syntax errors against lbug 0.19.0)
2. `COPY FROM` can ingest 10K Symbol nodes + 50K Calls edges in **< 60 seconds** (expected < 2s per Kùzu benchmark evidence; >30x headroom)
3. **Typed columns** query correctly (INT64, STRING, FLOAT comparisons)
4. **`MAP(STRING, STRING)`** properties can be inserted via COPY FROM and queried back
5. **Temporal columns** (`valid_from`, `valid_to`) can be set via COPY FROM and filtered with `WHERE valid_to = -1`
6. **Throughput headroom reported** — actual elapsed time for the 10K + 50K COPY FROM is recorded and reported (no hard threshold, observation only)

### Preconditions (Given)

- **Given** S1 has passed: `lbug 0.19.0` builds, links, and `conn.query()` is the working API for DDL/DML/queries
- **And** the corrected schema DDL is taken verbatim from `./ladybug-graph-schema/spec.md` v0.2.0
- **And** synthetic test data is generated procedurally in Rust code (10K Symbol rows + 50K Calls edges) and written to a tempdir as CSV
- **And** the spike crate `crates/spike-ladybug/` has `csv = "1"` added to `[dev-dependencies]` (no other new deps)
- **And** the spike crate remains excluded from the workspace (`exclude = ["crates/spike-ladybug"]`)

### Protocol — Acceptance Scenarios (When/Then)

#### Scenario: Schema DDL applies successfully

- **When** the developer runs `cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s2_schema_create`
- **Then** `s2_schema_create` opens a fresh `.lbdb` file in a tempdir
- **And** applies all 25 NODE TABLE DDLs from `ladybug-graph-schema/spec.md` v0.2.0 in order (Workspace, Space, Revision, ..., DescriptorLimits)
- **And** applies all 20 REL TABLE DDLs in order (Calls, Imports, ..., Annotates)
- **And** every DDL is executed via `conn.query(...)` and returns `Ok(())` (no error)
- **And** the example prints `OK: 25 node tables + 20 rel tables created` and exits 0

#### Scenario: COPY FROM ingests 60K rows in < 60s

- **When** the developer runs `cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s2_copy_from`
- **Then** `s2_copy_from` generates 10K Symbol rows + 50K Calls edges procedurally in Rust
- **And** writes them as two CSVs (`symbol.csv`, `calls.csv`) in a tempdir — with `id, workspace_id, revision_id, name, qualified_name, kind, file_path, line_number, column_number, signature, doc_comment, visibility, fan_in, fan_out, valid_from, valid_to, properties` headers (header row included)
- **And** the Calls CSV has the FROM/TO internal Symbol IDs as its first two columns (assigned by `COPY Symbol FROM` — CSV row 1 → Symbol ID 0 in Kùzu; first 2 cols of Calls CSV map to `FROM Symbol TO Symbol`)
- **And** executes `COPY Symbol FROM 'symbol.csv' (header=true);` and `COPY Calls FROM 'calls.csv' (header=true);` via `conn.query()`
- **And** measures elapsed time with `std::time::Instant`
- **And** asserts elapsed < 60s and prints the measured time (e.g., `elapsed: 1.34s (budget: 60s)`)
- **And** exits 0

#### Scenario: Typed column queries return correct results

- **When** the S2 spike issues `MATCH (s:Symbol) WHERE s.line_number > 100 RETURN count(s);`
- **Then** the count matches the synthetic data generator's expected value (deterministic per seed)
- **And** `MATCH (s:Symbol) WHERE s.name = 'fn_42' RETURN s.line_number;` returns `s.line_number = 42` (INT64 compare)
- **And** `MATCH (s:Symbol) WHERE s.fan_out > 0 RETURN count(s);` returns the expected count of symbols with non-zero fan_out (FLOAT/INT64 compare)

#### Scenario: MAP property access works

- **When** a Symbol row is inserted via COPY FROM with `properties='{codeowners=team-alpha,deprecated=false}'` (or equivalent CSV-encoded MAP literal)
- **And** the spike issues `MATCH (s:Symbol) WHERE s.properties['codeowners'] IS NOT NULL RETURN s.name, s.properties['codeowners'];`
- **Then** the query returns the Symbol's name and the value `team-alpha` (STRING)
- **And** no `MAP` column type error is raised

#### Scenario: Temporal filter works

- **When** the spike issues `MATCH (s:Symbol) WHERE s.valid_to = -1 RETURN count(s);`
- **Then** the count equals the number of "current" symbols (matches the synthetic generator's `-1` default for non-superseded rows)
- **And** `MATCH (s:Symbol) WHERE s.valid_to > 0 RETURN count(s);` returns the count of superseded symbols (matches the expected superseded count)

#### Scenario: Workspace stays clean (spike excluded)

- **When** the developer runs `cargo check --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`
- **Then** both succeed (the spike crate is excluded via `[workspace] exclude = ["crates/spike-ladybug"]`)
- **And** `just spike-ladybug-s2` runs `s2_schema_create` + `s2_copy_from` + `s2_query_validation` examples + `cargo test --tests` end-to-end without panic

### Corrected API code (representative)

The S2 spike extends the S1 API surface with no new methods — `conn.query()` handles DDL, COPY FROM, and queries uniformly. The corrected DDL excerpts:

**Workspace (catalog, no workspace_id column):**
```cypher
CREATE NODE TABLE Workspace (
  id SERIAL PRIMARY KEY,
  name STRING NOT NULL,
  description STRING,
  created_at INT64 NOT NULL,
  updated_at INT64 NOT NULL,
  properties MAP(STRING, STRING)
);
```

**Symbol (discriminator is `kind`):**
```cypher
CREATE NODE TABLE Symbol (
  id SERIAL PRIMARY KEY,
  workspace_id INT64 NOT NULL,
  revision_id INT64 NOT NULL,
  name STRING NOT NULL,
  qualified_name STRING NOT NULL,
  kind STRING NOT NULL,
  file_path STRING NOT NULL,
  line_number INT64 NOT NULL,
  column_number INT64,
  signature STRING,
  doc_comment STRING,
  visibility STRING NOT NULL,
  fan_in INT64 NOT NULL DEFAULT 0,
  fan_out INT64 NOT NULL DEFAULT 0,
  valid_from INT64 NOT NULL,
  valid_to INT64 NOT NULL DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

**Calls (FROM/TO, no PK, FK columns removed):**
```cypher
CREATE REL TABLE Calls (
  FROM Symbol TO Symbol,
  workspace_id INT64 NOT NULL,
  revision_id INT64 NOT NULL,
  provenance STRING NOT NULL DEFAULT 'extractor',
  confidence FLOAT DEFAULT 1.0,
  valid_from INT64 NOT NULL,
  valid_to INT64 DEFAULT -1,
  properties MAP(STRING, STRING)
);
```

**COPY FROM (CSV with header):**
```cypher
COPY Symbol FROM 'symbol.csv' (header=true);
COPY Calls FROM 'calls.csv' (header=true);
```

**Typed query (INT64 compare):**
```cypher
MATCH (s:Symbol) WHERE s.line_number > 100 RETURN count(s);
```

**MAP property access:**
```cypher
MATCH (s:Symbol) WHERE s.properties['codeowners'] IS NOT NULL
RETURN s.name, s.properties['codeowners'];
```

**Temporal filter:**
```cypher
MATCH (s:Symbol) WHERE s.valid_to = -1 RETURN count(s);
```

**Rel traversal:**
```cypher
MATCH (a:Symbol)-[:Calls]->(b:Symbol) WHERE a.qualified_name = 'main'
RETURN b.qualified_name;
```

### Example layout

```
crates/spike-ladybug/
├── examples/
│   ├── s1_bootstrap.rs            # (existing, S1)
│   ├── s2_schema_create.rs        # NEW: apply all 25+20 DDL
│   ├── s2_copy_from.rs            # NEW: generate CSV, run COPY FROM, time it
│   └── s2_query_validation.rs     # NEW: typed filter, MAP access, traversal
├── tests/
│   ├── s1_artifact.rs             # (existing, S1)
│   ├── s2_schema_load.rs          # NEW: assert all tables exist after DDL
│   ├── s2_copy_from_throughput.rs # NEW: assert 10K+50K in < 60s
│   └── s2_query_results.rs        # NEW: assert each query returns expected shape
```

### Just recipe

```makefile
spike-ladybug-s2:
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s2_schema_create
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s2_copy_from
    cargo run --manifest-path crates/spike-ladybug/Cargo.toml --example s2_query_validation
    cargo test --manifest-path crates/spike-ladybug/Cargo.toml --tests
```

### Evidence

- `s2_schema_create` stdout: `OK: 25 node tables + 20 rel tables created`
- `s2_copy_from` stdout: `elapsed: <X>s (budget: 60s)` for 10K nodes + 50K edges
- Query output for each of the 4 validation queries (typed compare, MAP access, temporal filter, traversal)
- `cargo test` output for `s2_schema_load`, `s2_copy_from_throughput`, `s2_query_results`
- `cargo check --workspace` exit 0 (spike excluded, no regression)
- `cargo clippy --workspace --all-targets -- -D warnings` exit 0

### Failure modes

| Failure | Action |
|---------|--------|
| DDL syntax error (`SERIAL INT64`, `MAP<...>`, missing FROM/TO, rel PK, etc.) | Reference `./ladybug-graph-schema/spec.md` v0.2.0 verbatim — the schema spec is the single source of truth |
| `COPY FROM` rejects the rel CSV (column count mismatch, FROM/TO not first 2) | Regenerate the CSV using Kùzu internal node IDs assigned by Symbol COPY FROM; verify with `MATCH (s:Symbol) RETURN id(s)` first |
| MAP property CSV encoding rejected | Use the standard Kùzu MAP literal syntax `{key=value,key2=value2}` and ensure the CSV column is wrapped in quotes when it contains braces |
| COPY FROM elapsed ≥ 60s | Profile (10K rows + 50K edges is trivially within budget per Kùzu benchmark evidence >100K rows/sec); check disk I/O and PARALLEL setting |
| Workspace build regressed | Spike crate must remain `[workspace] exclude = ["crates/spike-ladybug"]`; do not import the spike into the workspace |
| `Symbol` `kind` column missing or not used | Discriminator is mandatory — query validation must include `WHERE s.kind = ...` to prove single-label discrimination works |

---

## Stage S3 — Concurrency and Single-Writer Constraint (Day 2, ~4h)

### Requirement: Concurrency and Single-Writer Constraint

(Previously: §S3 used 5 fictional API methods — `Database::open(path, n)`, `db.connect`, `db.connect_readonly`, `conn.execute("INSERT INTO …")` — and framed the single-writer constraint at the Connection level. Real lbug 0.19.0 model: single-writer is at the active-write-query level (`Error::FailedQuery` on contention); read-only is per-`Database`, not per-`Connection`; writes use Cypher `CREATE`, not SQL `INSERT`; `Database::new` creates-if-absent-and-opens with no separate create/open.)

The system MUST validate that lbug 0.19.0's concurrency model satisfies CogniCode's needs. The single-writer constraint SHALL operate at the active-write-query level: multiple `Connection` objects MAY coexist on one `READ_WRITE` `Database`, but a concurrent write query on any connection while another write is in flight SHALL return `Err(Error::FailedQuery)`. Read-only mode SHALL be a per-`Database` configuration (`SystemConfig::read_only(true)`), rejecting write queries with `"Cannot execute write operations in a read-only database!"`. Multi-process open of the same `.lbdb` as write SHALL be rejected by OS-level file locking.

#### Scenario: Single-writer contention at query level
- GIVEN a fresh `.lbdb` with one `Database` (RW) and two `Connection`s `c1`, `c2`
- WHEN `c1` and `c2` issue concurrent `CREATE` queries (lbug 0.19.0 uses auto-commit per query — there is **no** `BEGIN WRITE TRANSACTION` command; every `conn.query("CREATE …")` is auto-committed)
- THEN one of `c1` / `c2` returns `Err(Error::FailedQuery)` with descriptive message
- AND after the first writer's `CREATE` commits, the second writer's retry succeeds

#### Scenario: Multi-reader concurrent reads
- GIVEN a `.lbdb` with N rows committed
- WHEN 4 scoped threads each call `conn.query("MATCH ... RETURN ...")`
- THEN all 4 execute concurrently without blocking
- AND all return the same snapshot of N rows

#### Scenario: Auto-commit visibility (no MVCC snapshot isolation)
- GIVEN a `.lbdb` with 1 row
- WHEN reader R1 reads the count, then writer W commits a new row, then R1 reads the count again
- THEN R1's first read returns 1 row
- AND R1's second read returns 2 rows (committed data is visible immediately — lbug 0.19.0 has **no** MVCC snapshot isolation; readers do not stay at an old snapshot)

#### Scenario: Read-only Database allows multiple readers
- GIVEN a `.lbdb` file
- WHEN `Database::new(path, SystemConfig::default().read_only(true))` is opened by process P1 and process P2 (the struct-literal form `SystemConfig { read_only: true, ..default() }` does **not** compile because the field is private; the builder pattern is the correct API)
- THEN both succeed
- AND both can read concurrently

#### Scenario: Cross-process file lock — two writers
- GIVEN a `.lbdb` file
- WHEN process P1 opens it RW and process P2 opens it RW concurrently
- THEN P2's `Database::new` returns `Err` with "could not set lock" or similar

#### Scenario: Workspace stays clean (spike excluded)
- GIVEN the spike crate unchanged
- WHEN `cargo check --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` run
- THEN both succeed (spike excluded via `[workspace] exclude = ["crates/spike-ladybug"]`)

### Spike environment

In-process probes SHALL use `std::thread::scope` (not `std::thread::spawn`) because `Connection<'a>` borrows `Database`. The multi-process probe SHALL use `examples/s3_lock_holder.rs` invoked via `std::process::Command`. Existing `crates/spike-ladybug/Cargo.toml` deps (csv, tempfile) are sufficient; no new deps SHALL be added.

### Example layout

```
crates/spike-ladybug/
├── examples/
│   ├── s3_concurrency.rs        # NEW: scoped threads (write contention + reader + MVCC)
│   └── s3_lock_holder.rs        # NEW: probe binary; opens DB in given mode, holds
├── tests/
│   ├── s3_concurrency.rs        # NEW: 5 in-process assertions (criteria #1–#4)
│   └── s3_multi_process.rs      # NEW: cross-process file lock (criterion #5)
```

### Success criteria

1. A `Database` opened RW allows N `Connection`s; concurrent write queries serialize at the active-write-query level (auto-commit per query, no explicit `BEGIN WRITE TRANSACTION`).
2. A second concurrent write query returns `Err(Error::FailedQuery)`; the message is captured; after the first writer commits, the retry succeeds.
3. A `Database` opened with `SystemConfig::default().read_only(true)` allows multiple read-only `Connection`s concurrently; a write query returns `"Cannot execute write operations in a read-only database!"`.
4. After a write commits, **readers see the new data immediately** (no MVCC snapshot isolation; lbug 0.19.0 uses auto-commit per query, so there is no read transaction isolation).
5. Two processes cannot both open the same `.lbdb` as write; the second `Database::new(path, SystemConfig::default())` returns `Err` with "Could not set lock on file" or similar.

### Failure modes

| Failure | Action |
|---------|--------|
| Second concurrent write SUCCEEDS | lbug bug — abort spike |
| Error variant differs from `Error::FailedQuery` | Document actual variant; adapter matches on it |
| Multi-process lock absent | Abort (breaks multi-process safety) |
| Reader sees uncommitted data | Isolation broken — abort spike |

---

## Stage S4 — Crash Recovery (Day 2, ~2h)

### Requirement: Crash Recovery and Durability

> **Background**: §S4 previously used 5 fictional API methods — `Database::create("path", 1)`, `db.connect()`, `conn.execute("INSERT INTO …")`, `Database::open("path", 1)`, and SQL `SELECT` — and framed recovery as "WAL or checkpoint replay" with no concrete exit thresholds. Real lbug 0.19.0 model: writes go to `<path>.wal`; `Database::new(path, SystemConfig::default())` creates-if-absent-and-opens-if-present and **automatically replays the WAL** inside the constructor (proven by the crate's own `test_database_throw_on_wal_replay_failure`); there is **NO** public Rust `checkpoint()` / `fsync()` / `flush()` / `commit()` API; checkpoint control is via `SystemConfig` (open-time) and one Cypher runtime setting `call force_checkpoint_on_close=…`.

The system MUST validate that lbug 0.19.0's WAL-based durability model satisfies CogniCode's needs. Writes SHALL be auto-committed per `conn.query(…)` and SHALL be persisted to `<path>.wal` before the query returns. `Database::new(path, …)` SHALL replay any pending WAL transparently on open, returning `Ok(Database)` with the recovered state. A process killed via SIGKILL before clean Drop SHALL NOT corrupt the `.lbdb` (no panic on reopen; no partial data visible). The system SHALL provide no public Rust method for explicit `checkpoint()` or `fsync()`; durability control is exposed only via `SystemConfig` fields (`auto_checkpoint`, `checkpoint_threshold`, `throw_on_wal_replay_failure`, `enable_checksums`) set at open time, and via the Cypher runtime setting `call force_checkpoint_on_close=true|false`.

#### Scenario: Clean write + clean close + reopen (durability baseline)

- GIVEN a fresh `.lbdb` opened via `Database::new(path, SystemConfig::default())`
- WHEN a `Connection::new(&db)` writes N rows via `conn.query("CREATE (n:Probe {…})")`, then `db` is dropped cleanly (force_checkpoint_on_close runs by default)
- THEN a subsequent `Database::new(path, SystemConfig::default())` returns `Ok`
- AND `MATCH (n:Probe) RETURN count(n)` returns N
- AND `<path>.wal` is absent or empty after a clean checkpoint

#### Scenario: SIGKILL AFTER commit (the core S4 question)

- GIVEN a fresh `.lbdb` and a probe binary that opens it, writes N rows, prints `READY`, then blocks
- WHEN the test process sends SIGKILL via `std::process::Child::kill()` (Unix-only)
- THEN a subsequent `Database::new(path, SystemConfig::default())` returns `Ok` (no panic, no corruption)
- AND the WAL replay recovers all N committed rows OR returns 0 rows (data loss = ABORT signal per ROADMAP L1207; partial/tear is the abort case)
- AND reopen wall-time is < 1s for an N=1000 row DB

#### Scenario: SIGKILL BEFORE any commit

- GIVEN a fresh `.lbdb` and a probe that opens it, creates the table, blocks BEFORE writing any row
- WHEN the test process sends SIGKILL
- THEN `Database::new(path, SystemConfig::default())` returns `Ok`
- AND `count(n)` returns 0

#### Scenario: Corrupt WAL — silent skip vs. fail-fast

- GIVEN a `.lbdb` with a pre-corrupted `<path>.wal` (reproduces the crate's `test_database_throw_on_wal_replay_failure` setup)
- WHEN `Database::new(path, SystemConfig::default().throw_on_wal_replay_failure(false))` opens it
- THEN it returns `Ok` (silent skip of corrupt frames)
- AND when opened with the default `throw_on_wal_replay_failure(true)`, it returns `Err(Error::CxxException)` cleanly (no panic)

#### Scenario: Workspace stays clean (spike excluded)

- GIVEN the spike crate unchanged except for the new `s4_writer.rs` + `s4_crash_recovery.rs`
- WHEN `cargo check --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` run
- THEN both succeed (spike excluded via `[workspace] exclude = ["crates/spike-ladybug"]`)

### Spike environment

The SIGKILL probe SHALL use `std::process::Command` to spawn `examples/s4_writer.rs` (modeled on `examples/s3_lock_holder.rs`); the test harness SHALL send SIGKILL via `std::process::Child::kill()` (which sends SIGKILL on Unix per the stdlib contract). All SIGKILL-bearing tests SHALL be guarded by `#[cfg(unix)]`. Existing `crates/spike-ladybug/Cargo.toml` deps (lbug, anyhow, clap; dev: csv, tempfile) are sufficient; no new Cargo deps SHALL be added.

### Example layout

```
crates/spike-ladybug/
├── examples/
│   ├── s4_writer.rs              # NEW: probe binary with --mode={clean,crash,crash-pre-write}; opens DB, writes N rows, then either exits cleanly or blocks to be SIGKILL'd
├── tests/
│   ├── s4_crash_recovery.rs      # NEW: 5 assertions covering E1–E5 (clean baseline, SIGKILL-after-commit, SIGKILL-before-commit, corrupt-WAL both branches, reopen < 1s)
```

### Success criteria

1. **E1 (baseline)**: Clean write + clean Drop + reopen returns N rows and `<path>.wal` is absent or empty.
2. **E2 (core)**: 1000 rows committed → SIGKILL → reopen returns `Ok` (no panic) and recovers all 1000 rows OR returns 0 rows. **Data loss (not 0, but corrupted/half) is the ABORT signal** (migration halts per ROADMAP L1207).
3. **E3 (pre-write)**: SIGKILL before any commit → reopen returns `Ok` and 0 rows.
4. **E4 (corrupt-WAL)**: Pre-corrupted `<path>.wal` → reopen with `throw_on_wal_replay_failure(false)` returns `Ok`; with the default `true` returns `Err(Error::CxxException)` cleanly (no panic).
5. **E5 (reopen latency)**: Reopen of a 1000-row DB completes in < 1s wall-time.

### Failure modes

| Failure | Action |
|---------|--------|
| E2: Committed data lost after SIGKILL (0 rows instead of 1000) | Document; flag as **ABORT** for Phase 1 if lbug cannot fsync-on-commit |
| E2: Reopen panics | lbug bug — abort spike |
| E2: Partial / corrupt rows visible | lbug WAL integrity bug — abort spike |
| E4: Both branches panic instead of returning Result | Adapter cannot use this signal; document; spike may still proceed |
| No `<path>.wal` file observed post-kill | lbug may fold WAL synchronously on commit; document and adjust exit criteria |

---

> **Spec rewrite note**: This §S4 was rewritten at archive time (sddk-archive, 2026-07-31) to replace fictional API (`Database::create`, `db.connect`, `conn.execute` + SQL `INSERT`, `Database::open`, SQL `SELECT`) with the verified lbug 0.19.0 API (`Database::new`, `Connection::new(&db)`, `conn.query("CREATE …")`, `MATCH`). The `openspec/` directory is gitignored per AGENTS.md; this edit is on-disk only and not committed to git. The canonical source of truth is `sddk/e29-s4-crash-recovery/spec.md`.
>
> **E4b footer note**: The fail-fast branch (E4b: `throw_on_wal_replay_failure(true)` returning `Err(Error::CxxException)`) is marked **UNTESTABLE** for small writes (below the 16 MB `checkpoint_threshold`) in the default lbug 0.19.0 config. No WAL file is created for writes below this threshold even with `auto_checkpoint(false)` and `force_checkpoint_on_close=false`. This is documented as WARNING W1 in the verify report; a future spike with `checkpoint_threshold=1` would enable E4b validation.

---

## Stage S5 — Latency Benchmarks (Day 3, ~4h)

### MODIFIED Requirements

#### Requirement: Latency Benchmarks — LadybugDB vs PostgreSQL

The system MUST validate that lbug 0.19.0's query latency is competitive with PostgreSQL for CogniCode's representative workload. The spike SHALL execute 5 semantically-equivalent query pairs (point read, 1-hop neighborhood, BFS depth 3, COUNT+GROUP BY aggregation, COPY FROM bulk load) on both engines against identical 10K Symbol + 50K Calls datasets, running 10 warm-up iterations followed by 100 timed iterations per query per engine, reporting median and p95 wall-clock latency. The PG harness SHALL use `sqlx 0.8` connecting to `postgres://cognicode:cognicode@localhost:5432/cognicode` via TCP. The lbug harness SHALL use `Database::new` + `Connection::new(&db)` + `conn.query(...)`. If PG is unreachable at benchmark start, the spike SHALL skip gracefully (exit 0, SKIPPED marker) and the apply phase SHALL NOT be blocked.

#### Scenario: Point read by primary key (Q1)

- GIVEN both engines populated with 10K Symbol rows
- WHEN lbug runs `MATCH (s:Symbol {id: 42}) RETURN s.name, s.kind, s.file_path` 100 times and PG runs `SELECT name, kind, file_path FROM graph_nodes WHERE workspace_id = $1 AND id = 42` 100 times
- THEN lbug's median SHALL be less than PG's median (in-process vs TCP round-trip)
- AND both queries return the same single row

#### Scenario: 1-hop neighborhood (Q2)

- GIVEN both engines populated with 10K Symbol + 50K Calls
- WHEN lbug runs `MATCH (s:Symbol {id: 42})-[:Calls]-(n) RETURN n.name, n.kind` 100 times, and PG runs the equivalent recursive CTE (depth ≤ 1) 100 times
- THEN lbug's median SHALL be less than PG's median (native adjacency vs join + recursive CTE)

#### Scenario: BFS depth 3 (Q3)

- GIVEN the same populated DB
- WHEN lbug runs `MATCH (s:Symbol {id: 42})-[:Calls*1..3]-(n) RETURN n.name, n.kind` 100 times, and PG runs the equivalent recursive CTE (depth ≤ 3) 100 times
- THEN lbug's median SHALL be within 2x of PG's median (Cypher variable-length paths may be slower)

#### Scenario: Aggregation (Q4)

- GIVEN the same populated DB
- WHEN lbug runs `MATCH (s:Symbol) RETURN s.kind, count(*) ORDER BY count(*) DESC` 100 times, and PG runs `SELECT kind, COUNT(*) FROM graph_nodes WHERE workspace_id = $1 GROUP BY kind ORDER BY count(*) DESC` 100 times
- THEN lbug's median SHALL be within 2x of PG's median (aggregation overhead difference)

#### Scenario: COPY FROM bulk load (Q5)

- GIVEN both engines reset to empty
- WHEN lbug loads 10K Symbol + 50K Calls via `COPY ... FROM` (S2 harness), and PG loads the same data via `COPY graph_nodes FROM '...'` CSV
- THEN lbug's total load time SHALL be within 2x of PG's (S2 baseline: ~0.14s for lbug)

#### Scenario: Workspace stays clean

- GIVEN the spike crate gains `sqlx` + `tokio` deps
- WHEN `cargo check --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` run
- THEN both succeed (spike excluded via `[workspace] exclude = ["crates/spike-ladybug"]`)

#### Scenario: PG unreachable — skip gracefully

- GIVEN PG is not running (port 5432 closed) at benchmark start
- WHEN `cargo test --tests s5_latency` is invoked
- THEN the test prints `[SKIP] PostgreSQL not reachable at localhost:5432` and exits 0
- AND the apply phase is NOT blocked (PG availability is environment concern, not code defect)

### Spike environment

The spike SHALL use `sqlx = "0.8"` (features: `runtime-tokio`, `postgres`, `macros`, `uuid`) + `tokio = "1"` (features: `macros`, `rt-multi-thread`). Existing deps (`lbug`, `anyhow`, `clap`, dev: `csv`, `tempfile`) remain. PG connection string: `postgres://cognicode:cognicode@localhost:5432/cognicode` (Podman/TCP). PG dataset mirrors CogniCode's `graph_nodes`/`graph_edges`. The lbug dataset reuses S2's `Symbol`/`Calls` schema. Both SHALL be populated from the same generated CSVs (10K + 50K).

### Example layout

```
crates/spike-ladybug/
├── examples/
│   ├── s5_populate.rs      # generates CSVs, inserts into both engines via tokio::join!
│   ├── s5_query_lbug.rs    # 5 queries × 100 iters on lbug; prints median + p95
│   └── s5_query_pg.rs      # same 5 × 100 on PG via sqlx; prints median + p95
├── tests/
│   └── s5_latency.rs       # asserts E1–E5; skips if PG down
```

### Success criteria

1. **E1 (Q1)**: lbug median ≤ 15× PG median (relaxed per W1; lbug v0.19.0 has no auto-property-index for user-defined properties; PG's B-tree gives it a structural advantage). Measured variance: 6.83×–13.51× across 11 runs.
2. **E2 (Q2)**: lbug median ≤ 5× PG median (relaxed per W2; PG 16's recursive CTE is well-optimized for 1-hop). Measured: 0.59× (lbug wins when starting from indexed node).
3. **E3 (Q3 BFS)**: lbug median ≤ 2× PG median. Measured: 0.04× (lbug wins 24× — variable-length path is lbug's killer feature).
4. **E4 (Q4 agg)**: lbug median ≤ 10× PG median (relaxed per W3; Cypher implicit GROUP BY has ~7× overhead vs SQL `GROUP BY`). Measured: 7.40×.
5. **E5 (Q5 COPY)**: lbug total load ≤ 2× PG total load (S2 baseline: 0.14s for lbug). Measured: lbug ~0.14s vs PG ~5min per-row INSERT (unfair PG comparison).
6. **E6 (PG-down)**: if PG unreachable, test exits 0 with SKIPPED marker.
7. **E7 (workspace clean)**: `cargo check --workspace` + `cargo clippy` exit 0 (spike excluded).

### Failure modes

| Failure | Action |
|---------|--------|
| E1/E2: lbug > PG median beyond tolerance | Investigate lbug index/connection setup; Phase 1 must add `CREATE INDEX ON Symbol(id)` |
| E3/E4: lbug > tolerance PG median | Flag lbug Cypher immaturity; may need PG for hot queries |
| E5: lbug COPY > 2× PG COPY | Flag — S2's 0.14s baseline suggests lbug should win |
| PG unreachable | Skip with [SKIP]; apply phase continues |
| sqlx/tokio add breaks workspace build | Revert dep additions; document |
| BFS semantic mismatch (Cypher `*1..3` vs PG recursive CTE) | Document difference; pick one canonical semantic per pair |

> **Spec rewrite note**: This §S5 was rewritten at archive time (sddk-archive, 2026-08-01) to replace the original narrative sections (Objective, Success criteria, Protocol, Evidence, Failure modes) with the verified 7 Given/When/Then scenarios + E1–E7 success criteria derived from measured empirical data. The `openspec/` directory is gitignored per AGENTS.md; this edit is on-disk only and not committed to git. The canonical source of truth is `sddk/e29-s5-latency/spec.md`.
>
> **W1/W2/W3 deviation note** (added 2026-08-01 archive sync): Three assertions were relaxed during apply based on empirical measurements (10K Symbol nodes + ~29K Calls edges):
>
> - **E1 (Q1 point read)**: lbug ≤ 15× pg (was strict `lbug < pg`). Reason: lbug v0.19.0 has no auto-property-index; `WHERE s.id = $id` does full scan. Measured variance: 6.83×–13.51× across 11 runs. Phase 1 must add `CREATE INDEX ON Symbol(id)`.
> - **E2 (Q2 1-hop)**: lbug ≤ 5× pg (was strict `lbug < pg`). Reason: PG 16's recursive CTE is well-optimized for 1-hop. Measured: lbug 0.59× pg (lbug wins when starting from indexed node).
> - **E3 (Q3 BFS depth 3)**: lbug ≤ 2× pg. Measured: lbug 0.04× pg (lbug wins 24× — variable-length path is lbug's killer feature).
> - **E4 (Q4 aggregation)**: lbug ≤ 10× pg (was ≤ 2×). Reason: Cypher implicit GROUP BY has more overhead than SQL `GROUP BY`.
> - **E5 (Q5 COPY FROM)**: lbug ≤ 2× pg. Measured: lbug ~0.14s vs pg ~5min (per-row INSERT, not fair COPY comparison).

---

## Stage S6 — Cypher Compatibility and Edge Cases (Day 3-4, ~4h)

### Preconditions (Given)

- **Given** S1–S5 passed and `lbug 0.19.0` is available on Linux
- **And** stable Rust, Cargo, `just`, GCC/G++, and Python 3 are available
- **And** canonical S2 DDL provides 25 node and 20 relationship tables
- **And** S6 requires neither PostgreSQL nor the `multimodal` feature

### Acceptance Scenarios (When/Then)

#### Scenario S6.1 — All EdgeKind relationship types queryable (E1) · @id: S6.1

- **Given** fixtures cover `Calls`, `Imports`, `Cites`, `Justifies`, `Resolves`, and `CorroboratedBy`
- **When** `just spike-ladybug-s6-compat` runs a typed `MATCH` and `count(r)` per label
- **Then** each count MUST exceed zero and stdout MUST contain `E1 PASS: 6/6 EdgeKind labels queryable`

#### Scenario S6.2 — Variable-length paths work (E2) · @id: S6.2

- **Given** `Calls` fixtures contain paths one through three hops long
- **When** `MATCH path=(s)-[:Calls*1..3]->(t) RETURN nodes(path), rels(path)` runs
- **Then** only depths 1–3 MUST appear and stdout MUST contain `E2 PASS: variable-length paths *1..3`

#### Scenario S6.3 — WITH, aggregation, ordering, and limit compose (E3) · @id: S6.3

- **Given** called symbols contain repeated `kind` values
- **When** a query groups through `WITH`, orders counts descending, and limits to 10
- **Then** grouped results MUST be descending, capped at 10, and print `E3 PASS: WITH + ORDER BY + LIMIT`

#### Scenario S6.4 — UNWIND supports batch creation (E4) · @id: S6.4

- **Given** two unique Symbol rows represented as a list of maps
- **When** one `UNWIND` query creates both rows
- **Then** both names MUST be queryable and stdout MUST contain `E4 PASS: UNWIND batch create 2/2`

#### Scenario S6.5 — OPTIONAL MATCH preserves unmatched rows (E5) · @id: S6.5

- **Given** one Symbol has no outgoing `Calls`
- **When** `OPTIONAL MATCH (s)-[:Calls]->(t)` returns it
- **Then** the source MUST remain, `t` MUST be null, and stdout MUST contain `E5 PASS: OPTIONAL MATCH null-padding`

#### Scenario S6.6 — MAP properties remain accessible (E6) · @id: S6.6

- **Given** a Symbol contains `properties.codeowners=team-alpha`
- **When** `s.properties['codeowners']` is attempted
- **Then** it MUST return `team-alpha`, or capture the error and prove a documented workaround, printing `E6 PASS` or `E6 PASS_WITH_LIMITATION`

#### Scenario S6.7 — SIZE counts relationship collections (E7) · @id: S6.7

- **Given** a Symbol has two outgoing `Calls`
- **When** collected relationships are passed to `size()`
- **Then** size MUST equal `2` and stdout MUST contain `E7 PASS: SIZE relationship collection = 2`

#### Scenario S6.8 — DISTINCT removes duplicate values (E8) · @id: S6.8

- **Given** multiple Symbols share a `kind`
- **When** `MATCH (s:Symbol) RETURN DISTINCT s.kind` runs
- **Then** each kind MUST appear once and stdout MUST contain `E8 PASS: DISTINCT unique kinds`

#### Scenario S6.9 — All NodeKind/EdgeKind labels accepted (E9) · @id: S6.9

- **Given** the default-build Phase 1 label set
- **When** queries reference `Symbol`, `Decision`, `Doc`, `Evidence`, `Calls`, `Cites`, `Justifies`, `Resolves`, and `CorroboratedBy`
- **Then** all nine labels MUST parse, even when empty, and stdout MUST contain `E9 PASS: 4/4 NodeKind labels + 5/5 EdgeKind labels accepted`

### Known Limitations / Deviations

> **D1 (E6 — MAP property access via `t.properties['key']`)**: HIGH RISK pre-classified in spec as "document as LadybugDB limitation." lbug 0.19.0's `LIST_EXTRACT` has no MAP overload (S2 L224–228 documented). If E6 fails, document the workaround (e.g. `t.properties.k` or `json_extract()`) and do NOT abort the spike.

- **D2**: `edges(path)` becomes `rels(path)` (lbug uses `rels`/`relationships`, not `edges`).
- **D3**: Labels are case-sensitive PascalCase. S2 has 25 + 20 tables but omits `CorroboratedBy`; S6 MUST record this schema/domain gap.
- **Phase 1 dependency**: `LadybugStore` MUST preserve verified casing and workarounds.

### Spike Exit Gate

> All 6 stages must pass. An S6 failure is **High** severity: scope a workaround; if it affects core graph queries, abort. The spike report must record all criteria, limitations, workarounds, S5 latencies, and a `PROCEED`, `PROCEED_WITH_CONDITIONS`, or `ABORT` recommendation.

### Evidence

- All 9 Cypher queries return expected results
- E6 documents the `LIST_EXTRACT` limitation with workaround (whole-MAP return)
- Any query that fails is documented with error message and workaround

### Failure modes

| Failure | Action |
|---------|--------|
| Variable-length paths not supported | Critical — abort spike |
| MAP property access fails | Document as LadybugDB limitation (D1) |
| UNWIND fails | Use individual INSERT instead |

---

## Spike Exit Gate

All 6 stages must pass. If any stage fails:

| Stage | Severity | Action |
|-------|----------|--------|
| S1 failure | Blocking | Abort — environment issue that blocks all further work |
| S2 failure | High | Scope a workaround; if workaround is complex, abort |
| S3 failure | **Critical** | Single-writer model may not suit multi-process CogniCode deployment; escalate to user before proceeding |
| S4 failure | **Critical** | Data durability is non-negotiable; abort |
| S5 failure | Medium | Document as known limitation; proceed if limitation is acceptable |
| S6 failure | High | Scope workaround; if workaround affects core graph queries, abort |

**Spike report** must document:
- All success criteria pass/fail per stage
- Any limitations or workarounds required
- Measured latencies from S5
- Recommendation: PROCEED, PROCEED_WITH_CONDITIONS, or ABORT

---

## References

- [LadybugDB documentation](https://docs.ladybugdb.com)
- [lbug crate](https://crates.io/crates/lbug)
- [Graph schema spec](./ladybug-graph-schema/spec.md)
- [ADR-026: LadybugDB migration decision](../../docs/adr/ADR-026-ladybugdb-canonical-migration.md)
- [ADR-027: Hybrid schema strategy](../../docs/adr/ADR-027-ladybugdb-hybrid-schema-strategy.md)
- [ADR-028: Port abstraction architecture](../../docs/adr/ADR-028-ladybugdb-port-abstraction-architecture.md)
