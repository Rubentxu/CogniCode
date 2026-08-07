# Tasks — issue-tracker-adapter

> 16 tasks across 3 batches. **Strict TDD**: every task starts with a RED test. Dependencies run in the order listed. Every task is estimated against the existing `docs_extractor` / `docs_confidence_rules` patterns (the new code is structurally identical, so the estimates are anchored).

## Batch 1 — Domain types + confidence rules + write path (no external deps)

> **Gate**: all 6 tasks in this batch MUST pass with `cargo build -p cognicode-core` and `cargo build -p cognicode-explorer` (no `octocrab` pulled in). The `multimodal` feature is extended, but no new external dep is required to land this batch.

---

### T1. `ISSUE_*_PROPERTIES` const module

**Depends on**: nothing.
**Spec**: `issue-node-properties/spec.md` (entire file).
**RED gate**: `validate_issue_properties` test — 6 cases (all required, missing key, bad status, bad tracker, case-folding, label comma). Module-absent under `--no-default-features`.
**Files**:
- `crates/cognicode-core/src/domain/value_objects/issue_properties.rs` (NEW, ~120 LOC)
- `crates/cognicode-core/src/domain/value_objects/mod.rs` (add `pub mod issue_properties;` behind `#[cfg(feature = "multimodal")]`)
**Estimated LOC**: 120 + 1.

### T2. `IssuesConfidenceRules` — 4-tier scoring

**Depends on**: nothing.
**Spec**: `issues-confidence-rules/spec.md` (entire file).
**RED gate**: 8 tests — 4 tier values, 4 provenance tags, idempotency (1000 calls), case-insensitivity. Compile-gate test confirms the module is absent under `--no-default-features`.
**Files**:
- `crates/cognicode-core/src/infrastructure/extraction/issues_confidence_rules.rs` (NEW, ~180 LOC, copy of `docs_confidence_rules.rs` with the 4 new tiers)
- `crates/cognicode-core/src/infrastructure/extraction/mod.rs` (register module, gated)
**Estimated LOC**: 180 + 2.

### T3. `parse_commit_issue_refs` — pure fn over `git log` output

**Depends on**: T1 (uses `validate_issue_properties` indirectly through the extractor, but T3 itself is independent — it just produces `CommitIssueRef` structs).
**Spec**: `issues-extractor/spec.md` §"Git log parsing for commit-issue references".
**RED gate**: 12 tests — Fixes, Closes, Resolves, Refs, Part-of, See (×2 case variants), multi-pattern per commit, `#0` rejected, empty log, single separator, non-UTF-8 stripped, case-fold.
**Files**:
- `crates/cognicode-core/src/infrastructure/git/commit_issue_parser.rs` (NEW, ~150 LOC)
- `crates/cognicode-core/src/infrastructure/git/mod.rs` (NEW, ~5 LOC)
- `Cargo.toml` (workspace) — confirm `regex` is a workspace dep (it is)
**Estimated LOC**: 150 + 6.

### T4. `GraphRepository` write methods + in-memory mock

**Depends on**: nothing.
**Spec**: `graph-repository-write/spec.md` (entire file).
**RED gate**: 6 tests on the in-memory mock — insert, re-ingest idempotency, all-or-nothing batch, self-loop rejected pre-DB, empty vec, duplicate `(id, kind)` rejected. 1 dyn-compat test (the trait still supports `Box<dyn GraphRepository + Send + Sync>`).
**Files**:
- `crates/cognicode-explorer/src/ports/graph_repository.rs` (MODIFIED, +40 LOC: 2 new methods + their doc-comments)
- `crates/cognicode-explorer/src/adapters/inmem_graph_repository.rs` (NEW, ~120 LOC, behind `#[cfg(any(test, feature = "multimodal"))]`)
- `crates/cognicode-explorer/src/adapters/mod.rs` (add the new module)
**Estimated LOC**: 40 + 120 + 1.

### T5. PG migration + PG adapter implementation

**Depends on**: T4.
**Spec**: `graph-repository-write/spec.md` §"PG adapter implementation".
**RED gate**: 1 PG integration test (CI only, behind `--features multimodal,postgres`): the `ON CONFLICT` round-trip on `graph_nodes` + `graph_edges`. 1 migration test: the `UNIQUE` constraints exist post-migration.
**Files**:
- `crates/cognicode-explorer/migrations/20260610000001_graph_upsert_constraints.sql` (NEW, ~10 LOC)
- `crates/cognicode-explorer/src/adapters/pg_graph_repository.rs` (NEW, ~250 LOC, `impl GraphRepository for PgGraphRepository`)
- `crates/cognicode-explorer/src/adapters/mod.rs` (register, behind `#[cfg(feature = "postgres")]`)
**Estimated LOC**: 10 + 250 + 1.

