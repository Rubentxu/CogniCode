# LSP End-to-End Testing Specification

## Purpose

Validate the complete LSP stack with real language server binaries (rust-analyzer, pyright) through integration tests.

## Requirements

### Requirement: Test Environment Detection

The system SHALL detect available LSP binaries and skip tests gracefully when unavailable.

#### Scenario: Binary available

- GIVEN `rust-analyzer` is installed and on PATH
- WHEN `rust_analyzer_available()` check is executed
- THEN the function SHALL return `true`

#### Scenario: Binary unavailable

- GIVEN `pyright` is not installed
- WHEN `pyright_available()` check is executed
- THEN the function SHALL return `false`
- AND the corresponding test SHALL be marked `#[ignore]`

### Requirement: Rust-Analyzer Integration Test

The system SHALL provide an integration test that validates hover operations with real rust-analyzer.

#### Scenario: Hover returns type information

- GIVEN a temporary Rust project with a function `fn greet() -> String`
- AND `rust-analyzer` binary is available
- WHEN hover is requested at the `greet` identifier
- THEN the response SHALL contain `fn greet() -> String`
- AND the test SHALL complete within 30 seconds

#### Scenario: Go-to-definition returns location

- GIVEN a temporary Rust project with a function defined at line 5
- AND `rust-analyzer` binary is available
- WHEN go-to-definition is requested at a call site
- THEN the response SHALL contain `line: 5` (or 1-indexed equivalent)

### Requirement: Pyright Integration Test

The system SHALL provide an integration test that validates go-to-definition with real pyright.

#### Scenario: Definition returns correct location

- GIVEN a temporary Python project with a function `def calculate(x):` at line 3
- AND `pyright` binary is available
- WHEN go-to-definition is requested at a call site `calculate(5)`
- THEN the response SHALL point to line 3 of the definition file

#### Scenario: Find references returns all usages

- GIVEN a Python project with a variable used in 3 locations
- AND `pyright` binary is available
- WHEN find-references is requested at the variable definition
- THEN the response SHALL contain exactly 3 locations

### Requirement: Test Isolation

The system SHALL isolate integration tests from each other and the system environment.

#### Scenario: Temporary project per test

- GIVEN an integration test starts
- WHEN the test creates a project
- THEN it SHALL use `tempfile::tempdir()`
- AND cleanup SHALL occur automatically on test completion

#### Scenario: No state leakage between tests

- GIVEN test A spawns rust-analyzer
- WHEN test B starts
- THEN test B SHALL NOT reuse any state from test A
- AND each test SHALL create a fresh `LspProxyService`

### Requirement: Timeout Handling

The system SHALL timeout gracefully when LSP servers are unresponsive.

#### Scenario: LSP server hangs

- GIVEN an LSP server does not respond within 10 seconds
- WHEN a request is made
- THEN the system SHALL return a timeout error
- AND NOT hang indefinitely

#### Scenario: Process cleanup on timeout

- GIVEN an LSP process is spawned
- WHEN a timeout occurs
- THEN the system SHALL terminate the child process
- AND release all resources
