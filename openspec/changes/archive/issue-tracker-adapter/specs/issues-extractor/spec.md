# issues-extractor Specification (NEW)

## Purpose

`IssuesExtractor` is the third concrete implementation of the `SourceExtractor` port (`crates/cognicode-core/src/domain/traits/source_extractor.rs`), targeting GitHub Issues and git commit message references. It mirrors the `DocsExtractor` shape: a pure parsing function plus an async trait impl, feature-gated behind `multimodal`, and produces `NodeKind::Issue` nodes with `EdgeKind::Resolves` / `EdgeKind::Dependency(References)` edges. Two source modes are supported — `SourcePath::Url` for the GitHub REST API and `SourcePath::Directory` for local git log scanning. The two modes compose: a `url:owner/repo` source can be paired with a `directory:local_path` source to enrich API issues with commit references, but the extractor only consumes one path at a time per call.

## Requirements

### Requirement: IssuesExtractor implements SourceExtractor

`IssuesExtractor` MUST implement `SourceExtractor` for the `source_kind() == "github_issues"`. It MUST be `Send + Sync`, dyn-compatible (`Box<dyn SourceExtractor + Send + Sync>`), and gated behind `#[cfg(feature = "multimodal")]`. The struct MUST be stateless (`#[derive(Default, Clone)]`) — all per-call state is passed via `SourcePath`.

#### Scenario: IssuesExtractor dyn-compatible
- GIVEN `let e: Box<dyn SourceExtractor + Send + Sync> = Box::new(IssuesExtractor::new())`
- WHEN `e.source_kind()` is called
- THEN it returns `"github_issues"`

#### Scenario: Feature-gated compile
- GIVEN `cargo build -p cognicode-core --no-default-features` (no `multimodal`)
- THEN `IssuesExtractor` is not in the symbol table
- AND the binary does not pull `octocrab` / `regex-git` deps

### Requirement: GitHub URL extraction via octocrab

When `extract` is called with `SourcePath::Url("https://github.com/{owner}/{repo}")` (or any path whose first segment is `github.com`), the extractor MUST fetch all open + closed issues via the GitHub REST API, paginated, and emit one `NodeKind::Issue` per issue. The `NodeId` MUST be `issue:github/{owner}/{repo}#{number}` (deterministic, re-ingest-safe). A `GitHubClient` trait MUST abstract the HTTP layer (DIP) so unit tests inject a mock without network I/O.

#### Scenario: 100 issues become 100 Issue nodes
- GIVEN a mocked `GitHubClient` returning 100 issues for `acme/widgets`
- WHEN `extract(SourcePath::Url("https://github.com/acme/widgets"))` is called
- THEN the result has exactly 100 `ExtractedNode`s, each with `kind == NodeKind::Issue`
- AND each `id` starts with `issue:github/acme/widgets#`

#### Scenario: Missing GITHUB_TOKEN surfaces clear error
- GIVEN no `GITHUB_TOKEN` env var and the GitHub API returns 401/403
- WHEN the extractor reaches the first 4xx response
- THEN it returns `Err(SourceExtractorError::Internal("github api: token required (set GITHUB_TOKEN)"))` and stops

### Requirement: Git log parsing for commit-issue references

When `extract` is called with `SourcePath::Directory(path_to_git_repo)`, the extractor MUST run `git log --all --pretty=format:%H%x1f%s%x1f%b` and parse commit subjects + bodies for the patterns `Fixes #N`, `Closes #N`, `Resolves #N`, `Refs #N`, `Part of #N` (case-insensitive, multi-pattern per commit). Each match becomes a `Resolves` or `References` edge from a synthetic commit node (`commit:{sha_short}`) to `issue:github/{owner}/{repo}#{N}` (the `owner/repo` is read from `git remote get-url origin`).

#### Scenario: "Fixes #42" in a commit subject
- GIVEN a git log entry with subject `Fixes #42: handle null pointer`
- WHEN the parser scans it
- THEN it emits a `Resolves` edge from `commit:abc1234` to `issue:github/acme/widgets#42` with `confidence == 0.85`

#### Scenario: Case-insensitive matching
- GIVEN a commit subject `FIXES #7 — typo in readme`
- WHEN the parser scans it
- THEN it emits a `Resolves` edge to `issue:github/acme/widgets#7`

#### Scenario: Multiple references in one commit
- GIVEN `Closes #10, Refs #11, Refs #12`
- WHEN the parser scans it
- THEN it emits 3 edges (1 `Resolves`, 2 `References`) from the same commit node

### Requirement: SourcePath dispatch

The extractor MUST support `SourcePath::Url` (GitHub API) and `SourcePath::Directory` (git log). `SourcePath::File` MUST return `SourceExtractorError::Unsupported` (issues are never a single file).

#### Scenario: File path is unsupported
- GIVEN `extract(SourcePath::File("/tmp/something"))`
- WHEN called
- THEN it returns `Err(SourceExtractorError::Unsupported("issues extractor requires Url (github.com) or Directory (git repo)"))`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Issue body contains `file:name:line` code refs | Emit a `References` edge per match to a `Symbol` `NodeId` (confidence 0.5, see `issues-confidence-rules`) |
| GitHub API rate limit (403 with `X-RateLimit-Remaining: 0`) | Return `Internal("github api: rate limit exceeded; set GITHUB_TOKEN to increase to 5000/hr")` |
| Local git repo has no remote | `Directory` mode still runs; commit-issue edges have `issue:github/<unknown>/<unknown>#N` and the frontend flags them as unresolved |
| Commit message is non-UTF-8 | `tracing::warn!` and skip; never panic |
| Pagination over 1000 issues | Hard cap at 1000 per call to prevent runaway; document in tool description |
| Issue has no `assignee` (GitHub returns `null`) | Property `assignee` is absent from the node (not stored as empty string) |
| `octocrab` is unavailable in the build (default features off) | `extract` returns `Unsupported` rather than panicking — the `octocrab` dep is gated under `multimodal` |

## Out of Scope

- Jira, GitLab, Linear adapters (Phase 6)
- Issue comments → Evidence nodes
- Webhook / polling for real-time updates
- GitHub Enterprise custom hostnames (`HOSTNAME/api/v3` URLs)
- Cross-repo issue federation
- Issue body attachment downloads (images, files)

## TDD RED Gate

1. `IssuesExtractor` dyn-compatibility compile test
2. `parse_github_issues` (pure fn, mocked JSON) — 4 cases: open issue, closed issue, body with code refs, no assignee
3. `parse_commit_issue_refs` (pure fn, mocked git log lines) — 6 cases: Fixes, Closes, Resolves, Refs, Part-of, multiple in one commit
4. Confidence tier mapping — 4 tests asserting exact confidence values
5. `SourcePath` dispatch — 3 tests: Url, Directory, File (Unsupported)
6. `GITHUB_TOKEN` missing — error envelope test
7. Feature-gate test: `cargo build --no-default-features` excludes `IssuesExtractor`
8. Idempotency test: re-extracting the same URL produces the same `NodeId`s

## Dependencies

- `docs-source-adapter` (precedent: `DocsExtractor` shape, `DocsConfidenceRules` module layout)
- `generic-graph-model` (provides `NodeKind::Issue`, `EdgeKind::Resolves`, `EdgeKind::Dependency(References)`)
- `multimodal` Cargo feature (gates every extractor symbol)
- `octocrab = "0.43"` (new dep, optional, gated behind `multimodal`)
- `regex` (workspace dep, already present; reuse for commit-message patterns)
- `GITHUB_TOKEN` env var (optional; unauthenticated requests get 60/hr)
