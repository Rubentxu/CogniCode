# Kernel Exploration: iac_query Implementation

**Date**: 2026-06-17
**Phase**: SDD Kernel — Explore (post auto-grill)
**Grill Report**: [docs/grill/2026-06-17-iac-query-design.report.md](../grill/2026-06-17-iac-query-design.report.md)
**ADR Draft**: [docs/adr/drafts/DRAFT-iac-query.md](../adr/drafts/DRAFT-iac-query.md)

## Current State

`iac_query` is a **ghost MCP tool** today — present in dispatch, absent from `tools/list`, returning placeholders. The architecture for a real implementation is structurally ready but three concrete gaps remain: (1) `LanguageConfig` lacks a `semantic_handler` field, (2) `interpret_ansible` is dead code never wired into extraction, (3) the handler stub returns `resource_type: "unknown"` with empty vectors.

**Pipeline today** (`extract_stage.rs:98` → `extractor.rs:26`):
```text
extract_stage.rs:98  extract_file(config, path, &source, &hash)
                       │
                       ├─ parse tree-sitter AST with config
                       ├─ emit generic Symbol(Function) per function_types
                       ├─ emit generic Symbol(Class)   per class_types
                       ├─ emit Calls edges             per call_types
                       ├─ emit Imports edges           per import_types
                       └─ return ExtractionResult      ← NO semantic_handler pass
```

For HCL/YAML files this produces generic `Symbol(Function)` nodes labeled with raw tree-sitter node text (e.g., `"block"`, `"attribute"`). No `tf:` / `ansible:` prefixed IDs, no IaC semantics, no references between resources. `iac_query` handler at `consolidated_handlers.rs:617` then receives a real symbol but cannot find any IaC-specific structure, hence the placeholder stub.

**Handler today** (`consolidated_handlers.rs:597-628`):
```rust
pub async fn handle_iac_query(ctx: &HandlerContext, input: IacQueryInput) -> HandlerResult<IacQueryOutput> {
    let _graph = ctx.get_graph_store().load_graph();
    Ok(IacQueryOutput {
        resource_id: input.resource_id,
        resource_type: "unknown".into(),       // ← placeholder
        dependencies: vec![],                   // ← empty
        dependents: vec![],                     // ← empty
    })
}
```

**Tool registration today** (`rmcp_adapter.rs:203-...`):
- `iac_query` dispatch arm: `rmcp_adapter.rs:1733` — wired and accepts JSON
- `iac_query` in `build_all_tools()`: **absent** — invisible to MCP clients

**Smoke test gate** (`scripts/mcp/mcp_smoke_all.py:206-214`):
```python
if name == "iac_query" and isinstance(result, dict) and result.get("resource_type") == "unknown":
    # Treats as STUB even at STABILITY_STABLE
```
This means: even if we add `iac_query` to `build_all_tools()` with `stability="stable"` and don't fix the handler, smoke tests fail.

## Context Quality

- **Level**: C2 (verified)
- **Evidence Present**:
  - All 7 code locations from grill verified directly against current source
  - `LanguageConfig` struct read end-to-end (lines 1-507) — confirmed `semantic_handler` does NOT exist
  - `interpret_ansible` signature verified (`fn(source_path: &str, source_hash: &str, result: &ExtractionResult) -> ExtractionResult`, line 15)
  - `extract_file` full body read (352 lines) — confirmed no semantic_handler pass exists
  - `extract_stage.rs:98` verified as the wiring point for a post-parse hook
  - `consolidated_handlers.rs:597-628` handler stub confirmed
  - `rmcp_adapter.rs:1733` dispatch arm confirmed
  - `build_all_tools()` confirmed to have NO `iac_query` Tool entry
  - `SymbolKind` enum has `Function`, `Class`, `Variable`, `File` etc. — confirmed `interpret_ansible` reuses these correctly
  - `DependencyType::References` variant exists
  - `Language::Hcl` and `Language::Yaml` enum variants exist (tree_sitter_parser.rs:45-46)
  - Smoke test special-cases `iac_query` at `mcp_smoke_all.py:206-214`
