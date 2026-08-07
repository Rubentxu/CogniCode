# Design — issue-tracker-adapter

> Companion to the 5 delta specs under `openspec/changes/issue-tracker-adapter/specs/`. The design follows the project's hexagonal layering (domain → ports → adapters → MCP triggers) and is entropy-aware (Protocols B + C of `entropy-sdd`).

## 1. Architecture

The change adds one new adapter (`IssuesExtractor`), one new scoring module (`IssuesConfidenceRules`), and one new port method (write capability on `GraphRepository`). The MCP tool and CLI command are thin triggers that compose the new pieces — they follow the exact pattern of `docs_ingest` / `docs-ingest`.

```
crates/cognicode-core/src/infrastructure/extraction/
├── mod.rs                            (register issues_extractor, issues_confidence_rules)
├── issues_extractor.rs               (NEW — IssuesExtractor, parse_github_issues, parse_commit_issue_refs)
├── issues_confidence_rules.rs        (NEW — ConfidenceTier + 4 scoring fns)
├── docs_extractor.rs                 (existing — now also calls graph_repo.upsert)
└── docs_confidence_rules.rs          (existing)

crates/cognicode-core/src/domain/value_objects/
└── issue_properties.rs               (NEW — ISSUE_REQUIRED_PROPERTIES, ISSUE_TRACKERS, validate_issue_properties)

crates/cognicode-core/src/infrastructure/git/
└── commit_issue_parser.rs            (NEW — pure fn over `git log` output)

crates/cognicode-core/src/infrastructure/github/
├── mod.rs                            (NEW — GitHubClient trait)
├── octocrab_client.rs                (NEW — OctocrabClient: GitHubClient impl, behind feature)
└── mock_client.rs                    (NEW — MockGitHubClient: for unit tests, behind #[cfg(test)])

crates/cognicode-core/src/interface/cli/commands.rs   (MODIFIED — add execute_issues_ingest)

crates/cognicode-explorer/src/ports/graph_repository.rs   (MODIFIED — add upsert_nodes, upsert_edges)
crates/cognicode-explorer/src/mcp.rs                       (MODIFIED — add TOOL_ISSUES_INGEST, dispatch_issues_ingest, schema)
crates/cognicode-explorer/src/adapters/                    (new pg_graph_repository adapter, in-memory test mock)
```

`IssuesExtractor` lives in `cognicode-core` (not `cognicode-explorer`) because the existing `DocsExtractor` lives there — extractor + scoring is core domain knowledge, not explorer-specific.

## 2. Data Model — Issue Property Schema

The schema is centralised in `crates/cognicode-core/src/domain/value_objects/issue_properties.rs`:

```rust
#[cfg(feature = "multimodal")]
pub const ISSUE_REQUIRED_PROPERTIES: &[&str] = &[
    "number", "title", "status", "url", "tracker", "repo",
];
#[cfg(feature = "multimodal")]
pub const ISSUE_OPTIONAL_PROPERTIES: &[&str] = &[
    "labels", "assignee", "author", "created_at", "updated_at",
];
#[cfg(feature = "multimodal")]
pub const ISSUE_TRACKERS: &[&str] = &["github", "gitlab", "linear", "jira"];
#[cfg(feature = "multimodal")]
pub const ISSUE_STATUSES: &[&str] = &["open", "closed"];

#[cfg(feature = "multimodal")]
pub fn validate_issue_properties(props: &HashMap<String, String>) -> Result<(), String> { … }
```

The 4-tier `ConfidenceTier` lives in `issues_confidence_rules.rs` (mirroring `docs_confidence_rules.rs`):

```rust
#[cfg(feature = "multimodal")]
pub enum ConfidenceTier {
    ExplicitLink,  // 0.9, Extracted
    CommitFixes,   // 0.85, Extracted
    CommitRefs,    // 0.7, Inferred
    BodyMention,   // 0.5, Inferred
}
```

`NodeId` convention: `issue:{tracker}/{repo}#{number}` (e.g. `issue:github/acme/widgets#42`). The `+` is reserved for V2 (cross-tracker mirrors). Determinism is the entire reason this scheme exists: `INSERT … ON CONFLICT (id, kind) DO UPDATE` requires a stable id.

## 3. GitHub API (DIP)

A thin `GitHubClient` trait abstracts the HTTP layer:

```rust
#[cfg(feature = "multimodal")]
#[async_trait]
pub trait GitHubClient: Send + Sync {
    async fn list_issues(&self, owner: &str, repo: &str, state: IssueState) -> Result<Vec<RawIssue>, GitHubError>;
}
```

