# MCP Exposure Specification

## Purpose

Expose code intelligence operations as MCP tools for AI agent consumption.

## Requirements

### Requirement: go_to_definition MCP Tool

The system SHALL register an MCP tool `go_to_definition` accepting `file_path`, `line`, `column` and returning the definition location with context.

#### Scenario: Definition found

- GIVEN a valid file position on a symbol usage
- WHEN `go_to_definition` is called
- THEN return `{ file, line, column, context: "3 lines around definition" }`

#### Scenario: No definition found

- GIVEN a position on whitespace or literal
- WHEN `go_to_definition` is called
- THEN return `{ found: false }`

#### Scenario: Multiple definitions (generics/macros)

- GIVEN a position that resolves to multiple definitions
- WHEN `go_to_definition` is called
- THEN return an array of locations with a note: `"Multiple definitions found"`

### Requirement: hover MCP Tool

The system SHALL register an MCP tool `hover` accepting `file_path`, `line`, `column` and returning type info and documentation.

#### Scenario: Symbol with type info

- GIVEN a position on a typed variable
- WHEN `hover` is called
- THEN return `{ type: "String", documentation: "..." }`

#### Scenario: No type info

- GIVEN a position on a keyword or whitespace
- WHEN `hover` is called
- THEN return `{ found: false }`

### Requirement: find_references MCP Tool

The system SHALL register an MCP tool `find_references` accepting `file_path`, `line`, `column` and returning all references.

#### Scenario: References found

- GIVEN a position on a function definition with 5 usages
- WHEN `find_references` is called
- THEN return array of `{ file, line, column, context, kind }` with 5 entries

#### Scenario: No references

- GIVEN a position on an unused private function
- WHEN `find_references` is called
- THEN return empty array

### Requirement: LSP Status in Tool Descriptions

The system SHALL include LSP availability status in MCP tool descriptions so agents know which features are available.

#### Scenario: Tool description reflects availability

- GIVEN rust-analyzer is installed but pyright is not
- WHEN `tools/list` is called
- THEN tool descriptions SHALL note: `"Semantic analysis available for: Rust. Tree-sitter fallback for: Python, TypeScript, JavaScript."`