- **Missing Context**:
  - `interpret_ansible` was written before the current `ExtractionResult` API — needs dry-run test with real YAML input to confirm it produces correct node IDs against the actual tree-sitter-yaml AST shape (heuristic label matching may fail on recent tree-sitter versions)
  - No existing Terraform fixture (`main.tf`) to validate `interpret_terraform` will be designed against
  - No Ansible fixture (`site.yml`) to validate `interpret_ansible` against real input
- **Recommended Effort**: **verify** (C2 — context is sufficient to propose; no deepen needed; the architectural decisions are solid)

## Knowledge Coverage

| Class | Status | Evidence | Gap Impact |
|------|--------|----------|------------|
| Roadmap/Backlog | present | `docs/mcp-production-roadmap.md:66,121` — `iac_query` flagged 2d effort; `M2.6` removed from `build_all_tools()` | Confirms priority: production-readiness sprint expects real implementation |
| Work Items | present | `docs/sdd-kernel/implement-stub-tools-archive-report.md:22,34,62,78,93` — tracks as last ghost tool, no IaC layer yet | Confirms scope: implement IaC query layer, not just stabilize stub |
| Architecture/ADRs | present | `docs/adr/ADR-024` (IaC extraction), `ADR-028` (high-value MCP tools), `ADR-034` (production readiness), `DRAFT-iac-query.md` (ADR-036 in flight) | Strong precedent: ADR-024 already promised `interpret_terraform`/`walk_hcl_references` but they were never implemented — execution gap, not design gap |
| Ownership | present | DRAFT-iac-query.md:65 places implementation in `consolidated_handlers.rs`; `LanguageConfig` extension owned by parser module | Clear |
| Learnings | present | Auto-grill report (96% coverage), DRAFT-iac-query.md, this preflight | One material correction surfaced (see "Changes from grill" below) |

## Problem Taxonomy

| Axis | Applies | Evidence |
|------|---------|----------|
| Domain modeling | **Yes** | IaC resources are conceptually different from code symbols but reusing `Symbol(SymbolKind)` with prefixed IDs is the right call — `interpret_ansible` already follows this pattern. Need `semantic_handler` to express "HCL blocks are resources/modules, YAML plays are playbooks" without polluting the generic extractor. |
| Boundary/seam | **Yes** | The semantic_handler hook is the boundary: generic extractor stays language-agnostic, semantic pass applies IaC-specific interpretation. Mirrors the existing `type_ref_walker` hook (Rust/Python/TS) which already proves the pattern works. |
| Coupling/connascence | **Yes** | Current connascence: `interpret_ansible` re-walks `result.nodes` via label heuristics — duplicated AST traversal. After fix: semantic_handler gets called once by `extract_stage.rs`, no duplication. Connascence of Name: prefixed IDs (`tf:`, `ansible:`) must be agreed in the contract. |
| API contract | **Yes** | `IacQueryInput` (resource_id, depth) and `IacQueryOutput` (resource_id, resource_type, dependencies, dependents) already exist — adding `edge_type` metadata is additive (per DRAFT-iac-query.md rejected-alternatives). |
| Refactor/legacy | **Yes** | `interpret_ansible` is dead code from ADR-024 era that was never wired. The fix is to wire it, not refactor it. `interpret_terraform`, `walk_hcl_references`, `handle_terraform_imports` were also promised but never created — need fresh minimal `interpret_terraform`. |
| Event/CQRS | No | Pure read query against the graph |
| Testing | **Yes** | No existing tests for `interpret_ansible` or `iac_query` — need: (1) unit tests for the two handlers, (2) roundtrip test (extract → query), (3) integration with sample `main.tf` + `site.yml` fixtures, (4) update `mcp_smoke_all.py` semantics |
| Security/operations | **Yes** | Per grill decision: structural metadata only, never expose Terraform variable values or Ansible vault secrets. The `extract_terraform` implementation must avoid inlining attribute values into `GraphNode.properties`. |

## Domain Language And Invariants