### T6. Wire `docs_ingest` to the new write path

**Depends on**: T4, T5.
**Spec**: `graph-repository-write/spec.md` §"TDD RED gate" (item 4).
**RED gate**: 1 regression test — re-running `docs_ingest` after the new write path is wired reports `nodes_created == 0` for unchanged files. 1 freshness test — first call on a fresh DB reports `nodes_created == N` and the rows are actually in `graph_nodes`.
**Files**:
- `crates/cognicode-explorer/src/mcp.rs` (MODIFIED, `dispatch_docs_ingest` gains a `graph_repo: Option<&Arc<dyn GraphRepository>>` parameter and calls `upsert_nodes` + `upsert_edges`)
- `crates/cognicode-explorer/src/mcp.rs` (MODIFIED, the dispatch `match` arm passes the new parameter)
- `crates/cognicode-cli/src/main.rs` (or `cognicode-core/src/interface/cli/commands.rs`) — CLI also calls `upsert_nodes` (so `cognicode docs-ingest` actually persists)
**Estimated LOC**: 60 + 5 (CLI).
**Note**: this task is the shared write-path fix from the proposal's "Modified Capabilities" section. Without it, `issues_ingest` would not have a writer to delegate to.

---

## Batch 2 — IssuesExtractor + GitHub API + git log parsing

> **Gate**: all 5 tasks in this batch MUST pass with `cargo build -p cognicode-core --features multimodal`. The first external dep (`octocrab`) lands in this batch; the default build is still byte-for-byte unchanged.

---

### T7. `GitHubClient` trait + `RawIssue` DTO

**Depends on**: T1, T2.
**Spec**: `design.md` §3 ("GitHub API") + `issue-node-properties/spec.md` §"Issue Property Schema".
**RED gate**: 3 tests — the trait is object-safe (`Box<dyn GitHubClient + Send + Sync>`), the `RawIssue` DTO deserialises from a sample `octocrab` JSON fixture, the error variants are exhaustively matched.
**Files**:
- `crates/cognicode-core/src/infrastructure/github/mod.rs` (NEW, ~10 LOC)
- `crates/cognicode-core/src/infrastructure/github/client.rs` (NEW, ~120 LOC — trait + `RawIssue` + `IssueState` enum + `GitHubError`)
**Estimated LOC**: 10 + 120.

### T8. `OctocrabClient` — production impl

**Depends on**: T7.
**Spec**: `design.md` §3.
**RED gate**: 1 contract test (uses `mockito` or `wiremock` to stub the GitHub API) — `list_issues("acme", "widgets", Open)` returns 5 issues from a stub. 1 token-missing test — `OctocrabClient::new()` with no `GITHUB_TOKEN` builds an unauthenticated client (returns the 60-req/hr variant).
**Files**:
- `crates/cognicode-core/src/infrastructure/github/octocrab_client.rs` (NEW, ~180 LOC)
- `crates/cognicode-core/src/infrastructure/github/mod.rs` (add module, gated behind `#[cfg(feature = "multimodal")]`)
- `Cargo.toml` (workspace) — add `octocrab = "0.43"` to `[workspace.dependencies]`
- `crates/cognicode-core/Cargo.toml` — add `octocrab = { workspace = true, optional = true }` and extend `multimodal = ["dep:octocrab", …]`
**Estimated LOC**: 180 + 2 + 1 + 1.
**Dependency risk**: `octocrab` is a mid-sized crate; CI must run a clean build to catch the dep-graph change.

### T9. `MockGitHubClient` — test impl

**Depends on**: T7.
**Spec**: `design.md` §3.
**RED gate**: 1 test — `MockGitHubClient::with_issues(vec![…]).list_issues(…)` returns the canned vec. 1 test — `MockGitHubClient::with_error(GitHubError::RateLimited).list_issues(…)` returns the error. The mock is `#[cfg(test)]`-only.
**Files**:
- `crates/cognicode-core/src/infrastructure/github/mock_client.rs` (NEW, ~80 LOC)
- `crates/cognicode-core/src/infrastructure/github/mod.rs` (add module, gated behind `#[cfg(test)]`)
**Estimated LOC**: 80 + 1.

### T10. `IssuesExtractor` — `parse_github_issues` + async impl

