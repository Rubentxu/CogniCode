# Roadmap: Ingest Pipeline — From Zero Graph to Live Multi-Language Graph

> **ADRs**: [ADR-017](../adr/ADR-017-pg-native-ingest-pipeline.md) ·
> [ADR-018](../adr/ADR-018-languageconfig-data-driven-parser.md) ·
> [ADR-019](../adr/ADR-019-legacy-tables-as-views.md) ·
> [ADR-020](../adr/ADR-020-workspace-scoped-schema.md) ·
> [ADR-021](../adr/ADR-021-streaming-bounded-mpsc.md) ·
> [ADR-022](../adr/ADR-022-pg-trigger-notify-incremental-refresh.md) ·
> [ADR-023](../adr/ADR-023-bulk-load-advisory-lock-error-isolation.md) ·
> [ADR-024](../adr/ADR-024-infrastructure-as-code-extraction.md) ·
> [ADR-025](../adr/ADR-025-mcp-dual-mode-standalone-pg.md) ·
> [ADR-026](../adr/ADR-026-graphify-style-mcp-tools.md)

This roadmap defines four sprints to deliver the ingest pipeline. Each sprint
ships independently and builds on the previous one. Sprint 1 closes the
Explorer loop (scan → serve). Sprint 2 adds analysis. Sprint 3 scales
languages. Sprint 4 adds incremental + robustness.

---

## Sprint 1: Close the Loop (Explorer scan → serve)

**Goal:** A user opens a workspace in the Explorer, clicks "Scan", and the
graph is built and served. Code-only, 6 existing languages.

### Schema work

| Task | ADR | Detail |
|------|-----|--------|
| Add `workspace_id` to `graph_nodes`, `graph_edges` | ADR-020 | ALTER TABLE + DEFAULT 'default' |
| Create `scan_manifest` table | ADR-020 | `{workspace_id, file_path, content_hash, mtime, status}` |
| Replace `symbols` table → VIEW | ADR-019 | `CREATE VIEW symbols AS SELECT ... FROM graph_nodes WHERE kind LIKE 'symbol.%'` |
| Replace `call_edges` table → VIEW | ADR-019 | `CREATE VIEW call_edges AS SELECT ... FROM graph_edges JOIN ...` |
| PG trigger `notify_graph_change()` | ADR-022 | Auto-NOTIFY on `graph_nodes` INSERT/UPDATE/DELETE |

### Pipeline stages

| Stage | Task | ADR | Detail |
|-------|------|-----|--------|
| **Scan** | Walk FS with `ignore` crate + `WalkFilter` | ADR-017 | `WalkBuilder` + classify by extension |
| | Compute SHA256 per file (rayon parallel) | ADR-017 | `sha2::Sha256` |
| | Compare against `scan_manifest` (mtime-first) | ADR-017 | `SELECT ... WHERE workspace_id = $1 AND file_path = ANY($2)` |
| | Output `Vec<FileChange>` | — | `{path, ChangeKind, content_hash}` |
| **Extract** | `LanguageConfig` for 6 existing languages | ADR-018 | Rust, Python, TS, JS, Go, Java configs |
| | Generic extractor: walk AST using config | ADR-018 | Functions, classes, calls, imports, contains |
| | Stream via bounded `mpsc` channel (cap 100) | ADR-021 | `rayon par_iter → blocking_send → tokio recv` |
| **PgUpsert** | Per-file transactional DELETE+INSERT | ADR-017 | `BEGIN; DELETE ... WHERE source_path=$1; INSERT ... ON CONFLICT; COMMIT;` |
| | Batch 10 files per transaction | ADR-021 | Accumulate in ingester before COMMIT |
| | Error isolation (`extract_safe`) | ADR-023 | Failed files → `scan_manifest.status = 'error'` |
| **Refresh** | Load CallGraph from PG (full load for v1) | — | `PostgresRepository::load_call_graph()` → `GraphCache::set()` |
| | `broadcast::send(GraphEvent::GraphReplaced)` | — | Existing GraphCache mechanism |

