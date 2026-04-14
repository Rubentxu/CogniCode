# Tasks: LSP Integration Fixup & End-to-End Testing

## Phase 1: LspProxyService Foundation

- [x] 1.1 Add `composite_provider: Option<Arc<CompositeProvider>>` field to `LspProxyService` in `src/application/services/lsp_proxy_service.rs`
- [x] 1.2 Add `workspace_root: Option<PathBuf>` field to `LspProxyService`
- [x] 1.3 Update `LspProxyService::new()` to accept `workspace_root: PathBuf` parameter
- [x] 1.4 Add `extract_location(&self, params: &serde_json::Value) -> Result<Location, LspProxyError>` helper method

## Phase 2: route_operation Implementation

- [x] 2.1 Implement `route_operation()` to match operation strings ("hover", "textDocument/hover", "textDocument/definition", "find_references") to CompositeProvider methods
- [x] 2.2 Wire hover operation to `CompositeProvider::hover(&location)`
- [x] 2.3 Wire definition operation to `CompositeProvider::get_definition(&location)`
- [x] 2.4 Wire references operation to `CompositeProvider::find_references(&location, true)`
- [x] 2.5 Update `enable_proxy_mode_with_provider()` to initialize CompositeProvider with workspace_root

## Phase 3: Integration Tests

- [x] 3.1 Create `tests/integration/lsp_integration_test.rs` with `rust_analyzer_available()` helper
- [x] 3.2 Create `pyright_available()` helper function
- [x] 3.3 Add `#[ignore]` test for rust-analyzer hover (temp Rust project, verify function signature)
- [x] 3.4 Add `#[ignore]` test for rust-analyzer go-to-definition (verify correct line returned)
- [x] 3.5 Add `#[ignore]` test for pyright go-to-definition (temp Python project)
- [x] 3.6 Add `#[ignore]` test for pyright find-references (verify all usage locations)
- [x] 3.7 Implement timeout handling (10s) with process cleanup

## Phase 4: Verification

- [x] 4.1 Run `cargo build` to verify compilation
- [x] 4.2 Run `cargo test` to verify existing tests pass
- [x] 4.3 Verify integration tests are properly ignored when binaries unavailable
