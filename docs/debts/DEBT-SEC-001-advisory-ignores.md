# DEBT-SEC-001 — Advisory ignore register for `deny.toml`

**Status:** current · **Opened:** 2026-10-01 · **Scope:** `[advisories] ignore` in [`deny.toml`](../../deny.toml)
**Enforced by:** `crates/cognicode-cli/tests/advisory_ignore_backing_contract.rs`

## What this document is

`deny.toml` is the blocking supply-chain gate for releases: it runs at
`release.yml:128` and `release-validate.yml:123`, and `merge-gate` is the only
required check on `main`. Its own header used to say that every ignore below it
had "a reason and a recorded fix path in
`docs/prf/specs/RECONCILIATION-MATRIX.md` and JOURNAL §55".

That was false, and it was false in the way this repository has been bitten
several times before: a written guarantee that no mechanism enforces.

- `RECONCILIATION-MATRIX.md` carries one summary sentence at line 44 claiming
  "5 deudas documentadas". It has no per-advisory row. None of the five ids
  appears in it.
- JOURNAL §55 is "cerrando el hueco que N+54 dejo declarado abierto", about the
  A-013 lifecycle gate. It is not about advisories.
- `docs/prf/` is frozen historical evidence and is not reopened to fix this, so
  the backing record lives here instead.

The real reasoning was in `docs/roadmap/JOURNAL.md:3584-3588`, where CR-07 was
explicitly deferred because breaking `/metrics` in production would be worse
than the advisory. A real trade-off, recorded in the wrong place and cited
wrongly. It is quoted below rather than lost.

## Current ignores

Every row was verified against the advisory database on **2026-10-01** by
running `cargo deny check advisories` with the ignore list emptied and reading
the reported crate, version and solution line. The "why" and "fix path" columns
below are what the tool says, not what was assumed.

| Advisory ID | Crate | Version in tree | Class | Why it is ignored | Fix path | Owner | Authorising record |
|---|---|---|---|---|---|---|---|
| RUSTSEC-2025-0141 | `bincode` | 2.0.1 | unmaintained | The bincode team ceased development permanently after a doxxing and harassment incident. Advisory `Solution:` is "No safe upgrade is available!". It is a **direct** dependency of `cognicode-core`, not a transitive one. | **Re-measured 2026-10-01; see "The bincode row was over-dimensioned" below.** The fix path is a migration away from `bincode`, but it does **not** carry an on-disk format risk: the only artefact `bincode` writes in production is `<workspace>/.cognicode/graph.cache`, it is git-ignored, and it is rebuilt on any parse failure. | Repo maintainer | `a886ecfd` (2026-09-22); premise re-measured under SDDK WorkItem `024c5e6d` |
| RUSTSEC-2026-0192 | `ttf-parser` | 0.25.1 | unmaintained | The author states the crate is unmaintained and will not receive further fixes. Advisory `Solution:` is "No safe upgrade is available!". Arrives as `cognicode-core -> mermaid-rs-renderer 0.2.2 -> fontdb 0.23.0`. | Wait for, or move to, `skrifa`, the actively maintained TrueType/OpenType parser named in the advisory. This is inside a transitive renderer, so the first move is upstream. | Repo maintainer | `a886ecfd` (2026-09-22) |

The three rows above are all `unmaintained` notices whose advisory
`Solution:` is "No safe upgrade is available!". That is a weaker category than
accepting a known vulnerability and the two should not be read as equivalent.
The one row that *was* a live vulnerability — `RUSTSEC-2024-0437` — is now
retired; see below.

### The bincode row was over-dimensioned

The row previously read *"Migrate the serialisation format. … Any of them is a
breaking change to the on-disk format."* That was a claim about blast radius
that nobody had measured, and measuring it found something different. Measured
**2026-10-01** against the tree, not against this document:

| surface | what it actually does |
|---|---|
| persistent store | **SQLite** (`persistence/m0009..m0020.sql`). No `bincode`. |
| `CachedGraphStore` | the real write path, and says so: *"no bincode, no Mutex"*. |
| `InMemoryGraphStore` | encodes to a `Vec<u8>` held in `Mutex<Option<Vec<u8>>>`. It is the default `CogniCodeHandler` fallback when no SQLite (`handlers/mod.rs:240`, `rmcp_adapter.rs:42`), so it runs in production — but those bytes never leave the process. |
| `save_durable_snapshot` | the **only** call-site that writes `bincode` to disk (`handlers/mod.rs:886`). Writes `("cognicode.graph.cache/v1", CallGraph, FileManifest)`. |
| `CallGraphV1` | the legacy v1 blob shape, `deprecated` in `domain/aggregates/mod.rs`; `into_v2` is called only from tests. No production read of legacy blobs. |

Two facts settle the risk:

1. **The artefact is git-ignored.** `.gitignore:92` declares `.cognicode/`, and
   `git check-ignore -q .cognicode/graph.cache` exits 0. The file has never been
   committed and has never been evidence.
2. **It is rebuilt, not preserved.** `load_durable_snapshot` returns `None` on
   any parse failure, in the code's own words: *"a corrupt snapshot is treated as
   absent (rebuild), never as valid evidence."*

So `bincode` holds no user-data contract. The worst case of replacing it is one
graph rebuild, not a broken file. **The row stays open** — `bincode` is still
unmaintained and still ignored — but its fix path no longer promises a format
break that does not exist, and the trigger for migrating it is "we have a reason
to", not "we are one rebuild away from data loss".

This is the register's own lesson applied to itself, again: a claim about
blast radius is a claim until measured. `notify`'s `instant` and the `ttf-parser`
row above carry the same *shape* of premise and have **not** been re-measured
this way; that is their own work item, and nothing here says they are fine.

## Retired ignores

Retirements are recorded here rather than deleted silently. Each was verified as
`advisory-not-detected` by `cargo deny` before removal, and uses the same column
layout as the table above so that the contract treats both uniformly.

| Advisory ID | Crate | Version at retirement | Class | Why it was ignored while it was listed | Fix path applied | Owner | Authorising record |
|---|---|---|---|---|---|---|---|
| RUSTSEC-2023-0057 | `libc` | 0.2.189 | unsound | Claimed: "libc pre-main std access (via crossbeam-deque build path); fix: libc bump". The advisory matched nothing: `libc 0.2.189` is past the affected range, so the ignore carried no risk. | None needed — the dependency moved on its own. Recorded 2026-10-01 so the removal is auditable; `unused-ignored-advisory = "deny"` in `deny.toml` now makes a dead ignore impossible to leave behind quietly. | Repo maintainer | Retired 2026-10-01 under SDDK WorkItem `367ca65e`, evidence `a00449b0` |
| RUSTSEC-2024-0437 | `protobuf` | 2.28.0 | **vulnerability** | Affected versions do not properly parse unknown fields in user-supplied input, allowing a stack overflow on untrusted data. This was a real vulnerability, not a hygiene notice, and it was the only row on this register carrying a deliberate reviewed trade-off. | **Applied 2026-10-01 (CR-07).** Migrated OTel `0.27 -> 0.29`: `opentelemetry-prometheus 0.27.0 -> 0.29.1`, `prometheus 0.13.4 -> 0.14.0`, `protobuf 2.28.0 -> 3.7.2`. The only code change was `PeriodicReader::builder(exporter)` losing its runtime argument; in 0.29 the reader spawns its own thread and `with_runtime()` no longer exists. | Repo maintainer | Deferred at `docs/roadmap/JOURNAL.md:3584-3588`; premise corrected in N+70.7; paid in CR-07 with operator review |
| RUSTSEC-2024-0384 | `instant` | 0.1.13 | unmaintained | Crate is no longer maintained; its author recommends the maintained `web-time` crate instead. Arrived as `cognicode-core -> notify 7.0.0 -> notify-types 1.0.1 -> instant 0.1.13`. | **Closed 2026-10-01** by `notify 7.0.0 -> 8.2.0`. No Rust source changed: `watcher.rs` uses `recommended_watcher`, `Event`, `EventKind`, `RecursiveMode` and `Watcher`, all unchanged across the major. `instant` left the tree entirely — `cargo tree -i instant` answers *"did not match any packages"*. | Repo maintainer | Closed under SDDK WorkItem `024c5e6d`; see below |