### API surface

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/workspaces/:id/scan` | POST | Start async scan job. Returns 202 + `{job_id}` |
| `/api/jobs/:job_id` | GET | Poll job status. `{stage, progress, scanned, total}` |
| `/api/workspaces/:id/graph/stats` | GET | `{symbol_count, edge_count, last_scan_at}` |

### CLI surface

```
cognicode scan [path]              # Full scan
cognicode scan [path] --force      # Force rebuild (ignore manifest)
```

### MCP adaptation (ADR-025)

| Task | ADR | Detail |
|------|-----|--------|
| Rewrite `build_graph` to use LanguageConfig | ADR-025 | Replace AnalysisService match-arms with generic extractor |
| Unify all analysis tools to read from GraphCache | ADR-025 | Fix dual-state bug: `GraphStore::load_graph()` → `GraphCache::get()` |
| Add `--postgres` flag + DATABASE_URL to MCP binary | ADR-025 | Mode B: `CogniCodeHandler::with_postgres()` |
| Mode B: load graph from PG on startup | ADR-025 | Same as Explorer's `open_graph_from_postgres` |
| Mode B: `build_graph` delegates to pipeline | ADR-025 | Scan → Extract → PgUpsert → Refresh |
| Deprecate `build_lightweight_index` | ADR-025 | Replaced by Scan stage manifest |
| Deprecate `merge_graphs` | ADR-025 | Replaced by PG as single source |

### Exit criteria

- [ ] POST `/api/workspaces/:id/scan` returns 202 with job_id
- [ ] Job tracks progress through stages (scan, extract, pg_upsert, refresh)
- [ ] After job completes, Explorer serves the graph (Spotter, Miller Columns, Interactive Graph)
- [ ] Re-scan with no changes completes in <1s (manifest compare only)
- [ ] `symbols` and `call_edges` VIEWs produce identical output to old tables
- [ ] Failed files are logged, scan continues
- [ ] **MCP Mode A: `build_graph` works standalone with LanguageConfig (no PG required)** (ADR-025)
- [ ] **MCP Mode B: `--postgres` flag loads graph from PG on startup** (ADR-025)
- [ ] **All MCP analysis tools read from GraphCache (dual-state bug fixed)** (ADR-025)

---

## Sprint 2: Analysis Integration

**Goal:** Every scan automatically produces communities, god nodes, dead code,
and a GraphReport. The Explorer shows insights without separate calls.

### Pipeline stages added

| Stage | Task | ADR | Detail |
|-------|------|-----|--------|
| **Resolve** | Cross-file call resolution via SQL | ADR-017 | `INSERT ... SELECT ... JOIN graph_nodes ON LOWER(label) = LOWER(callee)` |
| | Import-aware resolution (scope filtering) | — | CTE joining import edges to limit candidate callees |
| **Cluster** | Label Propagation community detection | — | `petgraph` Label Propagation on the in-memory graph |
| | Persist `community` as `graph_nodes.properties.community` | — | UPDATE via batch |
| **Analyze** | God nodes: PageRank top-N | — | `petgraph::algo::pagerank()` |
| | Surprising connections: cross-community edges | — | Filter edges where `community(source) != community(target)` |
| | Dead code: unreachable from entry points | — | `CallGraph::find_dead_code()` (existing) |
| | Hot paths: top fan-in | — | `CallGraph` fan-in index (existing) |
| **Report** | Generate `GraphReport` JSON | — | `{god_nodes, communities, surprising, dead_code, metrics}` |
| | Cache in `graph_reports` table | — | `CREATE TABLE graph_reports (id UUID, workspace_id, created_at, report JSONB)` |
| | Expose as `ContextualView` with `ViewKind: quality_hotspots` | — | Explorer renders report in LensPanel |
| **Refresh** | Incremental via `GraphDiffCalculator` | ADR-022 | `calculate_diff(old, new) → apply_events()` |

### Edge types extracted (all from Sprint 1's generic extractor)

| Edge kind | `DependencyType` | Provenance |
|-----------|------------------|------------|
| Calls (same-file) | `Calls` | `Extracted` (1.0) |
| Calls (cross-file resolved) | `Calls` | `Inferred` (0.7) |
| Imports | `Imports` | `Extracted` (1.0) |
| Contains (file → symbol) | `Contains` | `Extracted` (1.0) |
| Contains (class → method) | `Contains` | `Extracted` (1.0) |
| Inherits | `Inherits` | `Extracted` (1.0) |
| Implements | `UsesGeneric` | `Extracted` (1.0) |
| References (param_type, return_type, field) | `References` | `Extracted` (1.0) |

### Exit criteria

- [ ] After scan, `graph_reports` table has a fresh report
- [ ] `GET /api/workspaces/:id/report` returns the latest GraphReport
- [ ] Explorer LensPanel shows communities + god nodes
- [ ] Cross-file calls resolve with `Provenance::Inferred`
- [ ] Type reference edges exist for Rust, Python, TS, Go, Java
- [ ] Incremental refresh (1 file changed) updates ArcSwap cache in O(Δ)

### API surface (new endpoints)

| Endpoint | Method | Description |
|----------|--------|-------------|
| `GET /api/workspaces/:id/report` | GET | Latest GraphReport |
| `GET /api/workspaces/:id/communities` | GET | Community clusters |

### MCP new tools (Mode B only)

| Tool | Description |
|------|-------------|
| `scan_workspace` | Async pipeline trigger. Returns job_id for large projects. Mode B only. |
| `get_graph_report` | Fetch auto-generated GraphReport (god nodes, communities, surprising connections, dead code). Mode B only. |

### MCP Graphify-style tools (ADR-026)

| Task | Tier | Detail |
|------|------|--------|
| **`graph_query`** — NL graph topology query | T1 | Keyword extraction → IDF seed matching → BFS expansion → subgraph + explanation |
| **`graph_explain`** — composite deep-dive | T1 | Aggregates 6+ existing tools into one response (callers, callees, refs, community, god status, SOLID hints) |
| **`get_graph_report`** — pipeline report | T1 | Reads from `graph_reports` table (Mode B) or computes on-demand (Mode A) |
| **`get_type_references`** — References edges | T2 | Thin wrapper: graph traversal filtered by `DependencyType::References` |
| **`get_imports`** — Imports edges | T2 | Thin wrapper: graph traversal filtered by `DependencyType::Imports` |
| **`get_implementors`** — Implements/Inherits | T2 | Thin wrapper: reverse traversal from trait/interface node |
| **`get_members`** — Contains edges | T2 | Thin wrapper: traversal from class node via `DependencyType::Contains` |
| **`get_iac_references`** — Terraform/Ansible refs | T2 | Traversal from IaC node via `References` edges (requires ADR-024) |
| **`graph_query_filtered`** — provenance/kind filter | T3 | Extends graph_query with `Provenance`, `NodeKind`, `community_id` filters |
| **`export_callflow`** — module-level Mermaid | T3 | Community-level architecture diagram (aggregate call-flow between communities) |

---

## Sprint 3: Language Scaling

**Goal:** Expand from 6 to 20+ languages. Add type-reference extraction.

### Language rollout

| Phase | Languages | Priority |
|-------|-----------|----------|
| **3a** | C, C++, C# | High — large ecosystem |
| **3b** | Ruby, PHP, Swift, Kotlin | Medium — popular languages |
| **3c** | Scala, Lua, R, Zig, Dart, Julia | Lower — niche but growing |

### Tasks per language

| Task | Detail |
|------|--------|
| Add `tree-sitter-*` crate dependency | `Cargo.toml` under feature flag |
| Create `LanguageConfig` const | `function_types`, `class_types`, `import_types`, `call_types` |
| Add import handler | Per-language: C `#include`, C# `using`, Ruby `require`, PHP `use` |
| Add type-ref walker | Per-language: extract type annotations (param, return, field, generic) |
| Add test fixture | One source file with known symbols + edges |
| Register in `Language::from_extension` | Map file extension → config |

