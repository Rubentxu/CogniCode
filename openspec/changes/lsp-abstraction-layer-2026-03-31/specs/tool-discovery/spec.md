# Tool Discovery Specification

## Purpose

Detect available language servers on the host system and provide actionable error messages when they are missing.

## Requirements

### Requirement: Language Server Registry

The system SHALL maintain a registry mapping each supported `Language` to its corresponding server binary name, expected args, and install command.

| Language   | Binary                         | Install Command                                              |
|------------|--------------------------------|--------------------------------------------------------------|
| Rust       | `rust-analyzer`                | `rustup component add rust-analyzer`                         |
| Python     | `pyright`                      | `npm install -g pyright`                                     |
| TypeScript | `typescript-language-server`   | `npm install -g typescript-language-server typescript`       |
| JavaScript | `typescript-language-server`   | (same as TypeScript)                                         |

#### Scenario: Registry lookup for supported language

- GIVEN the language Rust
- WHEN the registry is queried
- THEN it SHALL return binary `rust-analyzer` and install command `rustup component add rust-analyzer`

#### Scenario: Registry lookup for unsupported language

- GIVEN an unsupported language (e.g., Go)
- WHEN the registry is queried
- THEN it SHALL return `None`

### Requirement: Tool Availability Check

The system SHALL verify whether a language server binary is accessible on `PATH`.

#### Scenario: Server is installed

- GIVEN `rust-analyzer` is on `PATH`
- WHEN `check_availability(Language::Rust)` is called
- THEN it SHALL return `Available { binary_path, version }`

#### Scenario: Server is NOT installed

- GIVEN `pyright` is NOT on `PATH`
- WHEN `check_availability(Language::Python)` is called
- THEN it SHALL return `Unavailable { language, binary_name, install_command }`

### Requirement: Descriptive Error Messages

When a language server is unavailable, the system SHALL produce an error containing: language name, missing binary, and exact install command.

#### Scenario: Error message format

- GIVEN `rust-analyzer` is unavailable
- WHEN a user requests go-to-definition for a Rust file
- THEN the error SHALL include `"rust-analyzer is not installed"` and `"Install with: rustup component add rust-analyzer"`
- AND the error SHALL suggest tree-sitter fallback is available

### Requirement: Doctor Command

The system SHALL provide a `cognicode doctor` CLI command that checks all supported language servers and reports status in a table.

#### Scenario: All servers available

- GIVEN all language servers are installed
- WHEN `cognicode doctor` runs
- THEN it SHALL print a table with columns: Language, Server, Status, Install Command
- AND exit with code 0

#### Scenario: Some servers missing

- GIVEN `rust-analyzer` is installed but `pyright` is not
- WHEN `cognicode doctor` runs
- THEN it SHALL show Rust as Available and Python as Unavailable with install instructions
- AND exit with code 0 (informational, not error)
