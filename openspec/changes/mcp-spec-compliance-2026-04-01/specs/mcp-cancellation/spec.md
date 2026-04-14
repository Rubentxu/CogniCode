# MCP Cancellation Specification

## Purpose

Define how the MCP server handles in-flight request cancellation from clients, per the MCP specification's `notifications/cancelled` mechanism.

## Requirements

### Requirement: Cancellation Notification Handling

The server MUST accept `notifications/cancelled` notifications from the client. The notification `params` MUST contain a `requestId` field identifying the in-flight request to cancel. The server MUST stop processing the cancelled request as soon as practicable via cooperative cancellation. The server MUST NOT send a response for the cancelled request once cancellation is acknowledged.

#### Scenario: Cancel a running build_graph

- GIVEN the server is in Ready state and a `build_graph` request with id `42` is in flight
- WHEN the client sends `notifications/cancelled` with `params.requestId = 42`
- THEN the server aborts processing of request `42`
- AND the server does NOT send a response for request `42`

#### Scenario: Cancel a request that already completed

- GIVEN the server has already sent a response for request id `55`
- WHEN the client sends `notifications/cancelled` with `params.requestId = 55`
- THEN the notification is ignored (idempotent, no error)

#### Scenario: Cancel with unknown requestId

- GIVEN the server is in Ready state and no request with id `99` exists
- WHEN the client sends `notifications/cancelled` with `params.requestId = 99`
- THEN the notification is ignored (no error)

### Requirement: CancellationToken Propagation

The server MUST use a cooperative cancellation mechanism passed into every tool handler function. Handlers MUST check the cancellation token between major processing steps and abort early when cancellation is signalled.

#### Scenario: Long-running handler checks token between steps

- GIVEN a `build_graph` request is in flight and the handler is scanning files
- WHEN the cancellation token is signalled between scanning steps
- THEN the handler aborts mid-scan
- AND no response is sent for the cancelled request

#### Scenario: Fast handler completes before cancellation

- GIVEN a fast tool call (e.g., `get_symbols`) has already completed and returned a result
- WHEN a `notifications/cancelled` arrives for that request id after completion
- THEN the notification is ignored with no side effects

### Requirement: Cancellation Before Initialization

The server MUST ignore `notifications/cancelled` received before the server reaches the Ready state. The `initialize` request itself MUST NOT be cancellable.

#### Scenario: Cancel notification before initialization completes

- GIVEN the server is in Uninitialized or Initializing state
- WHEN the client sends `notifications/cancelled` with any `requestId`
- THEN the notification is ignored

#### Scenario: Cancel notification targets the initialize request

- GIVEN the server is processing an `initialize` request with id `1`
- WHEN the client sends `notifications/cancelled` with `params.requestId = 1`
- THEN the notification is ignored and initialization proceeds normally