**Resolved domain terms** (from DRAFT-iac-query.md + CONTEXT.md):
- **iac_query**: MCP tool that queries IaC resources and their dependencies via the unified graph. Wraps `GraphQueryPort` with prefixed-ID filtering.
- **prefixed ID**: `tf:<file>:<resource_type>.<name>` or `ansible:<file>:<playbook_element>:<name>`. Discriminates IaC nodes from code symbols without new `NodeKind` variant.
- **semantic_handler**: Optional post-parse pass in `extract_file` that interprets YAML/HCL AST nodes as IaC constructs. None for non-IaC languages.
- **IaC extraction**: The phase where generic tree-sitter AST is interpreted as Terraform resources / Ansible plays/tasks/modules, producing prefixed-ID `GraphNode`s + `References`/`Calls` edges.

**Invariants** (resolved):
- Prefixed IDs are unique per file by construction (file component included).
- `LanguageConfig` may have `semantic_handler = None` — generic extractor must still produce a valid (if semantically empty) `ExtractionResult`.
- `iac_query` handler MUST resolve via `SymbolRepository::find_symbols_by_name()` with prefix filter — accepts both `tf:aws_instance.web` and bare `aws_instance.web`.
- `iac_query` MUST return `edge_type` metadata in `dependencies`/`dependents` lists — additive change, backward compatible.
- `iac_query` MUST NOT expose Terraform variable values or Ansible vault secrets — structural metadata only.

**Unresolved ambiguities** (need explicit decision before proposal):
1. **`GraphQueryPort::neighbors()` does not exist** — the ADR draft and grill report both say "call `GraphQueryPort::neighbors()`", but the trait only has `callers`, `callees`, `*_with_metadata`, `dependencies_with_metadata`, `traverse_callees`, `traverse_callers`, `fan_in`, `fan_out`. **This must be resolved in the proposal phase.**
2. **Semantic handler return type**: should it own the `Result` (replace generic output) or augment it (preserve generic + add IaC nodes/edges)? The existing `interpret_ansible` replaces — but that loses generic YAML symbols, which may be desirable for non-Ansible YAML. Needs decision.
3. **Cargo feature gate**: grill report flags `iac-query` vs `iac_extraction` vs `iac_query` naming as TBD. Recommend `iac-extraction` (kebab → underscore in Rust identifier) to mirror the existing `multimodal` precedent in `Cargo.toml:27`.
4. **`interpret_ansible` validation**: written 2+ years ago, uses label heuristics that may not match modern tree-sitter-yaml output. May need refresh before wiring.

## Knowledge Gaps

- **`interpret_ansible` validity against current tree-sitter-yaml AST** — written before tree-sitter-yaml grammar updates. A 10-line smoke test that parses `site.yml` and runs `interpret_ansible` would confirm or surface drift.
- **No Terraform fixture file** — need to create `tests/fixtures/iac/main.tf` and `tests/fixtures/iac/site.yml` for roundtrip tests.
- **Smoke test gate** — `mcp_smoke_all.py:206-214` will fail on `iac_query` until `resource_type != "unknown"`. Validation checklist already calls this out.
- **No existing benchmarks for IaC extraction** — acceptable to defer to v2.

## Affected Areas

- `crates/cognicode-core/src/infrastructure/parser/language_config.rs` — add `semantic_handler: Option<SemanticHandlerFn>` field (~5 lines); update 30 const configs (~30 lines, mechanical).
- `crates/cognicode-core/src/infrastructure/parser/mod.rs` — re-export new `interpret_terraform` (1 line).
- `crates/cognicode-core/src/infrastructure/parser/terraform_handler.rs` — **NEW FILE** ~150 lines, minimal HCL block interpretation.
- `crates/cognicode-core/src/application/ingest/extract_stage.rs:98` — invoke `semantic_handler` after `extract_file` returns (~5 lines).
- `crates/cognicode-core/src/interface/mcp/handlers/consolidated_handlers.rs:617-628` — replace stub body with real implementation (~60 lines).
- `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` — add `iac_query` to `build_all_tools()` with `cognicode_meta(stability="experimental", ...)` (~15 lines).
- `scripts/mcp/mcp_smoke_all.py:206-214` — remove the `iac_query` special-case OR ensure `resource_type != "unknown"` for STABLE (depends on rollout).
- `CONTEXT.md` — add 3 new glossary terms (iac_query, prefixed ID, semantic_handler) per grill report.
- `docs/adr/ADR-024-iac-extraction.md` — status update: "Accepted (partial — semantic_handler wired; interpret_terraform minimal v1; walk_hcl_references deferred to v2)".

