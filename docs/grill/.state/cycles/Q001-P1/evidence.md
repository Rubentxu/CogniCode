# Q001-P1 — Evidence Packet

## Question
Can C1 (Tool Registry refactoring) begin independently of C4 (Schema/DTO unification)?

## Raw Evidence

### Proxy Evidence (from rmcp_adapter.rs analysis)
- 65 match arms in `rmcp_adapter.rs` (lines 1045–2018)
- Every arm follows pattern: `schemas::XxxInput` → handler → JSON string
- Zero `.into()`, `.try_into()`, or DTO references inside any arm
- Only 6 of 66 Input types (~9%) have DTO bridges (ReadFile, WriteFile, EditFile, SearchContent, ListFiles, partial GetFileSymbols/GetCallHierarchy)
- 60 Input types (~89%) have no DTO counterpart

### Skeptic Evidence (from codebase grep)
1. `schemas.rs` line 6 imports `OverviewDetail, OverviewMeta` from `crate::application::dto::analysis` — boundary already violated
2. `handle_get_hot_symbols` returns `HandlerResult<HotSymbolsResult>` where `HotSymbolsResult` is a DTO type (application/dto/analysis.rs:1061)
3. `BuildGraphInput` has 53 codebase references and lives in `handlers/mod.rs`, not `schemas.rs`
4. `dto_mapping.rs` has 26 `From` impls called by zero production code — dead code
5. `handle_get_module_dependencies` does manual field-by-field conversion from DTO → schema types, bypassing dto_mapping.rs

### Structural Assessment
- Layer leak: `schemas.rs` importing `application::dto` breaks MCP protocol layer independence
- Naming confusion: `OutlineNodeDto` and `SearchResultDto` live in `schemas.rs` but carry Dto suffix
- Naming anomaly: `BuildGraphInput` is the only Input type outside schemas with 53 references

## Synthesized Evidence
**Compilation dependency chain identified:** dispatch arm compiles against handler function signatures, not just internal bodies. When C4 restructures DTO types that handlers return, dispatch arms that call those handlers fail to compile. The Proxy's "dispatch arms don't mention DTOs" is true but irrelevant — the compilation dependency flows through the handler call.

## Recommended Action
Three preconditions before C1 can safely begin: (1) Move BuildGraphInput to schemas.rs, (2) Audit all handler return types for DTO leakage, (3) Remove schemas.rs's dependency on application::dto. OR: run C4 first, then C1.