**Depends on**: T1, T2, T3, T7, T8, T9.
**Spec**: `issues-extractor/spec.md` (entire file).
**RED gate**: 8 tests — URL → 5 issues, Directory → 3 commit refs, `SourcePath::File` → `Unsupported`, GHE URL rejected, missing `GITHUB_TOKEN` (mocked), rate limit (mocked), body mention edge, idempotency (re-extract → same ids), dyn-compat.
**Files**:
- `crates/cognicode-core/src/infrastructure/extraction/issues_extractor.rs` (NEW, ~450 LOC, copy of `docs_extractor.rs` with the issue-specific logic)
- `crates/cognicode-core/src/infrastructure/extraction/mod.rs` (register, gated)
**Estimated LOC**: 450 + 2.
**Note**: this is the largest single file in the change. The split is `parse_github_issues` (pure fn, ~150 LOC) + `parse_commit_issue_refs`-integration (delegates to T3) + `IssuesExtractor` async impl (~150 LOC) + tests (~100 LOC).

### T11. End-to-end `IssuesExtractor` integration test

**Depends on**: T10.
**Spec**: `issues-extractor/spec.md` §"TDD RED Gate" item 8 (idempotency) + item 4 (confidence tier mapping).
**RED gate**: 1 integration test that:
1. Constructs an `IssuesExtractor` with a `MockGitHubClient` returning 5 issues and a fake git log with 3 commit refs.
2. Calls `.extract(SourcePath::Url("https://github.com/acme/widgets"))` and `.extract(SourcePath::Directory("/tmp/fake-repo"))` and concatenates.
3. Asserts the resulting `Vec<ExtractedNode>` has 5 issue nodes + 3 commit nodes, and 3 `Resolves` edges.
4. Re-runs the extraction and asserts the second run produces the same `NodeId`s (idempotency).
**Files**:
- `crates/cognicode-core/src/infrastructure/extraction/issues_extractor.rs` (extend the test mod)
**Estimated LOC**: 60 (test only).

---

## Batch 3 — MCP tool + CLI + frontend polish

> **Gate**: all 5 tasks in this batch MUST pass `cargo build -p cognicode-mcp --features multimodal` AND the regression suite (`cargo test -p cognicode-mcp`). The `tools/list` snapshot updates from 30 to 31.

---

### T12. MCP `TOOL_ISSUES_INGEST` constant + dispatch arm + schema entry

**Depends on**: T10.
**Spec**: `issues-ingest-mcp/spec.md` §"issues_ingest Tool Registration" + §"issues_ingest Input Schema".
**RED gate**: 3 tests — `TOOL_NAMES.len() == 31` (when feature on), `TOOL_NAMES.len() == 30` (when feature off), `build_tool_schemas()` includes `issues_ingest` (when feature on).
**Files**:
- `crates/cognicode-explorer/src/mcp.rs` (MODIFIED, +20 LOC: constant, `TOOL_NAMES` extension, schema entry)
**Estimated LOC**: 20.
**Note**: the existing 30-tool snapshot test (T14 in `mcp-multimodal-tools`) MUST be updated to 31 in the same PR — the test that asserts `TOOL_NAMES.len() == expected` is the regression gate.

### T13. MCP `dispatch_issues_ingest` helper

**Depends on**: T4, T5, T10, T12.
**Spec**: `issues-ingest-mcp/spec.md` §"issues_ingest Dispatch" + §"issues_ingest Output Schema".
**RED gate**: 5 tests — happy path (5 issues, mocked), `invalid_input` (missing source), `github_auth_required` (401), `github_rate_limited` (403 + `X-RateLimit-Remaining: 0`), `issues_ingest_unavailable` (no graph_repo).
**Files**:
- `crates/cognicode-explorer/src/mcp.rs` (MODIFIED, +150 LOC: the dispatch helper + the `IssuesIngestArgs` struct + the match arm in `dispatch`)
**Estimated LOC**: 150.

### T14. CLI `cognicode issues-ingest` subcommand

**Depends on**: T10.
**Spec**: `issues-extractor/spec.md` §"Out of Scope" (no — the CLI is in scope) + `design.md` §7.
**RED gate**: 2 integration tests (`assert_cmd`):
1. `cognicode issues-ingest https://github.com/acme/widgets --dry-run` exits 0 and prints a summary table.
2. `cognicode issues-ingest /nonexistent/path` exits 1 and prints a clear error.
**Files**:
- `crates/cognicode-core/src/interface/cli/commands.rs` (MODIFIED, +120 LOC: `IssuesIngest` clap variant + `execute_issues_ingest` fn)
- `crates/cognicode-core/src/interface/cli/mod.rs` (add the variant to the `Commands` enum, gated)
**Estimated LOC**: 120 + 2.
**Note**: the CLI is a read-only consumer — it prints what WOULD be persisted, mirroring the existing `docs-ingest` shape.

