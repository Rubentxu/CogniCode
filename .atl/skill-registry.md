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
refactor, git-versioning, doc-writer, cognicode-rules

## CogniCode Rules Development

**Skill**: `cognicode-rules` (`.claude/skills/cognicode-rules/SKILL.md`)
**Trigger**: When creating, modifying, testing, or auditing CogniCode detection rules in cognicode-axiom,
fixing false positives detected by dashboard, migrating regex rules to tree-sitter, or working with
rule catalogs and the quality analysis pipeline.

### Compact Rules (inject for rule development, testing, audit)

```
- ALWAYS use word boundaries in regex: (?:\b|_)des\b not just "des" or "sha1"
- ALWAYS skip comment lines in line-scanning rules (//, ///, //!, #, /*)
- ALWAYS add 3+ false positive tests per rule (comment, identifier, English word)
- RuleContext has tree_sitter::Tree — prefer queries over regex for structural patterns
- Dashboard is the FP feedback loop: monitor issues → fix rule → add test → verify on dashboard
- Tree-sitter queries match actual code nodes, never comments or strings
- 854 rules exist; <2% have tests; prioritize security/vulnerability rules for test coverage
- Use ctx.graph (CallGraph) and ctx.metrics (FileMetrics) for semantic context
- Self-improvement loop: FP report → fix rule → add regression test → verify → commit
- Migrate security rules (S2068, S4792, S5332) from regex to tree-sitter for accuracy
```

### Ecosystem Integration

| Component | Role |
|-----------|------|
| `cognicode-axiom` | Rule engine (854 rules, catalog, types) |
| `cognicode-quality` | Analysis handler (parses files, runs rules, persists to SQLite) |
| `cognicode-db` | Persistence (analysis_runs, issues table with status tracking) |
| `cognicode-dashboard` | Visualization (issue browser, metrics, quality gate, FP reports) |
| `cognicode-core` | Infrastructure (tree-sitter parser, call graph, semantic analysis) |
