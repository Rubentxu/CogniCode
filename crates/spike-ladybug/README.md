# E29 S1 Spike — LadybugDB (lbug) v0.19.0 Build Validation

Validates that the `lbug` crate downloads its prebuilt static lib from GitHub
and can create + query a `.lbdb` **file** database using the Kùzu-derived API.

**Important**: `.lbdb` is a single **file** (~49 KB), NOT a directory. This was
verified empirically on 2026-07-31.

## What it proves

1. `lbug 0.19.0` is published on crates.io (pubtime 2026-07-30)
2. The crate's `build.rs` downloads `liblbug-linux-x86_64.tar.gz` from
   `https://github.com/LadybugDB/ladybug/releases/download/v0.19.0/`
   automatically — **no cmake required**
3. The static lib links cleanly with the host toolchain (rustc 1.96.0, gcc 16.1.1)
4. `Database::new` + `Connection::new` + `conn.query` round-trip a single row
5. The `.lbdb` artifact is materialized as a file, not a directory

## How to run

```bash
just spike-ladybug          # build + run example + run test end-to-end
just spike-ladybug-clean    # wipe target/ and .cache/ for fresh download
```

## Build paths

The `lbug` crate has two build paths:

| Path | Trigger | Requires cmake? | Speed |
|------|---------|-----------------|-------|
| **Prebuilt static lib** (default) | automatic on first build | ❌ no | ~30s after download |
| **From source** | `LBUG_BUILD_FROM_SOURCE=1` env var | ✅ yes | 5–15 min |

This spike uses the prebuilt path by default. If the prebuilt download fails,
the crate falls back to cmake — which is **not installed on this host**.
The design explicitly forbids cmake fallback; see Troubleshooting below.

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

### Prebuilt download fails

Symptom: build error mentioning `failed to download liblbug` or curl exit non-zero.

Causes: network unreachable, GitHub rate limit, or the artifact name for this
target triple (`linux-x86_64`) is unavailable.

**On this host**: cmake is NOT installed, so the from-source fallback cannot run.
The build will fail loudly. This is by design — silent fallback would mask the
real issue.

Fix options:
1. Re-run with a fresh network (most likely transient rate limit)
2. Manually download `liblbug-linux-x86_64.tar.gz` from the v0.19.0 release
   and extract to `crates/spike-ladybug/.cache/lbug-prebuilt/version-0.19.0/lib/`
3. Install cmake (requires sudo, not currently possible on this host)

### Database::new returns error

Check disk space (`df -h`) and write permissions on the target directory.
The spike writes to `spike.lbdb` in the current working directory.

### API drift from Kùzu

The first compile is the API proof. If lbug has diverged from Kùzu since the
rename, the corrected API in this README may not match. Re-derive from the
upstream source: `https://github.com/LadybugDB/ladybug-rust/src/lib.rs`.

### Build > 30 min

If the prebuilt download succeeded but the build is still taking > 30 min,
the cmake fallback path was triggered. Investigate `build.rs` env vars:
- `LBUG_BUILD_FROM_SOURCE=1` → from-source
- `LBUG_PRECOMPILED_RUN_ID` → use prebuilt from a CI run
- `LBUG_VERSION` → pin to a specific release

The spike never sets these, so the default prebuilt path should win.

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

- Prebuilt download fails and cmake is unavailable
- lbug API does not match the corrected Kùzu-derived API
- `.lbdb` is created as a directory (would mean lbug v0.20+ changed storage)

## See also

- `sddk/e29-s1-build/proposal.md` — full proposal
- `sddk/e29-s1-build/spec.md` — Given/When/Then acceptance criteria
- `sddk/e29-s1-build/design.md` — technical design + risk register
- `sddk/e29-s1-build/tasks.md` — task breakdown
- `openspec/specs/ladybug-spike-validation/spec.md` — full 6-stage spike spec
- `docs/adr/ADR-026-ladybugdb-canonical-migration.md` — migration decision
