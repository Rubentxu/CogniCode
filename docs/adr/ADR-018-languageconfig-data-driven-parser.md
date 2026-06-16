# ADR-018: LanguageConfig Data-Driven Parser

**Status:** Accepted  
**Date:** 2026-06-15  
**Source:** grill-with-docs session — Graphify alignment

## Context

CogniCode's tree-sitter parser (`tree_sitter_parser.rs`) uses hard-coded match
arms for 6 languages. Each `Language` enum variant has dedicated match arms in
`function_node_type()`, `class_node_type()`, `call_node_type()`, and several
other methods. Adding a new language requires touching 5-6 methods.

The ingest pipeline (ADR-017) needs to support 36+ languages (matching
Graphify's coverage). Extending the match-arm approach to 36 languages would
produce ~200 match arms spread across 6 methods — unmaintainable.

## Decision

Adopt a **data-driven `LanguageConfig` pattern** inspired by Graphify. Each
language is described by a configuration struct consumed by a single generic
extractor.

```rust
pub struct LanguageConfig {
    pub language: Language,
    pub extensions: &'static [&'static str],
    pub function_types: &'static [&'static str],
    pub class_types: &'static [&'static str],
    pub import_types: &'static [&'static str],
    pub call_types: &'static [&'static str],
    pub import_handler: Option<ImportHandler>,
    // ... type-ref walker, call accessor config
}
```

One `const` config per language. The generic extractor walks the tree-sitter
AST using the config's node-type sets. Adding a language = adding a config,
not writing extraction code.

The 6 existing languages migrate from match arms to configs. The 30 new
languages are added as additional configs.

## Rationale

- **Scalability.** Adding a language touches exactly one file (the new config),
  not 6 methods across the parser.
- **Consistency.** Every language uses the same extraction logic. No per-
  language bugs from divergent match arms.
- **Graphify proven.** Graphify uses the exact same pattern (`LanguageConfig`
  dataclass) for 36+ languages successfully.
- **Testability.** The generic extractor is tested once. Per-language tests
  only verify config correctness (does `function_types` match the language's
  tree-sitter grammar?).

## Consequences

- The existing `Language` enum's `function_node_type()` / `class_node_type()`
  methods are deprecated in favor of config lookups.
- The generic extractor must handle edge cases that match arms currently
  special-case (e.g., C/C++ declarator unwrapping, JS arrow functions).
  Custom handlers (`import_handler`, `resolve_function_name_fn`) cover these.
- Initial migration of the 6 existing languages requires careful verification
  that the generic extractor produces identical output.

## Alternatives Considered

- **Extend match arms:** add 30 variants to `Language` + match arms. Rejected —
  does not scale; each new language touches 5-6 methods.
- **Tree-sitter queries (.scm):** each language has a query file. Rejected —
  adds runtime query compilation, harder to debug, and doesn't naturally
  express import resolution or type-reference walking.