### T15. Frontend `ObjectInspector` Issue-specific view

**Depends on**: T10 (the `NodeKind::Issue` nodes must be in the index to render anything).
**Spec**: `multimodal-frontend/spec.md` §"ObjectInspector Multimodal Fields" (already covers the generic shape) — this task adds Issue-specific polish.
**RED gate**: 1 Playwright snapshot test — given a fixture `Issue` node with `properties: { number: "42", status: "open", url: "…" }`, the inspector renders the title, the "Issue #42" badge, the status pill, and a "Open on GitHub" link. 1 regression — the existing 3 issue suggestions (`iss-resolves`, `iss-resolved-by`, `iss-related`) still appear in the suggestion bar.
**Files**:
- `apps/explorer-ui/src/components/ObjectInspector/multimodal.ts` (MODIFIED, +60 LOC: an `Issue` view that renders the badge + the URL link)
- `apps/explorer-ui/src/components/ObjectInspector/ObjectInspector.tsx` (MODIFIED, dispatch to the new view when `kind === "issue"`)
**Estimated LOC**: 60 + 5.
**Note**: the proposal says "Issue node properties already styled" — the existing red triangle + "Issue" badge are correct. This task only polishes the inspector body (the URL link is the only new affordance).

### T16. Cross-cutting regression + final verify

**Depends on**: T1–T15.
**Spec**: implicit — every spec has a TDD RED gate; this task is the final pass.
**RED gate** (this is the SHIP / NO-SHIP gate, not a new test):
1. `cargo test -p cognicode-core --features multimodal` — all unit + integration tests pass.
2. `cargo test -p cognicode-explorer --features multimodal,postgres` — PG round-trip passes (CI only).
3. `cargo test -p cognicode-mcp --features multimodal` — the 30 → 31 tool list test passes.
4. `cargo clippy --features multimodal --all-targets -- -D warnings` — no clippy warnings.
5. `cargo build --no-default-features` — the default build is byte-for-byte unchanged (regression: `IssuesExtractor` is absent, `octocrab` is not in the dep graph, `issues_ingest` is not in `TOOL_NAMES`).
6. The CogniCode `tools/list` smoke test (Playwright) reports 31 tools when the multimodal build is running.
7. A manual end-to-end smoke: `issues_ingest` on a real public GitHub repo (e.g. `https://github.com/octocat/Hello-World`) produces ≥ 1 `Issue` node + at least one row in `graph_nodes` (verified via `graph_search`).
**Files**: none — this task is the merge gate.
**Estimated LOC**: 0.

---

## Summary

| Batch | Tasks | New files | Modified files | Est. LOC (new) | Est. LOC (modified) |
|-------|-------|-----------|----------------|----------------|---------------------|
| 1 — Domain + write path | T1, T2, T3, T4, T5, T6 | 4 | 4 | ~770 | ~115 |
| 2 — Extractor + GitHub | T7, T8, T9, T10, T11 | 5 | 3 | ~950 | ~5 |
| 3 — MCP + CLI + frontend | T12, T13, T14, T15, T16 | 0 | 4 | 0 | ~360 |
| **Total** | **16** | **9** | **11** | **~1,720** | **~480** |

**Critical path**: T1 → T4 → T6 → T7 → T10 → T12 → T13 (8 tasks). The other 8 are parallel-friendly within their batch.

**External dep adds**: 1 (`octocrab = "0.43"`, optional, behind `multimodal`).
**External dep upgrades**: 0.
**DB migrations**: 1 (`graph_nodes` + `graph_edges` unique constraints).
**New MCP tools**: 1 (`issues_ingest`).
**New CLI subcommands**: 1 (`cognicode issues-ingest`).

## Rollback

| Step | Action | Risk |
|------|--------|------|
| 1 | `git revert` the merge commit | None — the change is self-contained |
| 2 | Drop the migration | None — `graph_nodes` / `graph_edges` keep their pre-change schema |
| 3 | Remove `octocrab` from `Cargo.toml` | None — the dep is optional + feature-gated |
| 4 | Verify `cargo build --no-default-features` is byte-for-byte identical | None — every new symbol is `#[cfg(feature = "multimodal")]` |
