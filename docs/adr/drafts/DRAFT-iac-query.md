# DRAFT: ADR-036 — IaC Query Module Architecture

**Status**: DRAFT (auto-grill)
**Date**: 2026-06-17
**Grill Report**: [docs/grill/2026-06-17-iac-query-design.report.md](../grill/2026-06-17-iac-query-design.report.md)

## Decision

`iac_query` is implemented as a thin wrapper over the existing `GraphQueryPort`. IaC nodes reuse `Symbol(SymbolKind)` with domain-prefixed IDs (`tf:`, `ansible:`). A `semantic_handler` field on `LanguageConfig` serves as the extension point for IaC-specific extraction. No new `NodeKind` variant is introduced.

## Context

`iac_query` is the last ghost MCP tool — it was removed from `build_all_tools()` in M2.4-2.7 because it returned `resource_type: "unknown"` with empty data. The ADR-034 production readiness sprint requires all tools in `tools/list` to return honest, non-placeholder data.

The exploration phase found:
- IaC extraction exists structurally (HCL_CONFIG, YAML_CONFIG registered) but is semantically empty — the generic tree-sitter extractor produces generic `Symbol(Function)` nodes without IaC semantics
- `interpret_ansible` exists as dead code (never wired into extraction)
- `interpret_terraform`, `walk_hcl_references`, and `handle_terraform_imports` do not exist (despite ADR-024 promising them)
- `LanguageConfig` has no `semantic_handler` field (ADR-024's proposed extension point was never implemented)

However, the infrastructure for a real iac_query IS ready:
- `GraphQueryPort` provides production-grade BFS traversal (`traverse_callees`, `traverse_callers`, `dependencies_with_metadata`)
- `SymbolRepository` provides fuzzy name resolution (`find_symbols_by_name`)
- The dispatch match and handler skeleton exist in `consolidated_handlers.rs`
- 63 tools are already annotated with `cognicode_meta` (M2.3 pattern)

### What AI agents actually ask about IaC
Research confirms AI coding agents primarily ask:
1. **Blast radius**: "What depends on aws_instance.web?" (BFS from resource node)
2. **Compliance**: "Show all resources of type X" (filter by prefixed ID prefix)
3. **Drift detection**: "What changed between deploys?" (already handled by `compare_graph`)

## Alternatives Considered

### Rejected: New `NodeKind::IaC` variant
Adding a new enum variant to `NodeKind` would require: DB migration for existing graph_nodes rows, frontend rendering updates for the new kind, every graph consumer to pattern-match the new variant, and tool schema regeneration. Prefixed IDs (`tf:`, `ansible:`) provide equivalent semantic discrimination at zero migration cost. The `multimodal` feature gate (Cargo.toml:27) is precedent for additive variants — but the cost-benefit does not justify it for v1.

### Rejected: Duplicated BFS in iac_query handler
Reimplementing graph traversal in the handler would duplicate `GraphQueryPort`'s production-grade BFS logic. This violates the Interface Segregation Principle and creates two divergent implementations of the same algorithm. The handler should translate IaC domain terms to `SymbolId`, call existing ports, and translate results back — no new graph traversal logic.

### Rejected: Full ADR-024 implementation (Option C)
Option C (full HCL AST walker with `walk_hcl_references`, `handle_terraform_imports`, for/splat/depends_on support) is estimated at 4-5 days. These edge cases represent <5% of typical Terraform code. Shipping Option B first (minimal extraction + one-level References edges) provides immediate utility and collects real usage data before investing in deeper extraction.

### Rejected: Canonical-only ID resolution
Requiring canonical prefixed IDs (`tf:main.tf:aws_instance.web`) makes the tool less accessible to AI agents, who think in terms of resource names (`aws_instance.web`). Following the `nl_to_symbol` precedent — accept fuzzy input, resolve via `SymbolRepository::find_symbols_by_name()` with prefix filtering — gives maximum utility with zero new abstraction.

## Consequences

### What becomes easier
- **Zero migration**: No new DB tables, no NodeKind variants, no frontend changes
- **Backward compatible**: Existing 63 tools unchanged. `iac_query` is additive.
- **AI agent friendly**: Accepts bare resource names, resolves to canonical IDs internally
- **Extensible**: `semantic_handler` on `LanguageConfig` is the right extension point — future IaC formats (K8s, Docker Compose, Pulumi) follow the same pattern
- **Observable**: Extraction metrics follow TerraMetrics/Arborist patterns — parse success rate, node/edge counts, semantic hit rate per file

### What becomes harder
- **Cross-domain queries** (code ↔ IaC) remain Phase 2 as ADR-024 specified — `iac_query` v1 is IaC-only
- **Full Terraform AST awareness** (for_each, count, depends_on) deferred to v2 — one-level References edges cover 95% of typical code
- **Ansible builtin modules** (`ansible:builtin:*`) accumulate fan-in across playbooks — lifecycle management requires the incremental refresh patterns from ADR-022

### Implementation sequence (4 steps, ~230 lines)
1. **Add `semantic_handler` field** to `LanguageConfig` struct (~5 lines, ~30 language configs updated to `None`)
2. **Wire `interpret_ansible`** into `extract_file` as post-parse pass (~15 lines)
3. **Create minimal `interpret_terraform`** — emits `tf:` prefixed IDs from HCL blocks + one level of References edges (~150 lines)
4. **Implement `iac_query` handler** — resolve resource_id via `SymbolRepository`, call `GraphQueryPort::neighbors()`, format output with `edge_type` metadata (~60 lines)

### Rollout strategy
1. **Invisible** (current): Handler exists but not in `build_all_tools()` — invisible to MCP clients
2. **Experimental**: Add to `build_all_tools()` behind `iac-query` Cargo feature flag with `cognicode_meta(stability="experimental")`
3. **Stable**: Promote to `stability="stable"` once tested with real Terraform/Ansible projects
