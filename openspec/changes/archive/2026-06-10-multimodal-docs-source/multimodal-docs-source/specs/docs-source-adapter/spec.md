# docs-source-adapter Specification (NEW)

## Purpose

A pluggable ingestion pipeline for documentation, ADR, and Markdown source files. The `SourceExtractor` trait abstracts the contract `given a source path, produce extracted nodes and edges`. `DocsExtractor` is the first implementation, targeting `.md` files (including ADR front-matter), using `DocsConfidenceRules` to score edge confidence. The adapter is exposed as a CLI command (`cognicode ingest-docs`) and an MCP tool (`docs_ingest`).

## Ports and Adapters

| Component | Location | Role |
|-----------|----------|------|
| `SourceExtractor` trait | `crates/cognicode-explorer/src/ports/source_extractor.rs` | Port: `async fn extract(&self, source: SourcePath) -> Result<Vec<ExtractedNode>>` |
| `SourcePath` enum | same | `LocalPath(PathBuf) \| GitBlob { repo, sha, path }` |
| `ExtractedNode` | same | `(GraphNode, Vec<GraphEdge>)` — one node + its outbound edges |
| `DocsExtractor` | `crates/cognicode-core/src/domain/services/docs_extractor.rs` | Adapter: implements `SourceExtractor` for `.md`/`.mdx` |
| `DocsConfidenceRules` | `crates/cognicode-core/src/domain/services/docs_confidence.rs` | Pure functions mapping parse signals to confidence |
| `docs_source_adapter` | `crates/cognicode-explorer/src/adapters/docs_source_adapter.rs` | Wires `DocsExtractor` to `GenericGraphRepository` |
| MCP tool `docs_ingest` | `crates/cognicode-explorer/src/mcp.rs` | Trigger |
| CLI `cognicode ingest-docs` | `crates/cognicode/src/main.rs` | Trigger |

## Requirements

### Requirement: SourceExtractor Trait Contract

`SourceExtractor` MUST be an `async` trait with one method: `async fn extract(&self, source: SourcePath) -> Result<Vec<ExtractedNode>, ExtractionError>`. The trait MUST be dyn-compatible (no generic methods, no `Self` in argument/return types beyond the receiver). All implementations MUST be `Send + Sync` so they can run in parallel via `tokio::spawn`.

#### Scenario: Trait method signature is dyn-compatible
- GIVEN `let extractor: Box<dyn SourceExtractor> = Box::new(DocsExtractor::new(rules))`
- WHEN `extractor.extract(SourcePath::LocalPath("docs/x.md".into())).await` is called
- THEN it MUST return `Result<Vec<ExtractedNode>, _>`

#### Scenario: Send + Sync requirement
- GIVEN an extractor wrapped in `Arc<dyn SourceExtractor>`
- WHEN `tokio::spawn(async move { extractor.extract(path).await })` is invoked
- THEN the future MUST compile (Send bound satisfied)

### Requirement: DocsConfidenceRules

`DocsConfidenceRules` MUST be a struct of pure scoring functions, exposed as constants for the 4 rules:

| Rule | Trigger | Confidence | Rationale |
|------|---------|-----------|-----------|
| `link_exact` | Markdown link `[text](path/to/file.rs#L10)` resolves to a known symbol | 0.9 | Explicit reference |
| `link_fuzzy` | Link or heading matches a symbol by FTS5 fuzzy search | 0.6 | Probable reference |
| `heading_match` | Doc heading matches a symbol name (case-insensitive) | 0.7 | Topic-level reference |
| `unresolved` | Reference cannot be resolved | 0.3 | Best-effort placeholder |

#### Scenario: link_exact returns 0.9
- GIVEN a `.md` content with `[`render`](src/render.rs#L42)` and `src/render.rs:render:42` is in the symbol index
- WHEN the extractor walks the link
- THEN the emitted edge has `confidence == 0.9` and `provenance == Provenance::Extracted`

#### Scenario: link_fuzzy returns 0.6
- GIVEN a link `[parser](src/parse.rs)` and the symbol index has only `parse_tree` (FTS5 match score 0.55)
- WHEN fuzzy resolution fires
- THEN the emitted edge has `confidence == 0.6` and `provenance == Provenance::Ambiguous`