### Why the `instant` row had the fix path it had

This is the second time this register's `unmaintained` rows turned out to
carry an unmeasured premise, and both times the measurement was cheap.

The row read *"No version bump fixes this on its own"* and *"Drop the transitive
`notify 7` dependency, which requires moving `cognicode-core` to `notify 8`"*.
Read together those read as: the bump exists but will not help. Measured, the
bump **is** the whole fix:

| | in the tree when written | after `notify 8` |
|---|---|---|
| `notify` | 7.0.0 | 8.2.0 (stable, 53M downloads) |
| `notify-types` | 1.0.1 | 2.1.0 |
| `notify-types` depends on | `instant ^0.1` | **`web-time ^1.1.0`** |
| `instant` in tree | yes | **no** |

`web-time` is the crate this advisory's own author points at. The maintainer of
`notify-types` made the same move the advisory asked for, one major earlier than
this register assumed.

Two things were verified rather than believed:

- **`notify-types 2.0.0`'s dependency list was read directly** from the crates.io
  API. It declares `serde` and `web-time`, and no `instant`.
- **The removal was not decided, it was forced.** With the bump landed and the
  ignore still listed, `unused-ignored-advisory = "deny"` failed on its own:
  `error[advisory-not-detected] … no crate matched advisory criteria`. The
  ignore was removed *because* the gate objected, the same way the protobuf row
  above was.

`notify-types` 1 → 2 does change event types, which is why `notify` 8 ships a
`serialization-compat-6` feature. That risk does not apply here and the reason
is measured: this repository does not serialise notify events. `watcher.rs`
classifies them by `EventKind` to filter code files and to debounce, and stores
nothing. `cargo check -p cognicode-core` is clean and
`cargo test -p cognicode-core --lib ingest::watcher` is 3 passed / 0 failed on
notify 8.2.0.

The `ttf-parser` row above was measured the same way and **holds**: the newest
stable `mermaid-rs-renderer`, 0.3.1, still declares `ttf-parser ^0.25` and
`fontdb ^0.23`, so bumping that renderer would not clear the advisory. Its
"the first move is upstream" fix path stands.

### Why this row is the interesting one in this register

It is the only row that was a **live vulnerability** rather than an
`unmaintained` notice, and the only one that was ever deliberately deferred.
It was deferred on a premise that turned out to be false, so the deferral
and the fix were both for a target that did not exist:

| recorded target | `prometheus` | `protobuf` | resolves the advisory? |
|---|---|---|---|
| `opentelemetry-prometheus 0.28` | `^0.13` | `^2.14` | no |
| `opentelemetry-prometheus 0.29.0` | `^0.13` | `^2.14` | no |
| **`opentelemetry-prometheus 0.29.1`** | `^0.14` | *(none)* | **yes** |

`prometheus 0.14.0` declares `protobuf ^3.7.2`, which is the patched version
the advisory names. So the real migration was two majors, and the only
working stop was a patch release that a version-major reading walks straight
past.

Two things about the fix itself are worth keeping, because both were measured
rather than assumed:

- **`opentelemetry-otlp` never published a `0.29.1`.** Its last release of the
  0.29 line is `0.29.0`; the next is `0.30.0`. So the four OTel crates are not
  version-locked to each other, and asking for a uniform `0.29.1` fails
  resolution outright. `opentelemetry-otlp` is pinned to `0.29.0` and it is not
  a compromise: measured from the pre-migration lockfile, `protobuf` was
  reached only through `opentelemetry-prometheus 0.27.0` and
  `prometheus 0.13.4`, never through otlp. All four still declare
  `opentelemetry ^0.29` + `opentelemetry_sdk ^0.29`, so they resolve together.
