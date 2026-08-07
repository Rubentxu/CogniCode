# MCP Progress Specification

## Purpose

Define how the CogniCode MCP server reports progress for long-running tool operations using the MCP progress token mechanism, enabling clients to display progress bars and status messages during expensive computations like graph building and index construction.

## Requirements

### Requirement: Progress Token Extraction

The server MUST extract `_meta.progressToken` from the `params` object of any `tools/call` request. The token MAY be a string or integer per the MCP specification. If `_meta` is absent or `_meta.progressToken` is not present, the server MUST NOT emit any progress notifications for that request.

#### Scenario: String progress token extracted

- GIVEN a `tools/call` request with `params._meta.progressToken` set to `"abc123"`
- WHEN the server processes the request
- THEN the server SHALL extract the token value `"abc123"`
- AND use it for all subsequent progress notifications for this request

#### Scenario: Integer progress token extracted

- GIVEN a `tools/call` request with `params._meta.progressToken` set to `42`
- WHEN the server processes the request
- THEN the server SHALL extract the token value `42`
- AND use it for all subsequent progress notifications for this request

#### Scenario: No _meta field present

- GIVEN a `tools/call` request without a `_meta` field in params
- WHEN the server processes the request
- THEN the server SHALL execute the tool normally
- AND SHALL NOT emit any `notifications/progress` messages

#### Scenario: _meta present but no progressToken

- GIVEN a `tools/call` request with `params._meta` containing other fields but no `progressToken`
- WHEN the server processes the request
- THEN the server SHALL NOT emit any `notifications/progress` messages

### Requirement: Progress Notification Emission

When a progress token is present and the tool is progress-eligible, the server MUST emit `notifications/progress` messages to stdout. Each notification MUST include `progressToken` matching the request token, `progress` as the current value, and MAY include `total` and `message`. The server MUST emit at least one notification at operation start and one at completion.

#### Scenario: Multi-step progress for long operation

- GIVEN a `tools/call` for `build_graph` with `progressToken` set to `"tok1"`
- WHEN the server executes the operation across multiple phases
- THEN the server SHALL emit `notifications/progress` with `progressToken: "tok1"` at least at start, during execution, and at completion
- AND each notification SHALL conform to the JSON-RPC format: `{"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"tok1","progress":<value>,"total":<value>,"message":"<text>"}}`

#### Scenario: Fast tool with progress token emits minimal notifications

- GIVEN a `tools/call` for a progress-eligible tool that completes in under 1 second
- WHEN a `progressToken` is present
- THEN the server SHALL emit at minimum a start notification and a completion notification

#### Scenario: Completion notification marks 100%

- GIVEN any progress-eligible tool call with a progress token
- WHEN the operation completes successfully
- THEN the server SHALL emit a final `notifications/progress` with `progress` equal to `total` (or a terminal value)

#### Scenario: Progress notification does not block tool execution

- GIVEN a progress-eligible tool call is executing
- WHEN progress notifications are emitted
- THEN the tool execution SHALL NOT be blocked or delayed by notification emission

### Requirement: Long-Running Tool Identification

Tools that MAY take longer than 1 second SHOULD emit progress notifications when a progress token is provided. The progress-eligible tools are: `build_graph`, `build_lightweight_index`, `build_call_subgraph`, `get_per_file_graph`, `merge_file_graphs`, `export_mermaid`. Tools NOT in this list SHOULD NOT emit progress notifications even if a token is provided.

#### Scenario: build_graph with progress token

- GIVEN a `tools/call` for `build_graph` with `progressToken` present
- WHEN the server executes the tool
- THEN the server SHALL emit `notifications/progress` during execution

#### Scenario: build_lightweight_index with progress token

- GIVEN a `tools/call` for `build_lightweight_index` with `progressToken` present
- WHEN the server executes the tool
- THEN the server SHALL emit `notifications/progress` during execution

#### Scenario: build_call_subgraph with progress token

- GIVEN a `tools/call` for `build_call_subgraph` with `progressToken` present
- WHEN the server executes the tool
- THEN the server SHALL emit `notifications/progress` during execution

#### Scenario: Fast tool with progress token emits nothing

- GIVEN a `tools/call` for `find_usages` with `progressToken` present
- WHEN the server executes the tool
- THEN the server SHALL NOT emit any `notifications/progress`

#### Scenario: Fast tool completion unaffected by token presence

- GIVEN a `tools/call` for a non-progress-eligible tool with `progressToken` present
- WHEN the server completes the tool call
- THEN the result SHALL be identical to a call without a progress token

### Requirement: Progress Notification Format Compliance

Each `notifications/progress` message MUST be a valid JSON-RPC notification with `method` set to `"notifications/progress"` and `params` containing `progressToken` and `progress`. The `params` object MAY include `total` (the expected final value) and `message` (a human-readable description).

#### Scenario: Valid notification structure

- GIVEN any progress notification is emitted
- WHEN the notification is serialized to stdout
- THEN it SHALL be a valid JSON-RPC 2.0 notification with no `id` field
- AND `params.progressToken` SHALL match the token from the original request
- AND `params.progress` SHALL be a non-negative number
- AND `params.total` SHALL be a positive number when present

#### Scenario: Message field is optional

- GIVEN a progress notification is emitted
- WHEN no human-readable message is available
- THEN the notification MAY omit the `message` field from `params`