#### Scenario: heading_match returns 0.7
- GIVEN a heading `# Auth Service` and a symbol `auth_service` exists
- WHEN the heading-to-symbol matcher runs
- THEN the emitted edge has `confidence == 0.7`

#### Scenario: unresolved returns 0.3
- GIVEN a link `[mystery](src/nowhere.rs)` with no matching symbol
- WHEN extraction proceeds
- THEN the edge has `confidence == 0.3` and `provenance == Provenance::Ambiguous`

### Requirement: DocsExtractor Markdown Parsing

`DocsExtractor::extract(source)` MUST:

1. Read the file (or git blob) as UTF-8.
2. Detect ADR front-matter (`---\n...status: accepted...\n---`).
3. Parse Markdown to extract: H1/H2 headings, Markdown links `[text](url)`, code fences ` ```lang `, and inline backticks.
4. For each heading → emit a `GraphNode { kind: NodeKind::Doc, label, source_path, metadata: { section, line } }`.
5. For each link → resolve via `DocsConfidenceRules` → emit a `GraphEdge { kind: EdgeKind::Cites, ... }`.
6. For ADR front-matter `status: accepted` → emit a `GraphNode { kind: NodeKind::Decision, metadata: { status, date } }` and link the decision to the file's content node via `EdgeKind::Justifies`.
7. Return `Vec<ExtractedNode>` — one entry per top-level node.

#### Scenario: Plain markdown produces Doc nodes
- GIVEN `docs/guide.md` containing `# Guide\nSee [render](src/render.rs#L10)`
- WHEN extracted
- THEN exactly one `GraphNode` with `kind == NodeKind::Doc` is produced
- AND one `GraphEdge` with `kind == EdgeKind::Cites` and target = the resolved `Symbol`

#### Scenario: ADR front-matter produces Decision node
- GIVEN `docs/adr/0001.md` starting with `---\nstatus: accepted\ndate: 2026-01-15\n---`
- WHEN extracted
- THEN a `GraphNode { kind: NodeKind::Decision, metadata: { status: "accepted", date: "2026-01-15" } }` is produced
- AND a `GraphEdge { kind: EdgeKind::Justifies }` connects it to the file's Doc node

#### Scenario: Code fence with language is preserved
- GIVEN ```` ```rust\nfn main() {}\n``` ```` in a doc
- WHEN extracted
- THEN the code block's language is recorded in the Doc node's metadata (`{ code_block_lang: "rust" }`)

### Requirement: docs_ingest MCP Tool

The MCP tool `docs_ingest` MUST be registered in the explorer group of `crates/cognicode-explorer/src/mcp.rs`. Input schema:

```json
{
  "type": "object",
  "properties": {
    "paths": { "type": "array", "items": { "type": "string" } },
    "recursive": { "type": "boolean", "default": true }
  },
  "required": ["paths"]
}
```

Output schema (the IngestionSummary):

```json
{
  "type": "object",
  "properties": {
    "files_scanned": { "type": "integer" },
    "nodes_created": { "type": "integer" },
    "edges_created": { "type": "integer" },
    "edges_ambiguous": { "type": "integer" },
    "duration_ms": { "type": "integer" }
  }
}
```

The tool MUST call `DocsSourceAdapter::ingest(paths, recursive)` which walks the filesystem, invokes `DocsExtractor` per `.md` file, and writes to `GenericGraphRepository`.

#### Scenario: Ingest a directory of markdown
- GIVEN `docs/` containing 5 `.md` files (2 ADRs, 3 guides)
- WHEN `docs_ingest` is called with `{ "paths": ["docs"], "recursive": true }`
- THEN the response includes `files_scanned == 5`, `nodes_created == 5` (2 Decision + 3 Doc), and `edges_created` reflects resolved links

#### Scenario: Non-markdown files are skipped
- GIVEN a directory with `.rs`, `.toml`, and `.md` files
- WHEN `docs_ingest` is called
- THEN only `.md` files are scanned; `files_scanned` reflects markdown only

#### Scenario: Empty path array rejected
- GIVEN `docs_ingest` called with `{ "paths": [] }`
- WHEN validated
- THEN the tool returns a schema error mentioning `paths must be non-empty`

### Requirement: CLI Command `cognicode ingest-docs`

