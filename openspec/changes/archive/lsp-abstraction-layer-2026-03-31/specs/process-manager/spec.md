# Process Manager Specification

## Purpose

Manage the lifecycle of external LSP server processes: spawn, initialize, communicate, and shut down.

## Requirements

### Requirement: LSP Process Spawning

The system SHALL spawn an LSP server as an async child process communicating via stdin/stdout using JSON-RPC 2.0.

#### Scenario: Successful spawn

- GIVEN `rust-analyzer` is available on PATH
- WHEN `spawn_server(Language::Rust, workspace_root)` is called
- THEN the system SHALL start the process with appropriate args
- AND return an `LspProcessHandle` with stdin/stdout channels

#### Scenario: Spawn failure

- GIVEN the binary exists but fails to start
- WHEN spawn is attempted
- THEN the system SHALL return `LspError::SpawnFailed` with stderr output

### Requirement: LSP Initialization Handshake

After spawning, the system MUST send `initialize` request and receive `ServerCapabilities` before routing operations.

#### Scenario: Successful initialization

- GIVEN a spawned rust-analyzer process
- WHEN `initialize` is sent with `root_uri` and `ClientCapabilities`
- THEN the system SHALL store the returned `ServerCapabilities`
- AND send `initialized` notification

#### Scenario: Initialization timeout

- GIVEN a spawned process that does not respond within 30 seconds
- WHEN the timeout fires
- THEN the system SHALL kill the process
- AND return `LspError::Timeout`

### Requirement: Capability Validation

The system SHALL check `ServerCapabilities` before routing operations. If the server does not support an operation, the system MUST fall back to tree-sitter.

#### Scenario: Capability supported

- GIVEN `ServerCapabilities.definition_provider = Some(true)`
- WHEN a go-to-definition request is made
- THEN the system SHALL route to the LSP server

#### Scenario: Capability NOT supported

- GIVEN `ServerCapabilities.hover_provider = None`
- WHEN a hover request is made
- THEN the system SHALL fall back to tree-sitter

### Requirement: Graceful Shutdown

The system SHALL shut down LSP processes gracefully: send `shutdown` request, wait for response, send `exit` notification, kill after 5s if still running.

#### Scenario: Graceful shutdown

- GIVEN an active LSP process
- WHEN `shutdown()` is called
- THEN send `shutdown` request, wait for response, send `exit` notification

#### Scenario: Forced kill on hang

- GIVEN an active LSP process that does not respond to shutdown within 5s
- WHEN the timeout fires
- THEN the system SHALL force-kill the process

### Requirement: Idle Timeout

The system SHALL shut down LSP processes after 5 minutes of inactivity to conserve resources.

#### Scenario: Idle process auto-shutdown

- GIVEN an LSP process with no requests for 5 minutes
- WHEN the idle timer fires
- THEN the system SHALL perform graceful shutdown

#### Scenario: Request resets timer

- GIVEN an idle LSP process at 4m50s
- WHEN a new request arrives
- THEN the idle timer SHALL reset to 5 minutes

### Requirement: Request Timeout

Every LSP request SHALL have a configurable timeout (default 30s). On timeout, fall back to tree-sitter.

#### Scenario: Request timeout

- GIVEN an LSP request with no response for 30s
- WHEN the timeout fires
- THEN log a warning and return `LspError::Timeout`
- AND the caller SHALL fall back to tree-sitter
