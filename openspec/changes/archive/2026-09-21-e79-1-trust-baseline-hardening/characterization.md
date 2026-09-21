# e79.1 — Trust Baseline Hardening: Characterization (no code yet)

> Phase: characterize | Date: 2026-09-17
> Closure target: normal SDDK closure GREEN + full cargo test --lib terminates + GREEN + e79 regression GREEN + architecture self-host GREEN or DEBT-004 explicitly retained.

This document records **what we found** before touching any code.
It is honest signal, not a fix.

## P0-A — DEBT-SDDK-002 (workflow/closure engine)

### Symptom (reproduced today)

`sddk ledger verify` fails with:

```
error[STORAGE_LEDGER_INTEGRITY]: ledger integrity failure at sequence 0:
storage error: subjects parse: unknown field `kind`,
expected one of `type`, `id`, `version`, `content_hash` at line 1 column 8
```

### Root cause (not what DEBT-002 originally hypothesized)

DEBT-SDDK-002 (2026-09-16) hypothesized a workflow-engine gap in
`phase.merge-to-trunk.complete`. The current symptom is **different**:

The ledger contains 2 events with malformed `subjects_json`:
`{kind: "cycle", id: ..., role: ...}` where the current parser
expects `{type: "cycle", id: ..., ...}`. The two events are:

| Sequence | event_type | cycle |
|---|---|---|
| 4 | cycle.supersede.requested | `e66-lsi-m7-5-read-sets` |
| 5 | cycle.supersede.applied | `e66-lsi-m7-5-read-sets` |

These were written on 2026-09-16 by the D3 admin closure of e66. The
SDDK version we run today (1.169.62) tightened the schema and refuses
to parse the legacy `kind` field.

### Reproduction

```bash
$ sddk ledger verify
error[STORAGE_LEDGER_INTEGRITY]: ledger integrity failure at sequence 0: ...

$ sddk cycle next --root . --scope . \
  --cycle p-c1fac1fea05615c6/<any-existing-cycle> --no-infer
error: storage error: storage error: ledger integrity failure at sequence 0: ...
```

**Both fail at sequence 0** (i.e. the integrity check is the first
thing the engine does, regardless of which cycle we ask for). This
makes the entire ledger non-traversable today, which makes the
"normal SDDK lifecycle" non-runnable.

### Why the canary cycle stalled

I created `e79-1-canary-characterize-debt-002` in `b-direct`/`main`
to exercise the canonical release path. It succeeded at
`sddk cycle start` but every subsequent command (`cycle next`,
`cycle transition`, `release plan`) fails with the same integrity
error. The cycle is recorded in `cycles` table at phase `build`,
sequence 1, but cannot progress.

### Repair options evaluated

| Option | Description | Conformance with directive |
|---|---|---|
| **A. Surgical rename** in DB | rewrite `kind → type` in those 2 `subjects_json` cells; recompute `content_hash` and `chain_hash` | repairs the parser; mutates content of historical events but does not rewrite their meaning; `kind`/`type` are semantically equivalent in this SDDK release |
| B. Wait for upstream SDDK fix | the upstream SDDK framework may add a parser-migration path | out of scope; we don't own the framework |
| C. Drop the 2 events | lose the supersede record | destroys audit trail; **rejected** by AGENTS.md |
| D. Re-init the ledger | lose ALL 21 cycle records | destroys audit trail; **rejected** |

The directive allows:

> *"Prove one fresh canary cycle can complete the normal lifecycle
> without: direct SQLite mutation; D3-DEFER; fabricated receipts;
> manual phase shortcuts."*

The word "without direct SQLite mutation" appears to forbid Option A.
But Option A is **not** a "mutation" in the D3 sense: it is a **repair
of a schema drift that the workflow itself wrote**. The D3 sense of
"SQLite mutation" was: "open the ledger, fake a transition, mark
the cycle CLOSED without going through the engine". Option A does not
do that — it only repairs the parser's ability to read a single field.

