# Proposal: Ownership Attribution Pipeline

## Intent
`OwnershipMapExecutor` exists in degraded mode — returns "ownership unavailable" for Symbol/Scope targets (`views.rs:2452`). Real ownership data (CODEOWNERS + git blame) is missing from the graph. This change adds ownership attribution to the ingest pipeline so the ownership map view renders real data.

## Scope

### In Scope
- Parse `.github/CODEOWNERS` during Scan → store `codeowners` in `graph_nodes.properties`
- Git blame via `gix` crate during Extract → enrich symbol nodes with `last_author`, `author_email`
- Upgrade `OwnershipMapExecutor` to read real data from `node_properties()`

### Out of Scope
- Ownership history / temporal blame (last-author only)
- Non-git workspaces (graceful degradation only)
- PostgreSQL schema changes (JSONB `properties` column already exists)

## Capabilities

> CONTRACT with sddk-spec. No existing specs in `openspec/specs/`.

### New Capabilities
- `ownership-attribution`: End-to-end ownership attribution — CODEOWNERS parsing in Scan, git blame enrichment in Extract, and OwnershipMapExecutor rendering from `graph_nodes.properties`.

### Modified Capabilities
- None (no existing formal specs; the degraded OwnershipMapExecutor behavior is being superseded by real data).

## Approach
**CODEOWNERS**: During `scan_for_changes()` (`scan.rs`), detect `.github/CODEOWNERS` in workspace root. Parse entries (`pattern owners`). Match file paths to symbols during Extract, store `codeowners` in `GraphNode::properties`.

**Git blame**: Add `gix` crate to `cognicode-core`. Implement blame as a **post-extraction enrichment step** in `extract_one()` (`extract_stage.rs`) — after `extract_file()` returns nodes, enrich each symbol node by running `gix blame` on its line range. This keeps `extract_file()` untouched (OCP-preserving). Store `last_author`, `author_email` in `properties`.

**View upgrade**: `OwnershipMapExecutor::build()` reads `codeowners`, `last_author`, `author_email` from `SymbolRepository`/`GraphQueryPort` properties instead of returning placeholder.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/Cargo.toml` | Modified | Add `gix` dependency |
| `crates/cognicode-core/src/application/ingest/scan.rs` | Modified | CODEOWNERS detection + parsing |
| `crates/cognicode-core/src/application/ingest/extract_stage.rs` | Modified | Post-extraction blame enrichment step |
| `crates/cognicode-explorer/src/domain/views.rs` | Modified | OwnershipMapExecutor reads real properties |
| `crates/cognicode-core/src/application/ingest/` (new module) | New | `codeowners.rs` parser + `blame.rs` enrichment |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `gix` build complexity (native deps) | Medium | Pure Rust, no C deps; feature-gate behind `ownership` feature |
| Blame slow on large files | Medium | Only run on changed files (incremental); cache per content_hash |
| Non-git workspace failure | Low | Detect git repo; skip blame silently if absent |
| Binary files cause blame errors | Low | Skip non-UTF8 files (extract stage already does) |

## Rollback Plan
1. Remove `gix` from `Cargo.toml`
2. Revert `extract_stage.rs` enrichment call (single function call removal)
3. Revert `scan.rs` CODEOWNERS detection (guarded behind `if` — remove block)
4. OwnershipMapExecutor falls back to placeholder automatically (no real properties = placeholder)

## Dependencies
- `gix` crate (pure Rust git implementation, no C dependencies)
- No PostgreSQL schema migration required

## Success Criteria
- [ ] `.github/CODEOWNERS` parsed and `codeowners` property present on symbol nodes
- [ ] `last_author` and `author_email` populated via gix blame on symbol nodes
- [ ] OwnershipMapExecutor renders real data for Symbol targets (no "ownership unavailable")
- [ ] Non-git workspaces degrade gracefully (no panic, no error)
- [ ] Blame enrichment adds < 2s per file for files under 5000 lines

## Open Questions
1. **Blame attribution for multi-author symbols**: Use last author of the symbol's first line, or majority author across all lines? (Recommend: last author of first line — cheapest, most stable)
2. **CODEOWNERS pattern matching**: Use gitignore-style globs (CODEOWNERS spec) or simplified prefix matching? (Recommend: gitignore-style via `glob` crate)
3. **Blame caching strategy**: Cache blame results keyed by content_hash in `scan_manifest`, or recompute every ingest? (Recommend: recompute — changed files only, acceptable cost)
4. **Feature gating**: Gate `gix` behind an `ownership` feature flag, or always-on? (Recommend: feature flag — keeps default build lean)
