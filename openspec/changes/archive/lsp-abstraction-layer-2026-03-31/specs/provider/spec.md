# Provider Implementation Specification

## Purpose

Implement the `CodeIntelligenceProvider` trait with an LSP-backed provider and a tree-sitter fallback, composed with graceful degradation.

## Requirements

### Requirement: LSP Intelligence Provider

The system SHALL implement `CodeIntelligenceProvider` for `LspIntelligenceProvider` that routes requests to the appropriate LSP process.

#### Scenario: Go to definition via LSP

- GIVEN `rust-analyzer` is running and initialized
- WHEN `get_definition(Location)` is called for a Rust symbol
- THEN the system SHALL send `textDocument/definition` to rust-analyzer
- AND return the definition location

#### Scenario: Find references via LSP

- GIVEN `pyright` is running and initialized
- WHEN `find_references(Location, true)` is called for a Python symbol
- THEN the system SHALL send `textDocument/references` with `includeDeclaration: true`
- AND return typed references (Read, Write, Call, Type, Import)

#### Scenario: Hover via LSP

- GIVEN a running LSP server
- WHEN hover is requested at a position
- THEN the system SHALL send `textDocument/hover`
- AND return type info and documentation

### Requirement: Tree-sitter Fallback Provider

The system SHALL implement `CodeIntelligenceProvider` for `TreesitterFallbackProvider` using existing tree-sitter infrastructure.

#### Scenario: Fallback go-to-definition

- GIVEN no LSP server is available for the language
- WHEN `get_definition` is called
- THEN the system SHALL search the LightweightIndex for a symbol matching the identifier
- AND return the best-match location

#### Scenario: Fallback find-references

- GIVEN no LSP server is available
- WHEN `find_references` is called
- THEN the system SHALL use existing tree-sitter identifier search across project files
- AND return results with `ReferenceKind::Unknown`

#### Scenario: Fallback hover

- GIVEN no LSP server is available
- WHEN hover is requested
- THEN the system SHALL extract the symbol's source code via `SymbolCodeExtractor`
- AND return the code snippet (no type info)

### Requirement: Composite Provider with Graceful Degradation

The system SHALL implement a `CompositeProvider` that tries LSP first, then falls back to tree-sitter on any failure.

#### Scenario: LSP succeeds

- GIVEN LSP server is available and responsive
- WHEN any operation is requested
- THEN return the LSP result

#### Scenario: LSP unavailable, fallback succeeds

- GIVEN LSP server is not installed
- WHEN any operation is requested
- THEN log a warning: `"No LSP server for {language}, using tree-sitter fallback"`
- AND return the tree-sitter result

#### Scenario: Both fail

- GIVEN LSP server crashes AND tree-sitter finds no result
- WHEN an operation is requested
- THEN return a descriptive error combining both failure reasons

### Requirement: Process Restart on Crash

The system SHALL automatically restart a crashed LSP process on the next request, up to 3 times within 10 minutes.

#### Scenario: Auto-restart on crash

- GIVEN rust-analyzer crashes mid-session
- WHEN the next request arrives
- THEN the system SHALL attempt to respawn and reinitialize
- AND log: `"rust-analyzer crashed, restarting..."`

#### Scenario: Crash loop detection

- GIVEN rust-analyzer has crashed 3 times in 10 minutes
- WHEN the 4th crash occurs
- THEN the system SHALL mark the server as `Failed`
- AND use tree-sitter fallback for the rest of the session
- AND log: `"rust-analyzer failed 3 times, giving up for this session"`
