# MCP Lifecycle Specification

## Purpose

Define the MCP server lifecycle: initialization handshake, capability negotiation, state transitions, and liveness checks per the Model Context Protocol specification (2025-03-26).

## Requirements

### Requirement: Initialize Handshake

The server MUST respond to `initialize` requests by parsing the client's `protocolVersion`, `capabilities`, and `clientInfo` from `params`. The server MUST respond with its own `protocolVersion` (the highest version it supports that is <= the client's), `capabilities`, and `serverInfo` (name: "cognicode", version from `CARGO_PKG_VERSION`). The server MUST declare `tools` capability with `listChanged: false` and `logging` capability.

#### Scenario: Valid initialize request

- GIVEN the server is in Uninitialized state
- WHEN the client sends `initialize` with `params.protocolVersion = "2025-03-26"`, `params.capabilities`, and `params.clientInfo = {name: "test-client", version: "1.0"}`
- THEN the server responds with result containing `protocolVersion`, `capabilities.tools` and `capabilities.logging`, and `serverInfo = {name: "cognicode", version: <Cargo.toml version>}`
- AND the server transitions to Initializing state

#### Scenario: Missing clientInfo

- GIVEN the server is in Uninitialized state
- WHEN the client sends `initialize` with `params` that omit `clientInfo`
- THEN the server responds with error code `-32602` (Invalid params)

#### Scenario: Client requests unsupported protocol version

- GIVEN the server is in Uninitialized state
- WHEN the client sends `initialize` with `params.protocolVersion = "2099-01-01"` (a version the server does not support)
- THEN the server responds with its own highest supported `protocolVersion`

### Requirement: Initialized Notification

After the client receives the `initialize` response, it MUST send a `notifications/initialized` notification. The server MUST accept this notification without sending a response. Until `notifications/initialized` is received, the server MUST reject all requests except `initialize` and `ping` with error `-32002` (Server not initialized).

#### Scenario: Client sends initialized after initialize

- GIVEN the server is in Initializing state (after returning an `initialize` response)
- WHEN the client sends `notifications/initialized` (no id, no response expected)
- THEN the server transitions to Ready state
- AND the server does NOT send a response

#### Scenario: Tool call before initialized notification

- GIVEN the server is in Initializing state (has responded to `initialize` but not yet received `notifications/initialized`)
- WHEN the client sends a `tools/call` request
- THEN the server responds with error code `-32002` and message "Server not initialized"

#### Scenario: Double initialize after initialized

- GIVEN the server is in Ready state
- WHEN the client sends another `initialize` request
- THEN the server responds with error code `-32600` (Invalid request) indicating the server is already initialized

### Requirement: Server State Machine

The server MUST maintain a state machine with three states: `Uninitialized`, `Initializing`, `Ready`. Transitions: `Uninitialized` -> `Initializing` (on `initialize` request success), `Initializing` -> `Ready` (on `notifications/initialized`). The state MUST be protected by `tokio::sync::RwLock` for concurrent safety.

#### Scenario: Initial state is Uninitialized

- GIVEN the server has just started
- WHEN the state is queried
- THEN the state is `Uninitialized`

#### Scenario: Initialize transitions to Initializing

- GIVEN the server is in Uninitialized state
- WHEN a valid `initialize` request is processed successfully
- THEN the state transitions to `Initializing`

#### Scenario: Notifications/initialized transitions to Ready

- GIVEN the server is in Initializing state
- WHEN a `notifications/initialized` notification is received
- THEN the state transitions to `Ready`

#### Scenario: Double initialize returns error

- GIVEN the server is in Ready state
- WHEN an `initialize` request is received
- THEN the server responds with an error and the state remains `Ready`

### Requirement: Ping/Pong

The server MUST respond to `ping` requests with an empty result `{}` in any state. Ping MUST work before initialization completes. This is required by the MCP spec for liveness checks.

#### Scenario: Ping before initialize

- GIVEN the server is in Uninitialized state
- WHEN the client sends a `ping` request
- THEN the server responds with result `{}`

#### Scenario: Ping after initialized

- GIVEN the server is in Ready state
- WHEN the client sends a `ping` request
- THEN the server responds with result `{}`

#### Scenario: Ping with string id

- GIVEN the server is in any state
- WHEN the client sends a `ping` request with `id = "my-corr-id"`
- THEN the server responds with result `{}` and `id = "my-corr-id"`

### Requirement: CLI Working Directory

The `cognicode-mcp` binary MUST accept `--cwd <path>` as a CLI argument to set the project root directory. If not provided, it MUST default to the current working directory. The HandlerContext MUST be initialized with this directory.

#### Scenario: --cwd sets the root

- GIVEN the binary is launched with `--cwd /path/to/project`
- WHEN the server starts and creates a HandlerContext
- THEN the HandlerContext root directory is `/path/to/project`

#### Scenario: No --cwd defaults to current directory

- GIVEN the binary is launched without `--cwd`
- WHEN the server starts and creates a HandlerContext
- THEN the HandlerContext root directory is the current working directory

#### Scenario: Invalid --cwd path

- GIVEN the binary is launched with `--cwd /nonexistent/path`
- WHEN the server attempts to start
- THEN the server exits with an error message to stderr
