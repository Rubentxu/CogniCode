# LSP Proxy Service Integration Specification

## Purpose

Wire `CompositeProvider` into `LspProxyService.route_operation()` to delegate LSP operations (hover, definition, references) instead of returning `None`.

## Requirements

### Requirement: CompositeProvider Storage

The system SHALL store an `Arc<CompositeProvider>` in `LspProxyService` for shared ownership across requests.

#### Scenario: Provider initialization with workspace

- GIVEN `LspProxyService` is constructed with a valid `workspace_root`
- WHEN `enable_proxy_mode_with_provider()` is called
- THEN the system SHALL create a `CompositeProvider` with the workspace root
- AND store it as `Option<Arc<CompositeProvider>>`

#### Scenario: Provider absent when not enabled

- GIVEN `LspProxyService` is constructed without enabling proxy mode
- WHEN `route_operation()` is called
- THEN `composite_provider` SHALL be `None`
- AND the method SHALL return `Ok(None)`

### Requirement: Operation Routing

The system SHALL map string-based operation names to `CompositeProvider` trait method calls.

#### Scenario: Hover operation routing

- GIVEN `composite_provider` is `Some`
- WHEN `route_operation("hover", params)` is called with valid LSP params
- THEN the system SHALL extract `Location` from params
- AND call `CompositeProvider::hover(&location)`
- AND return the serialized `HoverInfo` as JSON

#### Scenario: Definition operation routing

- GIVEN `composite_provider` is `Some`
- WHEN `route_operation("textDocument/definition", params)` is called
- THEN the system SHALL call `CompositeProvider::get_definition(&location)`
- AND return the `Location` as JSON if found

#### Scenario: References operation routing

- GIVEN `composite_provider` is `Some`
- WHEN `route_operation("find_references", params)` is called
- THEN the system SHALL call `CompositeProvider::find_references(&location, true)`
- AND return the `Vec<Reference>` as JSON

#### Scenario: Unknown operation returns None

- GIVEN any proxy state
- WHEN `route_operation("unknown_operation", params)` is called
- THEN the system SHALL return `Ok(None)`
- AND NOT throw an error

### Requirement: Location Extraction

The system SHALL parse LSP-style params into `Location` values.

#### Scenario: Standard LSP params parsing

- GIVEN params contain `{ "textDocument": { "uri": "file:///path/to/file.rs" }, "position": { "line": 10, "character": 5 } }`
- WHEN `extract_location(params)` is called
- THEN the system SHALL return `Location::new("/path/to/file.rs", 11, 6)` (1-indexed)

#### Scenario: Malformed params error

- GIVEN params are missing required fields
- WHEN `extract_location(params)` is called
- THEN the system SHALL return `Err(LspProxyError::InvalidParams)`

### Requirement: Backward Compatibility

The system SHALL preserve existing behavior when proxy mode is disabled.

#### Scenario: Disabled proxy returns None

- GIVEN `proxy_enabled` is `false`
- WHEN `route_operation()` is called with any operation
- THEN the system SHALL return `Ok(None)`
- AND NOT access `composite_provider`

#### Scenario: Existing tests pass unchanged

- GIVEN existing unit tests for `LspProxyService`
- WHEN the implementation is modified
- THEN all existing tests SHALL pass without modification
