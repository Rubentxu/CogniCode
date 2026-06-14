# Goal Model — Architectural Deepening Candidates

## Explicit Goal
Design implementable solutions for 6 architectural deepening candidates identified in `cognicode-core`, producing documented decisions ready for implementation.

## Target Users
- CogniCode maintainers (developers extending the MCP interface)
- AI agents navigating the codebase (improved locality = easier AI understanding)

## Context
The CogniCode codebase follows hexagonal architecture with clean domain layer. 
The MCP interface layer has grown into shallow, wide modules with high boilerplate-to-value ratio.

## Non-Goals
- Not modifying domain layer architecture (already deep)
- Not adding new features
- Not changing public API contracts
- Not creating production SQL migrations

## 6 Candidates (ranked)
1. **rmcp_adapter Tool Registry** — ~1200 lines of dispatch boilerplate → declarative macro
2. **HandlerContext Builder** — 7 telescoping constructors → builder pattern
3. **SKIP_DIRS Consolidation** — identical constant duplicated 5 times → single source
4. **Schema/DTO Unification** — ~5400 lines of near-duplicate types → type aliases
5. **file_operations ReadMode Strategy** — monolithic read_file → trait-based strategies
6. **Mocks in Domain Traits** — ~370 lines of test mocks in trait files → infrastructure/testing/

## Goal Model
- Q: What patterns does each candidate need?
- Q: Which candidates have dependencies on each other?
- Q: What's the migration path (incremental vs big-bang)?
- Q: How do we validate without production changes?
- Q: What macro/API design for the tool registry?
- Q: How does the builder interact with existing HandlerContext usage?
- Q: Does SKIP_DIRS consolidation need a new domain concept?
- Q: Where exactly do schemas and DTOs diverge vs align?
- Q: What trait interface for ReadMode strategies?
- Q: Does mock relocation affect cross-crate testability?