- **The deferral's stated risk was real, and it was untested.** *"Romper
  `/metrics` en producción sería peor que el advisory"* was a correct
  prediction with nothing behind it: measured 2026-10-01, **no test in the
  repository exercised `/metrics` at all** before this migration. The risk was
  retired by building the coverage first —
  `crates/cognicode-mcp/tests/cr07_metrics_exposition_contract.rs` pins the
  200, the literal `text/plain; version=0.0.4` content-type that orchestrator
  scrape configs match on, the presence of `# HELP`/`# TYPE` lines, the
  `target_info` marker that proves the exporter is registered rather than
  serving an empty registry, and the reported SDK version.

The removal of the ignore was not a judgement call. With the migration landed,
`unused-ignored-advisory = "deny"` failed the gate on its own:

```
error[advisory-not-detected]: advisory was not encountered
   ┌─ deny.toml:52:6
52 │     "RUSTSEC-2024-0437", # protobuf 2.28.0 ...
   │      ━━━━━━━━━━━━━━━━━ no crate matched advisory criteria
```

A live-vulnerability ignore that has silently become a dead one suppresses
nothing and hides the fact that the tree moved, so the gate being the thing
that removed it is the intended behaviour, not an accident.


## Known debt this register does NOT cover

**None open.** This section previously recorded `yoke-derive 0.8.3` as yanked
and blocking. That is resolved as of 2026-10-01; the resolution is kept here
rather than deleted, because the record of how it was misdiagnosed is the useful
part.

`deny.toml` sets `yanked = "deny"` and no ignore covers yanks, which is correct
and stays. The crate arrived purely transitively:
`cognicode-core -> lsp-types 0.93.2 -> url 2.5.8 -> idna 1.1.0 ->
icu_collections 2.3.0 -> yoke 0.8.3 -> yoke-derive 0.8.3`.

On 2026-10-01 this register was written asserting that no compatible update
existed, on the evidence of `cargo update -p yoke --dry-run` reporting `Locking
0 packages to latest compatible versions`, and that resolving it would require
advancing the `lsp-types` / `url` / `idna` chain or waiving the yank policy. Both
were wrong, and the mistake was to query the wrong crate: the gate's own
message names `cargo update -p yoke-derive`, and `yoke`'s newest release *is*
0.8.3, which the lock already held. Running the command against `yoke` correctly
reported nothing to do, and that null result was read as proof of impossibility.

Re-measured against the crates.io API: `yoke-derive` 0.8.4 exists and is not
yanked, and `cargo update -p yoke-derive --dry-run` reports `Locking 1 package to
latest compatible version`. The fix was a patch bump of one proc-macro, two lines
of `Cargo.lock`. No policy waiver was needed and no MCP-facing type moved.

**`cargo deny check advisories` is green as of 2026-10-01**, verified on the
resulting lockfile, with `cargo deny check licenses` also green.

The general lesson is the one this register exists for, applied to itself: a
negative result from a tool is evidence about the query that was run, not about
the world. The query was wrong, and the conclusion was stated with enough
confidence to shape a work item around it.

## Adding or retiring an ignore

1. Get the row from `cargo deny check advisories`, not from memory. The tool
   reports the crate, the version and the `Solution:` line; the previous header
   in `deny.toml` was wrong about two of the four current rows precisely because
   it was written from memory.
2. Add or update the row in the table above, with a non-empty fix path and a
   named authorising record.
3. `advisory_ignore_backing_contract` fails if a listed ignore has no row, or if
   a row exists for an id that is neither listed nor retired.
4. `unused-ignored-advisory = "deny"` means an ignore that no longer matches
   anything is an error, not a warning.