### Type-reference extraction

```rust
pub struct LanguageConfig {
    // ... existing fields ...

    /// Optional type-reference walker. Extracts type annotations
    /// (param_type, return_type, field_type, generic_arg) as
    /// `References` edges with context metadata.
    /// None = this language's type system is not yet supported.
    pub type_ref_walker: Option<fn(&tree_sitter::Node, &[u8]) -> Vec<TypeRef>>,
}

pub struct TypeRef {
    pub target_name: String,
    pub context: TypeRefContext,  // ParamType | ReturnType | FieldType | GenericArg
}
```

Type-ref walkers are per-language (each language's type syntax is different).
Priority: Rust → Python → TypeScript → Go → Java → C++ → C#.

### COPY optimization

| Task | Detail |
|------|--------|
| Implement `COPY FROM STDIN BINARY` for first scan | ADR-023 |
| Binary encode for `graph_nodes` row | TEXT id, TEXT kind, TEXT label, TEXT source_path, JSONB properties |
| Binary encode for `graph_edges` row | TEXT source_id, TEXT target_id, TEXT kind, TEXT provenance, REAL confidence, JSONB metadata |
| Decision rule: `>50 files → COPY path` | ADR-023 |

### Exit criteria

- [ ] 20+ languages parse correctly with `LanguageConfig`
- [ ] Type references extracted for top 5 languages
- [ ] First scan of 1000-file project completes in <15s (via COPY)
- [ ] Each language has a fixture test verifying extraction output

---

## Sprint 4: Incremental + Robustness

**Goal:** Production-grade incremental updates, file watching, and full
36+ language coverage.

### Incremental optimizations

| Task | ADR | Detail |
|------|-----|--------|
| Edge-level diffing in `GraphDiffCalculator` | ADR-022 | Extend to emit `DependencyAdded/Removed` events |
| `apply_events()` for edges | ADR-022 | Currently only handles symbol events |
| mtime-first optimization tuning | ADR-017 | Skip hash for mtime-unchanged files |
| Advisory lock for exclusive scan | ADR-023 | `pg_advisory_lock(hashtext(workspace_id))` |
| 409 Conflict on concurrent scan | ADR-023 | API returns existing job_id |

### File watcher (v1.5 → v1 feature)

| Task | Detail |
|------|--------|
| `notify` crate integration | `notify-debouncer-full` with 500ms debounce |
| Watch workspace root recursively | Create/Modify/Delete events |
| Queue changed files to scan channel | `tokio::sync::mpsc` to the ingest service |
| Periodic fallback re-scan | Every 5 minutes as safety net |
| Config: `watch: true/false` | Enable/disable per workspace |

```rust
// File watcher background task
let (watch_tx, watch_rx) = mpsc::channel::<Vec<PathBuf>>(16);
let mut debouncer = new_debouncer(Duration::from_millis(500), None, move |res| {
    if let Ok(events) = res {
        let paths: Vec<_> = events.iter()
            .filter(|e| e.kind.is_modify() || e.kind.is_create() || e.kind.is_remove())
            .flat_map(|e| e.paths.clone())
            .collect();
        if !paths.is_empty() {
            watch_tx.blocking_send(paths).ok();
        }
    }
})?;
debouncer.watcher().watch(&root, RecursiveMode::Recursive)?;
```

### Remaining languages (36+ target)

| Batch | Languages / Formats |
|-------|---------------------|
| **4a** | **Terraform (HCL)** + **Ansible (YAML)** — ADR-024 |
| **4b** | Groovy, Gradle, Scala, Lua, Pascal, Fortran |
| **4c** | Verilog, SystemVerilog, DreamMaker |
| **4d** | Bash, PowerShell, JSON configs |
| **4e** | Apex, Svelte, Vue, Astro, Elixir, Erlang, Haskell |

### Infrastructure-as-Code extraction (ADR-024)

| Task | Detail |
|------|--------|
| Add `tree-sitter-hcl` dependency | `tree-sitter-grammars/tree-sitter-hcl` v1.2.0, Apache-2.0 |
| Add `tree-sitter-yaml` dependency | `tree-sitter-grammars/tree-sitter-yaml` v0.7.2, MIT |
| `TERRAFORM_CONFIG` LanguageConfig | Blocks → GraphNodes, `resource`/`data`/`variable`/`module`/`provider` |
| `walk_hcl_references` walker | Extract `aws_instance.web.ami` → `References` edges |
| `depends_on` extraction | Explicit dependency → `References` edge, `Provenance::Extracted` |
| `ANSIBLE_CONFIG` LanguageConfig | YAML-based, needs `semantic_handler` |
| `interpret_ansible_playbook` handler | Detect playbook, extract plays/tasks/modules/vars/handlers |
| Shared builtin module nodes | `ansible:builtin:apt`, `ansible:builtin:file`, etc. — accumulate fan-in |
| `import_playbook` / `include_tasks` | → `Imports` edges between Ansible files |
| Terraform fixture test | `tests/fixtures/terraform/main.tf` with known resources + refs |
| Ansible fixture test | `tests/fixtures/ansible/site.yml` with known plays + tasks |

### EXIT criteria

- [ ] File watcher auto-scans changed files within 1s
- [ ] Incremental scan of 1 file completes in <500ms
- [ ] Advisory lock prevents concurrent scans (409 Conflict)
- [ ] 36+ languages supported
- [ ] **Terraform `.tf`/`.hcl` files extract resource/data/variable/module nodes + References edges** (ADR-024)
- [ ] **Ansible `.yml`/`.yaml` playbooks extract play/task/module nodes + Calls/Imports edges** (ADR-024)
- [ ] **MCP: `graph_query` answers "what connects X to Y?" with subgraph + provenance** (ADR-026)
- [ ] **MCP: `graph_explain` returns composite deep-dive in one call** (ADR-026)
- [ ] **MCP: edge-type queries work (`get_type_references`, `get_imports`, `get_implementors`, `get_members`)** (ADR-026)
- [ ] **MCP: `export_callflow` generates community-level Mermaid diagram** (ADR-026)
- [ ] MCP tool count reaches 63 (55 existing + 10 new - 2 deprecated)
- [ ] Periodic fallback re-scan catches missed watcher events

---

## Cross-Sprint Concerns

### Testing strategy

| Test type | Tool | Coverage target |
|-----------|------|----------------|
| **Unit** — `LanguageConfig` extraction | `cargo test` | Per-language fixture test |
| **Integration** — pipeline end-to-end | `cargo test --features postgres` | `TEST_DATABASE_URL` gating |
| **Contract** — VIEWs match old tables | SQL diff test | `SELECT FROM symbols` == old `symbols` |
| **Performance** — scan benchmarks | `criterion` | 1000-file project <15s (COPY), <1s (incremental) |
| **E2E** — Explorer scan flow | Playwright | POST /scan → poll → verify graph renders |

### Performance budget

| Operation | Budget | Rationale |
|-----------|--------|-----------|
| Scan manifest compare (1000 files) | <100ms | Single SQL SELECT + in-memory diff |
| Extract 1 file (avg 500 LOC) | <50ms | tree-sitter parse + AST walk |
| PgUpsert 1 file (transactional) | <20ms | BEGIN + DELETE + INSERT + COMMIT |
| COPY 1000 files bulk load | <5s | Binary COPY protocol |
| Full Refresh (10k symbols) | <2s | `load_call_graph()` + `ArcSwap::set()` |
| Incremental Refresh (1 file) | <50ms | `GraphDiffCalculator` + `apply_events()` |
| Cluster (Label Propagation, 10k nodes) | <1s | petgraph on in-memory graph |

### Dependency additions

| Crate | Version | Purpose | Sprint |
|-------|---------|---------|--------|
| `sha2` | 0.10 | SHA256 content hashing | S1 |
| `notify` | 7.x | File system watching | S4 |
| `notify-debouncer-full` | 0.4 | Debounced watcher events | S4 |
| `tree-sitter-hcl` | 1.2 | HCL/Terraform grammar | S4 (4a) |
| `tree-sitter-yaml` | 0.7 | YAML grammar (Ansible playbooks) | S4 (4a) |
| `tree-sitter-c` | latest | C grammar | S3 |
| `tree-sitter-cpp` | latest | C++ grammar | S3 |
| `tree-sitter-c-sharp` | latest | C# grammar | S3 |
| `tree-sitter-ruby` | latest | Ruby grammar | S3 |
| `tree-sitter-php` | latest | PHP grammar | S3 |
| `tree-sitter-swift` | latest | Swift grammar | S3 |
| `tree-sitter-kotlin` | latest | Kotlin grammar | S3 |
| ... | ... | (remaining per batch) | S3/S4 |