`OctocrabClient` wraps `octocrab::Octocrab` and is the production impl. `MockGitHubClient` returns canned JSON and lives in `mock_client.rs` behind `#[cfg(test)]`. The extractor is generic over `Arc<dyn GitHubClient>` — constructor injects the client, no module-level singletons.

`octocrab` is added to `cognicode-core/Cargo.toml` as `optional = true` under `[features] multimodal = ["dep:octocrab", …]` (the `multimodal` feature already exists; we extend it). `GITHUB_TOKEN` is read once at startup via `std::env::var("GITHUB_TOKEN")` and passed to `Octocrab::builder().personal_token(token).build()`. The unset case constructs an unauthenticated client (60 req/hr) and the first 401 surfaces a clear error — never a panic.

## 4. Git Log Parsing

`commit_issue_parser.rs` exposes a single pure function:

```rust
#[cfg(feature = "multimodal")]
pub fn parse_commit_issue_refs(log_output: &str, owner: &str, repo: &str) -> Vec<CommitIssueRef>;

#[cfg(feature = "multimodal")]
pub struct CommitIssueRef {
    pub commit_sha: String,        // short SHA, 7 chars
    pub issue_number: u32,
    pub ref_kind: CommitRefKind,   // Fixes | Closes | Resolves | Refs | PartOf | See
}
```

The regex is built once (via `once_cell::sync::Lazy` — `once_cell` is already a workspace dep transitively) and used in a tight loop over `git log --all --pretty=format:%H%x1f%s%x1f%b` output (the `\x1f` separator is the ASCII unit-separator — never appears in commit messages). Patterns:

| Pattern (case-insensitive) | `ref_kind` | Tier (from confidence rules) |
|---------------------------|-----------|-------------------------------|
| `\b(fixes\|closes\|resolves)\s+#(\d+)\b` | `Fixes` | `CommitFixes` (0.85) |
| `\b(refs\|references\|see\|part of)\s+#(\d+)\b` | `Refs` | `CommitRefs` (0.7) |

The function is a pure string transform — no `git` subprocess, no I/O. The `IssuesExtractor` spawns the `git` subprocess once and pipes the output through. This split is the key TDD-friendly seam: `parse_commit_issue_refs` has 12 unit tests against canned `git log` strings and never touches the filesystem.

The `owner/repo` is read from `git remote get-url origin` once at the start of `Directory` mode via a helper that does the subprocess + URL parse. The branch: if the remote is missing, the extractor falls back to `unknown/unknown` and emits `tracing::warn!`.

## 5. Repository Write Path

`GraphRepository` gains two methods (with the in-memory mock + the PG adapter both implementing them):

```rust
#[cfg(feature = "multimodal")]
pub trait GraphRepository: Send + Sync {
    // … existing read methods …
    fn upsert_nodes(&self, nodes: Vec<GraphNode>) -> ExplorerResult<usize>;
    fn upsert_edges(&self, edges: Vec<GraphEdge>) -> ExplorerResult<usize>;
}
```

`upsert_nodes` semantics: `(id, kind)` is the conflict key. A row whose `(id, kind)` already exists has its `properties` map, `label`, and `source_path` replaced; `updated_at` is set to `Utc::now()`; `created_at` is preserved. The returned `usize` is the number of NEW rows (an update is `0`).

