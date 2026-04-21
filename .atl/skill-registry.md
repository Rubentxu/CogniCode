# Skill Registry for CogniCode

**Generated**: 2026-04-21

## SDD Pipeline

init → explore → propose → design → spec → tasks → apply → verify → archive

## CogniCode MCP Integration

**Skill**: `cognicode-sdd` (`~/.config/opencode/skills/cognicode-sdd/SKILL.md`)
**MCP server**: `/home/rubentxu/Proyectos/rust/CogniCode/target/release/cognicode-mcp` (`enabled: true`)
**Trigger**: Automatic for all sdd-* phases that involve code analysis, impact assessment, refactoring, or architecture validation.

### Compact Rules (inject for sdd-explore, sdd-design, sdd-apply, sdd-verify)

```
- ALWAYS call cognicode_build_graph before any other CogniCode tool
- Use compressed: true in explore phases to preserve context window
- cognicode_safe_refactor preview=true is MANDATORY before preview=false — no exceptions
- cognicode_analyze_impact before any non-trivial change — surface the blast radius
- cognicode_validate_syntax after every safe_refactor preview=false — non-negotiable
- cognicode_check_architecture score < 80 = flag as existing debt in proposals and designs
- New architecture cycles after apply = CRITICAL in verify (not a warning)
- CogniCode is enhancement, not a requirement — if unavailable, proceed without it
- Never block an SDD phase waiting for CogniCode — report unavailability and continue
```

### Phase → CogniCode Tools

| Phase | Tools |
|-------|-------|
| sdd-explore | build_graph, get_entry_points, get_leaf_functions, get_hot_paths, analyze_impact, get_complexity, semantic_search, get_file_symbols, get_outline |
| sdd-design | build_graph, analyze_impact, check_architecture, trace_path, get_complexity, get_call_hierarchy |
| sdd-tasks | build_lightweight_index, get_call_hierarchy, find_usages, query_symbol_index |
| sdd-apply | build_lightweight_index, validate_syntax, safe_refactor, find_usages, analyze_impact, get_symbol_code |
| sdd-verify | find_usages, check_architecture, get_hot_paths, build_lightweight_index |

## Rust-Specific Skills (high relevance)

- `rust-testing` — Rust test patterns, cargo test, mockall, tokio
- `rust-refactor-helper` — Safe Rust refactoring with LSP
- `rust-symbol-analyzer` — LSP symbol analysis
- `rust-call-graph` — Call graph visualization with LSP
- `rust-ddd-expert` — Domain-Driven Design for Rust
- `review-wasm` — DOD/ECS/WASM performance audit

## User Skills (~/.config/opencode/skills/)

sdd-init, sdd-propose, sdd-design, sdd-spec, sdd-tasks, sdd-apply, sdd-verify,
sdd-archive, sdd-explore, cognicode-sdd, go-testing, issue-creation, branch-pr,
judgment-day, skill-creator

## Project Skills (.claude/skills/)

bug-find, rust-ddd-expert, ralph-rust, pretty-mermaid, product-owner,
documentacion, investigacion, review-wasm, frontend-design, pruebas-cli,
refactor, git-versioning, doc-writer
