# issue-node-properties Specification (NEW)

## Purpose

`NodeKind::Issue` already exists (`as_str() == "issue"`, `from_str("issue")`), but the property schema an `Issue` `GraphNode` carries is unstated. This spec freezes the V1 property set so the extractor, the persistence layer, the MCP tool, and the frontend all agree on the shape. The schema lives in a single shared const module (`crates/cognicode-core/src/domain/value_objects/issue_properties.rs`) — extractor writes, frontend reads. Drifting the schema is a compile error, not a runtime surprise.

## Requirements

### Requirement: Issue Property Schema

An `Issue` `GraphNode` MUST carry the following `properties` entries. The key set is **frozen**; the values are free-form strings. Missing properties are valid (e.g. an issue with no assignee omits the key — the property is absent, not an empty string).

| Key | Type | Source | Example | Required? |
|-----|------|--------|---------|-----------|
| `number` | integer (string-encoded) | GitHub API | `"42"` | **Required** |
| `title` | string | GitHub API | `"Null pointer in render path"` | **Required** |
| `status` | enum string | GitHub API | `"open"` \| `"closed"` | **Required** |
| `url` | URL string | GitHub API | `"https://github.com/acme/widgets/issues/42"` | **Required** |
| `labels` | comma-joined string | GitHub API | `"bug,p1,area:render"` | Optional |
| `assignee` | string | GitHub API | `"alice"` | Optional (absent if unassigned) |
| `author` | string | GitHub API | `"bob"` | Optional |
| `created_at` | RFC 3339 | GitHub API | `"2026-06-10T13:00:00Z"` | Optional |
| `updated_at` | RFC 3339 | GitHub API | `"2026-06-10T15:30:00Z"` | Optional |
| `tracker` | enum string | Extractor's runtime choice | `"github"` | **Required** |
| `repo` | string | Extractor (from URL) | `"acme/widgets"` | **Required** |

#### Scenario: Required keys are always present
- GIVEN any `Issue` `GraphNode` produced by `IssuesExtractor`
- WHEN `node.properties.keys()` is enumerated
- THEN `number`, `title`, `status`, `url`, `tracker`, `repo` are always present
- AND no required key is empty

#### Scenario: Optional keys are absent when N/A
- GIVEN an issue with no assignee in GitHub
- WHEN the extractor builds the node
- THEN `properties.get("assignee")` returns `None` (the key is absent, not `Some("")`)

#### Scenario: Labels are comma-joined
- GIVEN an issue with labels `["bug", "p1", "area:render"]`
- WHEN the extractor builds the node
- THEN `properties["labels"] == "bug,p1,area:render"`
- AND a label containing a comma is rejected at parse time with `Err(SourceExtractorError::Internal("label contains comma: 'a,b'"))`

### Requirement: Tracker Enum

`tracker` MUST be one of the stable string literals: `"github"` (V1), `"gitlab"`, `"linear"`, `"jira"` (the last three are reserved for V2 — values are accepted but the corresponding adapters are not yet implemented). The extractor MUST refuse to start if a `SourcePath::Url` host is not in the known set.

#### Scenario: Unknown host rejected
- GIVEN `SourcePath::Url("https://bitbucket.org/acme/widgets")`
- WHEN the extractor parses it
- THEN it returns `Err(SourceExtractorError::Unsupported("issues extractor: host 'bitbucket.org' not in V1 (github only)"))`

#### Scenario: GitHub URL parses tracker="github"
- GIVEN `SourcePath::Url("https://github.com/acme/widgets")`
- WHEN the extractor parses it
- THEN the resulting `Issue` node has `properties["tracker"] == "github"` and `properties["repo"] == "acme/widgets"`

### Requirement: NodeId Convention

Every `Issue` `GraphNode` MUST have `id == "issue:{tracker}/{repo}#{number}"` (e.g. `"issue:github/acme/widgets#42"`). The id is **deterministic** — re-ingesting the same tracker+repo+number produces the same id, which is what makes the upsert in `graph-repository-write` idempotent.

#### Scenario: Determinism
- GIVEN two `IssuesExtractor` invocations on the same `https://github.com/acme/widgets`
- WHEN both produce issue #42
- THEN both nodes have `id == "issue:github/acme/widgets#42"`
- AND `graph_repository.upsert_nodes` collapses the duplicate

#### Scenario: Cross-repo isolation
- GIVEN `acme/widgets#42` and `acme/gadgets#42`
- WHEN both are ingested
- THEN they produce different `NodeId`s (`issue:github/acme/widgets#42` vs `issue:github/acme/gadgets#42`)