## Options

| Option | Pros | Cons | Effort |
|--------|------|------|--------|
| **A. Wire semantic_handler + minimal interpret_terraform + real handler + register tool (DRAFT plan)** | Matches ADR-036, ~230 lines, additive, no DB migration, AI-agent-friendly, leverages existing GraphQueryPort | Depends on graph containing the right nodes; smoke test currently rejects; `interpret_ansible` may need refresh | ~2-3 days |
| B. Fix handler only (no semantic_handler, no IaC extraction) | Fastest (1 day); unblocks `iac_query` registration; satisfies ADR-034 honesty requirement | Handler returns empty for real IaC — still effectively a ghost tool, just one with stable classification | 0.5-1 day |
| C. Skip iac_query, mark as GATED in tools/list | Trivial; complies with production-readiness rules | Loses ADR-024 commitment; third ghost-tool postponement cycle | <1h |

## Entropy Envelope

- **Method**: heuristic (C2 — direct code reading, no CogniCode graph available for this exploration)
- **Coupling risk**: **low**
  - `semantic_handler` is a `fn` field on `LanguageConfig` — additive, no breaking change to existing configs (default to `None`).
  - `iac_query` handler is a thin wrapper — no business logic, only ID resolution + BFS delegation.
  - Prefixed IDs are a naming convention, not a schema change — `GraphNode::id` is already `String`.
- **Connascence of Name risk**: **low**
  - Prefixes `tf:` and `ansible:` are localized to the two handlers; other code is prefix-agnostic.
  - Mitigation: a single `const TF_PREFIX: &str = "tf:"` and `const ANSIBLE_PREFIX: &str = "ansible:"` in the handler modules; both expose them via a shared `pub(crate) const` if cross-module consistency becomes a concern.
- **OCP risk**: **low**
  - Adding a new IaC format (Kubernetes YAML, Pulumi) becomes: add a `LanguageConfig` entry + write a `semantic_handler`. No changes to existing code.
- **One risk worth flagging**: `interpret_ansible` is heuristic-based (`label.contains("hosts:")`). If tree-sitter-yaml grammar has changed since this was written, the heuristic may silently produce zero output. **Recommend adding a unit test with a real `site.yml` fixture as the first commit of Step 2.**

## Recommendation

**Adopt Option A** — the path is well-defined, the architecture is sound, the architecture decision (ADR-036) has cleared the grill, and the gaps are concrete and bounded (~230 lines, 4 files). The drift from grill assumptions is small (one method-name correction: `neighbors()` → `dependencies_with_metadata` / `callers_with_metadata`).

**Pre-proposal must-resolve questions** (in priority order):
1. **Resolve `GraphQueryPort::neighbors()` drift** — replace with the actual method. Recommended: `dependencies_with_metadata` for outgoing + `callers_with_metadata` (or `traverse_callers` for BFS) for blast radius. This affects the handler design and the ADR wording.
2. **Decide semantic_handler return semantic** — does it replace or augment? Recommendation: **augment** (return `Result<ExtractionResult>` where `nodes = generic ∪ iac` and `edges = generic ∪ iac`). Reason: preserves generic YAML/HCL symbols for non-IaC YAML (k8s manifests, docker-compose) and lets future semantic_handlers layer without losing upstream work.
3. **Validate `interpret_ansible` against current tree-sitter-yaml** — write a 30-line test, fix if heuristic broke.
4. **Decide Cargo feature name** — recommend `iac-extraction` (kebab-case to underscore `iac_extraction` in Rust identifier) for consistency with `multimodal` precedent.

**Ready For Proposal**: **Yes**, after the 4 questions above are resolved. The architectural foundation is verified, the gaps are scoped, and there are no blocking unknowns. The proposal phase can write a delta spec that resolves the `neighbors()` → `dependencies_with_metadata`/`callers_with_metadata` mapping, decides augment-vs-replace, and locks the feature flag name.