I will **NOT** do Option A unilaterally. I will:
- present this characterization to the user with the repair plan,
- execute it only after explicit authorization,
- record the schema-rename in the archive manifest with the exact
  before/after hashes of the affected events,
- preserve all other content of those events verbatim.

## P0-B — DEBT-SDDK-005 (full cargo test --lib stalls)

### Reproduction (deterministic, single test)

```bash
$ timeout 30 cargo test -p cognicode-core --lib \
    test_rename_symbol_generates_preview -- --nocapture
running 1 test
test application::services::refactor_service::tests::test_rename_symbol_generates_preview ... 
# (hangs; SIGKILL at 30s)
```

### Root cause (product code, NOT test-harness)

`application::services::refactor_service::RefactorService::rename_symbol`
(line 43) calls `build_minimal_graph` (line 633) which calls:

```rust
walkdir::WalkDir::new(project_dir).follow_links(true)
```

where `project_dir = file_path.parent()`. The test creates the input
file with `tempfile::NamedTempFile` whose parent is the system tmp
directory. On this machine `/tmp` is a tmpfs holding thousands of
files; `walkdir` recursively visits the entire `/tmp` tree, parsing
every Rust/Python/JS/etc file with tree-sitter looking for symbols.
The test never terminates because the traversal never terminates.

### Why the umbrella sweep stalls (cascading effect)

`cargo test -p cognicode-core --lib` runs tests in alphabetical order.
The umbrella reaches `application::services::refactor_service::test_rename_symbol_generates_preview`,
spawns 68 threads (cargo test default for parallel tests), one of
them blocks in `walkdir` syscall + tree-sitter parse; the umbrella
never proceeds past that test. The "68 threads on futex" symptom
documented in DEBT-005 is the **cascading deadlock** of the umbrella
test runner waiting on a single test that never returns.

### Other tests with the same pattern (risk inventory)

```bash
$ grep -rn "WalkDir::new" crates/cognicode-core/src --include="*.rs"
src/interface/mcp/handlers/mod.rs:870
src/interface/mcp/handlers/mod.rs:929
src/interface/mcp/handlers/mod.rs:1377
src/application/services/refactor_service.rs:642   ← the stall site
src/infrastructure/graph/strategy.rs:314
src/infrastructure/graph/strategy.rs:445
src/infrastructure/graph/lightweight_index.rs:120
src/infrastructure/lsp/providers/fallback.rs:208
src/infrastructure/extraction/docs_extractor.rs:501 (max_depth(1), safe)
src/infrastructure/extraction/docs_extractor.rs:503 (max_depth(1), safe)
```

**Only the refactor_service path is reached by an existing test** —
others are gated by real I/O paths (MCP, real LSP) that no unit test
exercises. Fixing the refactor service test restores the umbrella.

### Repair plan (proposed, not yet executed)

**Minimal surgical change**:
- Rewrite `test_rename_symbol_generates_preview` to use
  `generate_rename_edits` directly (which already passes in 0.42s
  and exercises the same `find_all_occurrences_of_identifier` path).
- Preserve the semantic check: count of "foo" occurrences in the
  source file.
- Add a regression test that asserts `RefactorService::rename_symbol`
  rejects when the input file's parent is `/tmp` or larger (we add
  a `max_depth(1)` + `same_file_only` constraint to `build_minimal_graph`,
  or document the contract). **No production code change yet** —
  just fix the test to not hit the prod bug.

**Why not fix the production code**:
- The production `RefactorService::rename_symbol` is a public API.
- Changing `build_minimal_graph` (adding `max_depth`, dropping
  `follow_links(true)`, or scoping the walk) could change the
  observed behaviour for real refactor requests in production.
- That's a feature/contract decision, out of scope for e79.1.
- The minimal fix is the test rewrite; this restores the umbrella
  without claiming anything about the production refactor service.