### Requirement: Issue Property Constants Module

The schema MUST be exported as `const` arrays in `crates/cognicode-core/src/domain/value_objects/issue_properties.rs`:

```rust
pub const ISSUE_REQUIRED_PROPERTIES: &[&str] = &["number", "title", "status", "url", "tracker", "repo"];
pub const ISSUE_OPTIONAL_PROPERTIES: &[&str] = &["labels", "assignee", "author", "created_at", "updated_at"];
pub const ISSUE_TRACKERS: &[&str] = &["github", "gitlab", "linear", "jira"];
pub const ISSUE_STATUSES: &[&str] = &["open", "closed"];
```

The module is `#[cfg(feature = "multimodal")]`-gated. A free function `validate_issue_properties(&HashMap<String, String>) -> Result<(), String>` MUST check that every required key is present, that `status` is in `ISSUE_STATUSES`, and that `tracker` is in `ISSUE_TRACKERS`. The function is called by `IssuesExtractor` before emitting a node (fail-fast).

#### Scenario: Missing required key fails fast
- GIVEN an `Issue` candidate with no `url` property
- WHEN `validate_issue_properties` is called
- THEN it returns `Err("missing required issue property: url")`
- AND the extractor returns `Err(SourceExtractorError::Internal("invalid issue candidate: missing required issue property: url"))`

#### Scenario: Unknown status rejected
- GIVEN `properties["status"] == "in_progress"`
- WHEN `validate_issue_properties` is called
- THEN it returns `Err("unknown issue status: 'in_progress' (expected: open | closed)")`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Issue title is 256 chars | Truncated to 256 chars (max) by the extractor; a `truncated_title: true` property is added |
| Issue body has 10000+ chars | Not stored in the `GraphNode` (V1: title + metadata only; body → Evidence node is V2) |
| Issue URL has a query string (`?q=is:pr`) | Strip the query string before persisting the `url` property |
| Issue number is negative | Reject at parse time; GitHub issue numbers are non-negative integers |
| Issue number exceeds `i32::MAX` | Persist verbatim as a string (PG column is `text`, not `integer`) |
| `created_at` is in the future (clock skew) | Accept; the frontend does not validate timestamps |
| `labels` list is empty | The `labels` key is absent (not `Some("")`) |
| `assignee` is a bot account | Persist verbatim; the frontend renders the badge normally |
| Two issues share a title | Allowed; the `NodeId` is the disambiguator (the title is a label, not a key) |
| `repo` contains a slash (`acme/widgets`) | Persist verbatim; the slash is part of the id, not a path separator |

## Out of Scope

- Issue body → `Evidence` node (V2: will close `docs-source-adapter` ↔ `issue-tracker-adapter` loop)
- Issue comments → `Evidence` nodes
- Linked PRs (`#41 closes #42` cross-references) — the `Resolves` edge covers this in V1
- Reactions, milestones, projects — V2 metadata
- Cross-tracker federation (e.g. a Linear issue mirrored to a GitHub issue)

## TDD RED Gate

1. `ISSUE_REQUIRED_PROPERTIES` const length test
2. `ISSUE_TRACKERS` membership test — 4 cases (github, gitlab, linear, jira)
3. `validate_issue_properties` — 4 cases: all required present, missing `url`, bad `status`, bad `tracker`
4. `NodeId` convention test — 3 cases: determinism, cross-repo isolation, unknown host rejected
5. `tracker` extraction test — 4 cases: `github.com` → `github`, `ghe.acme.com` → rejection, no host → rejection, mixed-case `GitHub.com` → normalised to `github`
6. `status` normalisation test — `OPEN` → `open` (case-folded)
7. Compile-gate test: module is absent under `--no-default-features`

## Dependencies

- `generic-graph-model` (provides `GraphNode::properties` HashMap)
- `docs-source-adapter::docs_confidence_rules` (precedent for a `#[cfg(feature = "multimodal")]`-gated property schema module)
- `multimodal` Cargo feature
- `octocrab` types (`octocrab::models::issues::Issue`) — gated behind `multimodal`

## Risks

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| GitHub adds new issue fields the frontend wants to render | High (V1 lifespan) | The properties HashMap is open-ended; new keys are additive, never breaking |
| `tracker` enum grows faster than the const list | Medium | The validate function is the single source of truth — adding a tracker is a one-line change |
| Frontend hard-codes the property keys | Medium | Generate the frontend's `IssueProperty` Zod schema from the Rust const arrays (out of scope for V1, flagged for follow-up) |