The CLI MUST expose `cognicode ingest-docs <PATH>... [--recursive] [--no-recursive]`. Default behavior is recursive. Output is a human-readable summary table printed to stdout, plus a non-zero exit code on partial failure (some files failed but at least one succeeded).

#### Scenario: Ingest current directory
- GIVEN `./docs` exists with markdown files
- WHEN `cognicode ingest-docs docs` runs
- THEN stdout shows a table with `file | kind | nodes | edges | confidence_avg`
- AND exit code is 0 on full success

#### Scenario: Partial failure exits non-zero
- GIVEN `./docs` contains one valid `.md` and one with an invalid UTF-8 byte sequence
- WHEN the CLI runs
- THEN the valid file is ingested; the invalid file is logged to stderr; exit code is 1

### Requirement: Ingestion Idempotency

Re-running `docs_ingest` over the same paths MUST be idempotent: nodes upsert (`ON CONFLICT (id, kind) DO UPDATE`), edges upsert. The `files_scanned` count MAY grow (new files), but `nodes_created` MUST NOT double-count existing nodes.

#### Scenario: Re-ingest does not duplicate nodes
- GIVEN `docs/adr/0001.md` was ingested previously (1 Decision + 1 Doc + 3 edges)
- WHEN `docs_ingest` runs again on the same file
- THEN `nodes_created` reports 0 (or only the diff, not the original)
- AND the row count in `graph_nodes` is unchanged for the existing IDs

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| File is not valid UTF-8 | Skip file, log warning, count in `files_failed` (additive to summary) |
| File has no headings (e.g., fragment-only `.md`) | Still emit a `Doc` node with `label == filename` and zero edges |
| Circular link (a doc cites itself) | Emit one edge; do not loop the extractor |
| ADR with status other than `accepted` (e.g., `proposed`, `superseded`) | Still emit a `Decision` node; status stored verbatim in metadata |
| Markdown link with URL fragment (`#section`) | Resolve the file's anchor as the target; if anchor has no symbol, fall back to `unresolved` (0.3) |
| Two `.md` files in different repos share the same basename | Different `NodeId` (full path is part of the ID); no collision |
| Link target is outside the indexed repo | Mark as `unresolved` (0.3); do not error |
| FTS5 index is empty (first ingestion in a fresh DB) | `link_fuzzy` produces no matches; `link_exact` may still work via filename guess; otherwise `unresolved` (0.3) |
| Path passed to CLI does not exist | Print error to stderr; exit code 1; do not run the extractor |
| Concurrent ingestion of the same file from two CLI invocations | Both run; PK collisions handled by `ON CONFLICT DO UPDATE`; final state is the same |

## Out of Scope

- PDF / HTML / DOCX ingestion — only `.md` and `.mdx`
- OCR or image-based docs
- Issue tracker adapters (Jira/GitHub) — Phase 5
- Cross-repo federation
- Re-extraction of code symbols — that path is the tree-sitter pipeline
- Confidence calibration (no Bayesian update over time)
- Watch mode / incremental file system events

## TDD RED Gate

Before any implementation, the following failing tests MUST exist:

1. `SourceExtractor` trait object compile test (Box<dyn SourceExtractor>)
2. `DocsConfidenceRules::link_exact/link_fuzzy/heading_match/unresolved` — one test per rule, exact confidence value asserted
3. `DocsExtractor` — at least 6 fixture tests: plain md, ADR, mixed links, code fence, no headings, circular link
4. `docs_ingest` MCP tool — schema validation test (3 cases: valid, empty paths, missing paths field)
5. CLI command — 2 integration tests (success + partial failure)
6. Idempotency test — re-ingest yields zero new nodes
7. Feature-gate test: `cargo build -p cognicode-explorer --no-default-features` compiles without `docs_ingest` tool

## Dependencies

- `generic-graph-model` capability (provides `NodeKind`, `GraphNode`, `GraphEdge`, `GenericGraphRepository`)
- Existing `Provenance` enum (3 variants — `Extracted`, `Inferred`, `Ambiguous`)
- Existing `InspectableObjectType::DecisionArtifact` (DTO surface — no conflict with `NodeKind::Decision` after rename to `EvidenceBlock` for the Evidence DTO variant)
- `tree-sitter` for code-fence language detection (already in dependency tree)
- `pulldown-cmark` or `markdown` crate for Markdown parsing — new dependency, declared under `multimodal` feature
