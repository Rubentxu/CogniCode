# CLI Exposure Specification

## Purpose

Expose code intelligence operations as CLI commands for direct user consumption.

## Requirements

### Requirement: navigate Subcommand

The system SHALL provide `cognicode navigate` with subcommands: `definition`, `hover`, `references`.

#### Scenario: Navigate to definition

- GIVEN file `src/main.rs` with a function call at line 25, column 10
- WHEN `cognicode navigate definition src/main.rs:25:10` is executed
- THEN print definition location: `src/lib.rs:42:5`
- AND print 3 lines of context around the definition

#### Scenario: Hover from CLI

- GIVEN file `src/main.rs` with a variable at line 15, column 8
- WHEN `cognicode navigate hover src/main.rs:15:8` is executed
- THEN print the type information and any documentation

#### Scenario: Find references from CLI

- GIVEN a symbol with references across the project
- WHEN `cognicode navigate references src/main.rs:10:5` is executed
- THEN print a table with columns: Location, Kind, Context

### Requirement: doctor Subcommand

The system SHALL provide `cognicode doctor` that reports status of all language servers.

#### Scenario: Doctor output table

- WHEN `cognicode doctor` is executed
- THEN print a table:

```
Language    | Server                      | Status      | Install Command
Rust        | rust-analyzer               | Available   | rustup component add rust-analyzer
Python      | pyright                     | Missing     | npm install -g pyright
TypeScript  | typescript-language-server  | Available   | npm install -g typescript-language-server
JavaScript  | typescript-language-server  | Available   | (same as TypeScript)
```

#### Scenario: JSON output format

- WHEN `cognicode doctor --format json` is executed
- THEN output a JSON array of `{ language, server, available, binary_path, install_command }`
