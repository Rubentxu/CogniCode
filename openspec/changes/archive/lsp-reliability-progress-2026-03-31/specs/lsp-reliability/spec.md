# LSP Reliability & Progress Specification

## Purpose

Provide structured status tracking, progress reporting, and graceful fallback for LSP server lifecycle management.

---

## Requirements

### Requirement: ServerStatus State Machine

The system SHALL track LSP server lifecycle through a `ServerStatus` enum with states: `Starting`, `Indexing`, `Ready`, `Busy`, `Crashed`.

#### Scenario: Initial state is Starting

- GIVEN an LSP process is spawned
- WHEN the process starts but has not completed initialization
- THEN `ServerStatus` SHALL be `Starting`

#### Scenario: Transition to Indexing on progress notification

- GIVEN server status is `Starting` or `Ready`
- WHEN an LSP `$/progress` notification with `workDoneProgress` is received
- THEN status SHALL transition to `Indexing { progress }` with the reported percentage

#### Scenario: Transition to Ready after initialization

- GIVEN server status is `Starting` or `Indexing`
- WHEN the LSP `initialized` handshake completes
- AND no indexing is in progress
- THEN status SHALL transition to `Ready`

#### Scenario: Transition to Busy during request handling

- GIVEN server status is `Ready`
- WHEN a request is sent to the LSP server
- THEN status SHALL transition to `Busy`
- AND SHALL return to `Ready` after response is received

#### Scenario: Transition to Crashed on process death

- GIVEN server status is any non-Crashed state
- WHEN the LSP process exits unexpectedly
- THEN status SHALL transition to `Crashed { reason }`
- AND the reason SHALL include exit code or signal if available

---

### Requirement: ProgressUpdate Callback System

The system SHALL report progress updates via a callback mechanism during long operations.

#### Scenario: Callback invoked during wait

- GIVEN `wait_for_ready` is called with a progress callback
- WHEN the server status changes or polling occurs
- THEN the callback SHALL be invoked with a `ProgressUpdate` struct

#### Scenario: ProgressUpdate contains required fields

- GIVEN a progress callback is invoked
- WHEN the callback receives a `ProgressUpdate`
- THEN it SHALL contain `message: String`
- AND `percentage: Option<f32>` (0.0-100.0)
- AND `status: ServerStatus`

#### Scenario: Callback is optional

- GIVEN `wait_for_ready` is called with `None` for callback
- WHEN the server becomes ready
- THEN the function SHALL return normally without invoking any callback

---

### Requirement: Wait-for-Ready with Timeout

The system SHALL provide `wait_for_ready(timeout, callback)` that blocks until the server is ready or timeout expires.

#### Scenario: Returns Ready when server initializes quickly

- GIVEN an LSP server that initializes in 5 seconds
- WHEN `wait_for_ready(timeout=30)` is called
- THEN the function SHALL return `Ok(ServerStatus::Ready)` within 5 seconds

#### Scenario: Returns ServerNotReady on timeout

- GIVEN an LSP server that is still indexing after 30 seconds
- WHEN `wait_for_ready(timeout=30)` is called
- THEN the function SHALL return `Err(LspProcessError::ServerNotReady { status, waited_secs: 30 })`

#### Scenario: Polling interval for status checks

- GIVEN `wait_for_ready` is polling for readiness
- WHEN checking server status
- THEN the system SHALL poll at approximately 500ms intervals

#### Scenario: Returns immediately if already ready

- GIVEN server status is already `Ready`
- WHEN `wait_for_ready(timeout=30)` is called
- THEN the function SHALL return `Ok(ServerStatus::Ready)` immediately without waiting

---

### Requirement: Graceful Tree-Sitter Fallback

The system SHALL fall back to tree-sitter parsing with a structured error message when LSP is unavailable.

#### Scenario: Fallback on timeout with reason

- GIVEN `wait_for_ready` times out after configured duration
- WHEN `CompositeProvider` receives `ServerNotReady` error
- THEN it SHALL log a warning with the status
- AND invoke tree-sitter fallback provider
- AND return result with `fallback_reason: Some("LSP server not ready after 30s: Indexing")`

#### Scenario: Fallback on server crash

- GIVEN the LSP server crashes during a request
- WHEN `CompositeProvider` receives `ServerCrashed` error
- THEN it SHALL record the crash
- AND fall back to tree-sitter with `fallback_reason: Some("LSP server crashed: {reason}")`

#### Scenario: Fallback result includes reason

- GIVEN a fallback to tree-sitter occurs
- WHEN the result is returned to the caller
- THEN the `FallbackResult<T>` SHALL include `fallback_reason: Option<String>` explaining why fallback was used

---

### Requirement: Cancellation Support

The system SHALL support cooperative cancellation of long-running LSP requests.

#### Scenario: Request cancelled via token

- GIVEN a request is in progress with a cancellation token
- WHEN the token is triggered (e.g., via `CancellationToken::cancel()`)
- THEN the request SHALL return `Err(LspProcessError::Cancelled { method })`

#### Scenario: Cancelled request cleans up resources

- GIVEN a request is cancelled
- WHEN cancellation occurs
- THEN any in-flight LSP requests SHALL be cleaned up
- AND the server status SHALL return to `Ready`

#### Scenario: Cancellation during wait_for_ready

- GIVEN `wait_for_ready` is polling for readiness
- WHEN cancellation is requested
- THEN the function SHALL return `Err(LspProcessError::Cancelled { method: "wait_for_ready" })`

---

### Requirement: Structured LspProcessError

The system SHALL provide specific error variants for each failure mode.

#### Scenario: ServerNotReady error structure

- GIVEN server is not ready after timeout
- WHEN the error is returned
- THEN `LspProcessError::ServerNotReady` SHALL contain `status: ServerStatus` and `waited_secs: u64`

#### Scenario: ServerCrashed error structure

- GIVEN server process dies
- WHEN the error is returned
- THEN `LspProcessError::ServerCrashed` SHALL contain `reason: String` and `crash_count: u32`

#### Scenario: RequestTimeout error structure

- GIVEN a request exceeds its timeout
- WHEN the error is returned
- THEN `LspProcessError::RequestTimeout` SHALL contain `method: String` and `waited_secs: u64`

#### Scenario: Cancelled error structure

- GIVEN a request is cancelled
- WHEN the error is returned
- THEN `LspProcessError::Cancelled` SHALL contain `method: String`
