# MCP Logging Specification

## Purpose

Define how the MCP server exposes client-controlled log levels and emits structured log messages to clients via `notifications/message`. This enables MCP clients (Claude Desktop, Cursor, etc.) to control verbosity and receive server-side log events in real time.

## Requirements

### Requirement: Logging Capability Declaration

The server MUST include `logging: {}` in the `capabilities` object of its `initialize` response. This declares that the server supports the MCP logging protocol.

#### Scenario: Initialize response includes logging capability

- GIVEN the server is starting a new session
- WHEN the server responds to an `initialize` request
- THEN the response `capabilities` object MUST contain `"logging": {}`

### Requirement: Set Log Level

The server MUST handle `logging/setLevel` requests from the client.

The `params` object MUST contain a `level` field with one of the following values: `"debug"`, `"info"`, `"notice"`, `"warning"`, `"error"`, `"critical"`, `"alert"`, `"emergency"`.

The server MUST map the MCP level to the corresponding `tracing` level and apply it to the active tracing subscriber.

The server MUST respond with an empty result `{}` on success.

If the `level` value is not one of the valid MCP log levels, the server MUST return a JSON-RPC error with code `-32602` (Invalid params).

If the server has not yet reached the `initialized` state, the server MUST return a JSON-RPC error with code `-32002` (Server not initialized).

#### Scenario: Client sets level to debug

- GIVEN the server is in the `initialized` state
- WHEN the client sends `logging/setLevel` with `params.level` = `"debug"`
- THEN the server MUST set its tracing level to DEBUG
- AND respond with `{}`

#### Scenario: Client sets level to error

- GIVEN the server is in the `initialized` state
- WHEN the client sends `logging/setLevel` with `params.level` = `"error"`
- THEN the server MUST set its tracing level to ERROR
- AND respond with `{}`

#### Scenario: Client sends invalid level string

- GIVEN the server is in the `initialized` state
- WHEN the client sends `logging/setLevel` with `params.level` = `"verbose"`
- THEN the server MUST respond with error code `-32602`

#### Scenario: Client sends setLevel before initialization completes

- GIVEN the server has NOT completed the initialization handshake
- WHEN the client sends `logging/setLevel`
- THEN the server MUST respond with error code `-32002`

### Requirement: Log Message Notifications

The server MAY send `notifications/message` to the client when log events occur.

The notification `params` MUST contain a `level` field (string, valid MCP log level) and a `data` field (string, the log message).

The server MUST emit these notifications as JSON-RPC messages on stdout — NOT on stderr.

The server MUST NOT send log notifications with a level below the currently configured level set by `logging/setLevel`. The default level before any `setLevel` call SHALL be `"info"`.

#### Scenario: Debug notification emitted after setting debug level

- GIVEN the server's current log level is `"debug"`
- WHEN a debug-level event occurs in the server
- THEN the server MUST send a `notifications/message` with `params.level` = `"debug"` and `params.data` containing the event description

#### Scenario: Debug notification suppressed at error level

- GIVEN the server's current log level is `"error"`
- WHEN a debug-level or info-level event occurs in the server
- THEN the server MUST NOT send any `notifications/message` for that event

#### Scenario: Notification format compliance

- GIVEN the server emits a log notification
- WHEN the notification is serialized
- THEN it MUST conform to: `{"jsonrpc":"2.0","method":"notifications/message","params":{"level":"<level>","data":"<message>"}}`
- AND the message MUST be written to stdout

#### Scenario: Default level before setLevel

- GIVEN the server has completed initialization but received no `logging/setLevel` request
- WHEN a debug-level event occurs
- THEN the server MUST NOT send a `notifications/message` for that event
- WHEN an info-level event occurs
- THEN the server MAY send a `notifications/message` with `params.level` = `"info"`
