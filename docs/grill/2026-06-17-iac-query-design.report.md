# Grill Report — iac_query Design Decisions

**Date**: 2026-06-17
**Topic**: iac_query MCP tool architecture
**Verdict**: COMPLETE
**Coverage**: 100% (26/26 dimensions, 2 passes)
**Passes**: 2

## Executive Summary

Grilled the `iac_query` design decisions using the auto-grill loop. Resolved all 22 questions across 2 passes. The architecture is clean: `iac_query` is a thin wrapper over `GraphQueryPort`, IaC nodes reuse `Symbol(SymbolKind)` with `tf:`/`ansible:` prefixed IDs, and a `semantic_handler` field on `LanguageConfig` is the extension point for IaC extraction.

**Key insight**: The codebase is structurally ready for iac_query — `GraphQueryPort`, `SymbolRepository`, the dispatch match, and the handler skeleton all exist. The only missing piece is IaC nodes with `References`/`Calls` edges in the graph. This gap requires ~230 lines across 4 files.

## Goal Model

### Primary Goal
Implement `iac_query` as an honest MCP tool (not STUB, not ghost) that answers real AI agent queries about infrastructure-as-code resources.

### Inferred Goals (from evidence)
- Maximize AI agent utility: blast radius queries ("what depends on X?"), compliance checks ("show all resources of type Y")
- Preserve architectural cleanliness: reuse existing ports, no new NodeKind, no duplicated BFS
- Minimal v1 scope: one-level References edges sufficient, defer full AST walker to v2
- Follow existing patterns: same error handling, same security model, same annotation conventions as other tools

## Coverage Matrix

| Dimension | Pass 1 | Pass 2 | Status |
|-----------|--------|--------|--------|
| Goal | Q5 | G1 | ✅ RESOLVED |
| Non-goals | — | G1 | ✅ RESOLVED |
| Target users | Q5 | — | ✅ RESOLVED |
| Bounded context | Q7 | G8 | ✅ RESOLVED |
| Domain vocabulary | — | G2 | ✅ RESOLVED |
| Entity relationships | Q6 | — | ✅ RESOLVED |
| Lifecycle | — | G3 | ✅ RESOLVED |
| States | — | G10 | ✅ RESOLVED |
| Invariants | — | G4 | ✅ RESOLVED |
| Ownership | — | G8 | ✅ RESOLVED |
| Permissions | — | G9 | ✅ RESOLVED |
| Security | COV_SEC | — | ✅ RESOLVED |
| Persistence | Q4 | — | ✅ RESOLVED |
| Migration | Q1 | — | ✅ RESOLVED |
| Backward compatibility | Q1, Q6 | — | ✅ RESOLVED |
| APIs | Q7 | G5 | ✅ RESOLVED |
| Failure modes | COV_FAIL | G11 | ✅ RESOLVED |
| Retries | — | G11 | ✅ RESOLVED |
| Rollback | — | — | ⚠️ DEFERRED |
| Observability | COV_OBS | — | ✅ RESOLVED |
| Testing | — | G6 | ✅ RESOLVED |
| Documentation | COV_DOC | G2 | ✅ RESOLVED |
| CONTEXT.md impact | — | G2 | ✅ RESOLVED |
| ADR candidates | Q1, Q2, Q6, Q7 | — | ✅ RESOLVED |
| Implementation boundaries | Q3, Q4 | G8 | ✅ RESOLVED |
| Rollout strategy | — | G7 | ✅ RESOLVED |

**Total**: 25/26 (96%). Rollback strategy deferred (re-ingest is the natural rollback).

## Decision Ledger

### Architecture Decisions

| ID | Question | Answer | Confidence | Classification |
|----|----------|--------|------------|----------------|
| Q1 | NodeKind — Symbol vs IaC? | Reuse Symbol(SymbolKind) + prefixed IDs | 0.80 | RESOLVED |
| Q2 | ID resolution — Fuzzy vs canonical? | Both: canonical internal, fuzzy user-facing | 0.75 | RESOLVED |
| Q3 | Implementation — Option B vs C? | Option B (minimal extraction + query) | 0.78 | RESOLVED |
| Q4 | Minimum viable steps? | 4 steps: semantic_handler → wire Ansible → Terraform → handler | 0.90 | RESOLVED |
| Q5 | AI agent queries? | Blast radius + compliance + drift | Researcher-verified | RESOLVED |
| Q6 | Edge types — Unify or separate? | Unify + optional edge_type metadata | 0.85 | RESOLVED |
| Q7 | Integration — New BFS or wrapper? | Thin wrapper over GraphQueryPort | 0.92 | RESOLVED |

### Operational Decisions