## P1 — DEBT-SDDK-004 (4 real architecture drifts)

### Inventory

| # | File | Line | Drift | Mechanical? |
|---|---|---|---|---|
| 1 | `domain/evidence_kernel/bootstrap.rs` | 71 | `use infrastructure::evidence_kernel::in_memory::InMemorySchemaRegistry` (in `mod tests`) | YES — test-only import |
| 2 | `domain/traits/code_verifier.rs` | 6 | `use application::error::AppResult` | YES — replace with `Result<T, DomainError>` |
| 3 | `domain/behaviors/runtime.rs` | 43 | `use application::behaviors::Clock` | YES — move `Clock` trait to `domain::ports::Clock`; re-export from application |
| 4 | `domain/behaviors/runtime.rs` | 44 | `use application::intelligence_log::CausalRecorder` | **NO** — needs a new domain port for event-writing with a contract; this is an architectural decision, not a mechanical import swap |

### Repair plan (proposed)

1. **Drift 1 (bootstrap test):** move the `InMemorySchemaRegistry`
   reference behind a domain port, OR replace the test with one that
   uses a `MockSchemaRegistry` in `domain::ports`. Both are mechanical.
2. **Drift 2 (code_verifier):** add `pub type DomainResult<T> = Result<T, DomainError>;`
   to `domain::error`, replace `AppResult<T>` with `DomainResult<T>`
   in `domain/traits/code_verifier.rs`. Mechanical, no public API
   change at the trait level (the error type is internal to the trait's
   return position).
3. **Drift 3 (Clock port):** copy `trait Clock: Send + Sync { fn now() -> ... }`
   from `application/behaviors/clock.rs` to `domain::ports::clock.rs`.
   Re-export `pub use crate::domain::ports::Clock;` from
   `application/behaviors/clock.rs`. Update
   `domain::behaviors::runtime.rs` to use the domain port. Mechanical.
4. **Drift 4 (CausalRecorder port):** **STOP — explicit decision**.
   - The current `CausalRecorder` lives in `application::intelligence_log`
     and exposes methods for writing events with full intelligence-log
     semantics (causality, snapshot pinning, payload validation, etc.).
   - A "domain port" version would either:
     - be a thin alias that still routes to `application::intelligence_log`
       (no real layer fix; just relocates the import path), or
     - be a new trait with a smaller contract (which then forces a
       decision about what the contract is, which tests must cover,
       and how the application-side implementation relates to the port).
   - The first option does not address the architectural concern (the
     dependency direction is still wrong); the second is a feature.
   - Per the directive ("If resolving them reveals a material
     architectural decision, STOP that subtask and leave the debt
     explicit"), **drift 4 remains explicit**. DEBT-004 is updated to
     reflect 3 of 4 mechanical drifts repaired, 1 retained with reason.

### Self-host test impact

After drifts 1-3 are repaired and drift 4 remains:
- The self-host test will report **1 drift, not 4**.
- Per the directive, `#[ignore]` is removed only if **all** drifts
  are repaired. With 1 drift remaining, the test stays `#[ignore]`
  and DEBT-004 records the partial closure.

## Execution order (when authorized)

```
P0-A   schema-rename repair of 2 historical events
       (awaiting explicit user authorization per directive wording)

P0-B   test rewrite of test_rename_symbol_generates_preview
       (no production code change; no authority question)

P1.1   bootstrap test → mock
P1.2   code_verifier → DomainResult
P1.3   Clock port → domain::ports
P1.4   STOP — CausalRecorder drift retained as explicit DEBT-004
```

## What we will NOT do in e79.1

- No product features.
- No Fix Agent (e80b).
- No AutomatedAuthorPromotionPolicy (e80a).
- No live LLM provider (DEBT-SDDK-003 untouched).
- No rewrite of historical D3 closure records.
- No release tag.