`upsert_edges` semantics: `(source, target, kind)` is the conflict key. Updates replace `confidence`, `provenance`, and `metadata`. Self-loops and out-of-range `confidence` are rejected pre-DB (using `GraphEdge::new`'s invariant checks — the trait method does NOT re-implement them).

PG migration adds:

```sql
ALTER TABLE graph_nodes ADD CONSTRAINT graph_nodes_id_kind_unique UNIQUE (id, kind);
ALTER TABLE graph_edges ADD CONSTRAINT graph_edges_stk_unique UNIQUE (source, target, kind);
```

The new methods are transactional: one `BEGIN; … COMMIT;` per batch call. The dispatch helpers in `mcp.rs` (both `docs_ingest` and `issues_ingest`) call `upsert_nodes` first, then `upsert_edges`, in two separate transactions (a failure on the second does NOT roll back the first — acceptable for V1 because the re-ingest is idempotent and the second call simply observes the partial state).

## 6. MCP Tool — `issues_ingest`

Follows `dispatch_docs_ingest` line-for-line:

```rust
#[cfg(feature = "multimodal")]
pub const TOOL_ISSUES_INGEST: &str = "issues_ingest";

#[cfg(feature = "multimodal")]
async fn dispatch_issues_ingest(
    service: &Arc<ExplorerService>,
    graph_repo: Option<&Arc<dyn crate::ports::graph_repository::GraphRepository>>,
    arguments: serde_json::Value,
) -> CallToolResult { … }
```

The signature adds `graph_repo` (a new parameter on the `dispatch` function's match arm) because `issues_ingest` is the first multimodal tool that needs the WRITE path. `docs_ingest` keeps its old signature for now — its write path upgrade is a follow-up that piggy-backs on the same `GraphRepository` change.

The dispatch helper:
1. Parses + validates the `IssuesIngestArgs` struct.
2. Resolves the `mode` (URL → `github` default; path → `git_log` default; explicit wins).
3. Constructs an `IssuesExtractor` (with a `GitHubClient` chosen by feature + env).
4. Calls `.extract(source)`.
5. On `Ok(nodes)`, fans out `graph_repo.upsert_nodes(...)` and `graph_repo.upsert_edges(...)`.
6. Returns the `McpResultEnvelope` payload `{nodes_created, edges_created, issues_skipped, errors}`.

The schema entry mirrors `TOOL_DOCS_INGEST` — same gating, same description style, same `McpResultEnvelope` payload.

## 7. CLI — `cognicode issues-ingest`

`execute_issues_ingest(source, mode, max_issues)` mirrors `execute_docs_ingest` exactly:
- Construct the extractor.
- Call `extract`.
- Print a human-readable summary table to stdout (`source | kind | nodes | edges | confidence_avg`).
- Exit `0` on full success, `1` on partial failure (one or more issues errored but at least one succeeded).
- The CLI does NOT have a `GraphRepository` (the CLI is a read-only consumer of the local workspace) — it prints what WOULD be persisted, and the user is expected to run the MCP tool or the API path to actually write.

## 8. Feature Gate

Every new module, every new trait method, every new MCP constant is gated behind `#[cfg(feature = "multimodal")]`. The build matrix:

| Build | `IssuesExtractor` present? | `issues_ingest` tool listed? | `octocrab` pulled in? |
|-------|---------------------------|------------------------------|----------------------|
| `cargo build` (default) | ❌ | ❌ | ❌ |
| `cargo build --features multimodal` | ✅ | ✅ | ✅ |
| `cargo build -p cognicode-mcp --no-default-features` | ❌ | ❌ (regression test) | ❌ |

The `octocrab` dep is declared `optional = true` and the `multimodal` feature is extended to include `dep:octocrab`. The default build is byte-for-byte unchanged.

## 9. Protocol C — Information Bottleneck Check

The IB check asks: for each port, does the interface reveal only what the caller needs (I(X;T) ≈ I(T;Y))? We audit the three new / extended ports:

### 9.1 `SourceExtractor` (existing — used unchanged by `IssuesExtractor`)

| Aspect | I(X;T) (caller sees) | I(T;Y) (caller needs) | Verdict |
|--------|---------------------|----------------------|---------|
| Method `extract(&self, source: SourcePath) -> Result<Vec<ExtractedNode>>` | full `ExtractedNode` list | one node + edges per extracted unit | **Tight** — caller iterates `Vec`, picks what it needs; no opaque internals leak |
| `SourcePath::Url(String)` | string | URL (octocrab parses it inside the extractor) | **Loose — but acceptable** — the URL is passed through verbatim to the GitHub API; the extractor is the only consumer. The IB cost is one branch in the URL parser. Documented in the spec. |

### 9.2 `GraphRepository` (extended)

| Method | I(X;T) (caller sees) | I(T;Y) (caller needs) | Verdict |
|--------|---------------------|----------------------|---------|
| `upsert_nodes(Vec<GraphNode>) -> Result<usize>` | full node struct | the row to insert/update | **Tight** — the caller's `GraphNode` is the canonical row. `usize` is the minimal "how many new" signal. |
| `upsert_edges(Vec<GraphEdge>) -> Result<usize>` | full edge struct | the row to insert/update | **Tight** — same as nodes. |
| Read methods (`search`, `find_nodes_by_kind`, `get_node`, `find_outgoing_edges`) | unchanged | unchanged | **Untouched** — the addition is purely additive. |

**No connascence of value > 3.0 bits was introduced** by the new write methods.

### 9.3 `GitHubClient` (new — DIP wrapper)

| Method | I(X;T) (caller sees) | I(T;Y) (caller needs) | Verdict |
|--------|---------------------|----------------------|---------|
| `list_issues(&self, owner, repo, state) -> Result<Vec<RawIssue>, GitHubError>` | a flat `RawIssue` struct | the same `RawIssue` struct (the extractor is the only consumer) | **Tight** — the trait exposes the minimum data the extractor needs. `RawIssue` is a V1-internal DTO, not a public type. |

The alternative — exposing `octocrab::models::issues::Issue` directly — would leak the GitHub API schema into the domain. The `RawIssue` wrapper shields the domain from upstream churn (GitHub adds new fields → no domain code changes). **Connascence of value to `octocrab` is reduced from ~2.0 bits (the proposal's estimate) to ~0.5 bits.**

## 10. Entropy Summary (Protocol B)

| Connascence pair | Type | I(bits) | Severity | Mitigation in this design |
|------------------|------|---------|----------|---------------------------|
| `IssuesExtractor` ↔ `SourceExtractor` | Type | ~1.0 | Medium | Trait contract, no change |
| `IssuesExtractor` ↔ `NodeKind::Issue` | Name | ~0.5 | OK | Shared `kind_prefix` fn (copied from `docs_extractor`) |
| `IssuesExtractor` ↔ `octocrab` | Value | ~0.5 (was ~2.0) | OK | `GitHubClient` trait (DIP) |
| `issue_properties` ↔ frontend | Meaning | ~0.8 | Medium | Shared `ISSUE_*_PROPERTIES` const arrays; Zod schema generation is a follow-up |
| `issues_ingest` ↔ `docs_ingest` | Algorithm | ~1.5 | Medium | Table-driven tool registration is a follow-up refactor (flagged in the spec) |
| `GraphRepository::upsert_*` ↔ PG schema | Value | ~1.0 | OK | PG migration adds the unique constraints; the trait is the contract |

**Coupling score: ~0.9 bits avg (Low).** No critical pairs (>3.0 bits).

## 11. Test Strategy

| Layer | Test type | Tool | Coverage |
|-------|----------|------|----------|
| Domain — `issue_properties::validate_issue_properties` | Unit | `cargo test` | 6 cases (all required, missing key, bad status, bad tracker, case-folding, label comma) |
| Domain — `issues_confidence_rules::ConfidenceTier` | Unit | `cargo test` | 8 cases (4 tier values, 4 provenance tags, idempotency, case-insensitivity) |
| Domain — `commit_issue_parser::parse_commit_issue_refs` | Unit | `cargo test` | 12 cases (Fixes / Closes / Resolves / Refs / Part-of / See, multi-pattern per commit, case-fold, `#0`, empty log) |
| Port — `GraphRepository` mock | Unit | `cargo test` | 6 cases (insert, re-ingest, self-loop, empty vec, batch rollback, idempotency) |
| Adapter — `IssuesExtractor` | Unit + integration | `cargo test --features multimodal` | 8 cases (URL → issues, Directory → commit refs, Unsupported File, GHE rejected, missing token, rate limit, body mentions, idempotency) |
| Trigger — `dispatch_issues_ingest` | Unit | `cargo test --features multimodal` | 5 cases (happy path, invalid_input, github_auth_required, github_rate_limited, issues_ingest_unavailable) |
| Trigger — `execute_issues_ingest` (CLI) | Integration | `assert_cmd` | 2 cases (success, partial failure) |
| Regression — `tools/list` | Unit | `cargo test --features multimodal` | 2 cases (30 off, 31 on) |
| PG adapter | Integration | `cargo test --features multimodal,postgres` (CI only) | 1 case: `INSERT … ON CONFLICT` round-trip |

**Target: ≥ 80% line coverage on the new code paths**, with the protocol gate of "every new public function has at least one passing test" being the floor.

## 12. Rollback

1. `git revert` the merge commit. The change is self-contained.
2. `octocrab` dep is removed; the build graph shrinks to its pre-change state.
3. The `multimodal` feature is restored to its pre-change scope.
4. The new migration (`graph_nodes` + `graph_edges` unique constraints) is reversible with a one-line `ALTER TABLE … DROP CONSTRAINT`.

## 13. Out of Scope (Design-level)

- Multi-tracker support (the `ISSUE_TRACKERS` const reserves the slots; no impl is shipped)
- Issue body → `Evidence` node (V2)
- Issue comments → `Evidence` nodes
- Webhook / polling
- GitHub Enterprise hostnames
- Cross-repo federation
- Zod schema generation from the Rust const arrays (frontend hand-codes the keys for V1)