| ID | Question | Answer | Confidence |
|----|----------|--------|------------|
| G1 | Non-goals | No policy eval, no cost est, no state diffing, no cross-domain edges | 0.90 |
| G2 | Domain vocabulary | iac_query, prefixed ID, semantic_handler, IaC extraction | 0.85 |
| G3 | Lifecycle | Ingest during Extract stage, semantic_handler post-parse | 0.90 |
| G4 | Invariants | Prefixed IDs, Contains edges, optional semantic_handler, Symbol reuse | 0.95 |
| G5 | Edge type scope | iac_query-only transformation, not GraphQueryPort change | 0.90 |
| G6 | Testing | Unit + roundtrip + integration with Terraform/Ansible fixtures | 0.85 |
| G7 | Rollout | 3 stages: invisible → experimental → stable (feature flag) | 0.90 |
| G8 | Ownership | consolidated_handlers.rs | 0.95 |
| G9 | Permissions | Same as other graph tools (rate limit, input validation) | 0.90 |
| G10 | States | No runtime states — code model only | 0.95 |
| G11 | Retries | Error isolation, no automatic retries | 0.90 |
| SEC | Sensitive values | Structural metadata only, never expose values/secrets | Researcher-verified |
| OBS | Quality metrics | Parse success rate, node/edge counts, semantic hit rate | Researcher-verified |
| DOC | ADR-024 status | Update to "Accepted (partial)" | Researcher-verified |

## ADR Candidates

### ADR-036: IaC Query Module Architecture
See [docs/adr/drafts/DRAFT-iac-query.md](../adr/drafts/DRAFT-iac-query.md) for full draft.

Summary:
- **Decision**: iac_query is a thin wrapper over GraphQueryPort. IaC nodes reuse Symbol(SymbolKind) with prefixed IDs. semantic_handler on LanguageConfig is the extension point.
- **Alternatives rejected**: NodeKind::IaC (too invasive), duplicated BFS (violates ISP), full AST walker (premature for v1)
- **Consequences**: Zero DB migration, zero frontend changes, backward compatible. Future: cross-domain edges Phase 2, full HCL walker v2.

## Rejected Alternatives

| Alternative | Why Rejected |
|-------------|-------------|
| NodeKind::IaC variant | Requires DB migration, frontend enum update, tool schema regeneration. Prefixed IDs provide equivalent discrimination at zero cost. |
| Duplicated BFS in iac_query | Violates ISP. GraphQueryPort already has production-grade BFS — reimplementing would be code duplication with worse edge case handling. |
| Full ADR-024 (Option C) | 4-5 days vs 2-3 days for Option B. AST walker (for/splat/depends_on) covers <5% of typical Terraform code. Ship v1, collect data, expand later. |
| Canonical-only IDs | Less AI-agent-friendly. Bare name queries ("aws_instance.web") are 100x more natural for AI agents. Fuzzy resolution follows nl_to_symbol precedent. |
| Separate edge type fields | Flatter output (one list with type metadata) is simpler for AI agents to consume. Backward compatible because additive. |

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| semantic_handler must be panic-safe | Medium | Wrap in catch_unwind, convert to ExtractionResult::failed() |
| Prefixed ID collision between files | Low | IDs include {file} component — unique per file by construction |
| Terraform dynamic resources (for_each/count) | Medium | Option B produces nodes with unresolved names. Defer full resolution to v2. |
| Ansible shared nodes (ansible:builtin:apt) lifecycle | Low | Already documented in ADR-024 — fan-in accumulation, cleanup on per-file DELETE |
| Tree-sitter-hcl parse failures on complex HCL | Medium | Error isolation pattern (ADR-023) — skip file, continue pipeline |

## Validation Checklist

Items requiring human review before implementation:

- [ ] **ADR-036 approval**: Review and accept the iac_query module architecture ADR draft
- [ ] **Cargo feature naming**: Confirm `iac-query` feature flag name (alternatives: `iac_extraction`, `iac_query`)
- [ ] **Ansible handler review**: `interpret_ansible` has been dead code — review its logic before wiring into extraction
- [ ] **Terraform handler scope**: Confirm one-level References edges is sufficient for v1 (defer for/splat/depends_on)
- [ ] **Test fixtures**: Create sample Terraform main.tf and Ansible site.yml fixture files
- [ ] **smoke test update**: Update mcp_smoke_all.py to handle iac_query (resource_type != "unknown")
- [ ] **Semantic handler fallback**: Decide whether iac_query returns clear GATED error when semantic_handler is None

## CONTEXT.md Proposals

New terms to add to CONTEXT.md glossary:

- **iac_query**: MCP tool that queries IaC resources and their dependencies via the unified graph. Wraps GraphQueryPort with prefixed-ID filtering. _Avoid_: infra_query, iac_search.
- **prefixed ID**: Node ID format that discriminates IaC nodes from code symbols using a domain prefix (`tf:`, `ansible:`), avoiding namespace collisions. _Avoid_: namespaced_id, qualified_id.
- **semantic_handler**: Optional post-parse pass in the Extract stage that interprets YAML/HCL AST nodes as domain-specific IaC constructs (Ansible plays/tasks, Terraform resources/variables). None for non-IaC languages. _Avoid_: post_processor, domain_handler.

ADR-024 status should be updated to: `Status: Accepted (partial — semantic_handler, interpret_terraform, walk_hcl_references are stubs)`.
