# Delta for MCP Tools

## MODIFIED Requirements

### Requirement: Tools List Response Format

The server MUST return tools in the format required by the MCP specification. Each tool entry in the `tools` array MUST contain exactly three fields: `name` (string), `description` (string), and `inputSchema` (valid JSON Schema object). The `inputSchema` MUST include `type: "object"` along with `properties` and `required` fields that accurately describe the tool's parameters.

(Previously: tools were returned in a similar format but individual tool schemas could be incomplete or non-standard, missing required JSON Schema fields.)

#### Scenario: Valid tools/list response

- GIVEN server is in Ready state
- WHEN client sends `tools/list` request
- THEN response contains a `tools` array
- AND each tool has `name`, `description`, and `inputSchema` fields
- AND each `inputSchema` has `type: "object"` with `properties` and `required`

#### Scenario: Empty tool list

- GIVEN server is in Ready state with no tools registered
- WHEN client sends `tools/list`
- THEN response contains `tools: []`

#### Scenario: tools/list before initialized

- GIVEN server has not received `notifications/initialized`
- WHEN client sends `tools/list`
- THEN server returns JSON-RPC error with code -32002

### Requirement: Tools Call Request Format

The server MUST parse `tools/call` requests with `params.name` (string, required) as the tool identifier and `params.arguments` (object, defaults to `{}`) as the tool input. The server MUST also accept an optional `params._meta` object containing a `progressToken` for progress tracking. The previous nested format where params contained a wrapper object MUST be replaced with the flat `{name, arguments, _meta?}` format defined by MCP spec.

(Previously: params were parsed as a wrapper object containing name and arguments in a nested structure, causing incompatibility with standard MCP clients.)

#### Scenario: Valid tools/call request

- GIVEN server is in Ready state
- WHEN client sends `tools/call` with params `{name: "build_graph", arguments: {directory: "/project"}}`
- THEN server invokes the `build_graph` handler with the provided arguments

#### Scenario: Tools/call with progress token

- GIVEN server is in Ready state
- WHEN client sends `tools/call` with params `{name: "build_graph", arguments: {directory: "/project"}, _meta: {progressToken: "abc123"}}`
- THEN server invokes the `build_graph` handler and emits `notifications/progress` during execution

#### Scenario: Missing tool name

- GIVEN server is in Ready state
- WHEN client sends `tools/call` with params `{arguments: {}}`
- THEN server returns JSON-RPC error with code -32602

#### Scenario: Unknown tool name

- GIVEN server is in Ready state
- WHEN client sends `tools/call` with params `{name: "nonexistent_tool"}`
- THEN server returns JSON-RPC error with code -32601

#### Scenario: Tools/call before initialized

- GIVEN server is NOT in Ready state
- WHEN client sends `tools/call`
- THEN server returns JSON-RPC error with code -32002

### Requirement: Tools Call Response Format

The server MUST return tool results as `{content: [{type: "text", text: "..."}]}`. On successful execution, `isError` MUST be absent or `false`. On tool execution error, the server MUST return a normal result (NOT a JSON-RPC error response) with `isError: true`: `{content: [{type: "text", text: "error message"}], isError: true}`.

(Previously: tool execution errors were returned as JSON-RPC error responses, which the MCP spec explicitly discourages for tool-level errors.)

#### Scenario: Successful tool result

- GIVEN tool executes successfully
- WHEN returning result
- THEN response has `result.content` array with `{type: "text", text: "..."}` objects
- AND `result.isError` is absent or `false`

#### Scenario: Tool execution error

- GIVEN tool execution fails with an error
- WHEN returning result
- THEN response has `result.content` array containing the error message as text
- AND `result.isError` is `true`
- AND the response is a normal JSON-RPC result, NOT a JSON-RPC error response

#### Scenario: Tool returns structured data

- GIVEN tool executes successfully and returns structured output
- WHEN returning result
- THEN response has `result.content` array with `{type: "text", text: "..."}` objects
- AND structured data is serialized as JSON within the `text` field
