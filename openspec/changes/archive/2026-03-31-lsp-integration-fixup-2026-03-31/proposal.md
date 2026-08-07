# Proposal: LSP Integration Fixup & End-to-End Testing

## Intent

Task 2.5 from `lsp-abstraction-layer-2026-03-31` was marked complete without actual implementation — `LspProxyService.route_operation()` still returns `None` instead of delegating to the new `CompositeProvider`. This change fixes that incomplete integration and validates the entire LSP stack with real language servers before archiving.

## Scope

### In Scope
- Wire `CompositeProvider` into `LspProxyService.route_operation()` (fix task 2.5)
- End-to-end testing with real rust-analyzer and pyright binaries
- Archive `lsp-abstraction-layer-2026-03-31` to `openspec/changes/archive/`

### Out of Scope
- New LSP features (completion, rename)
- Additional language servers (gopls, clangd)
- Performance optimizations

## Approach

1. **Fix Integration** — Add `CompositeProvider` field to `LspProxyService`, initialize in constructor, delegate `route_operation()` calls
2. **E2E Testing** — Create integration tests that spawn real LSP processes (rust-analyzer, pyright), send actual LSP requests, verify responses
3. **Archive** — Move completed change to archive with today's date

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/application/services/lsp_proxy_service.rs` | Modified | Add CompositeProvider, implement route_operation() delegation |
| `tests/integration/` | New | Real LSP integration tests |
| `openspec/changes/archive/` | New | Archived lsp-abstraction-layer change |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| LSP binaries unavailable in CI | Medium | Skip tests gracefully with `#[ignore]` if binary not found |
| Integration test flakiness (process timing) | Medium | Use generous timeouts, retry logic |
| CompositeProvider requires workspace_root | Low | Pass from service initialization context |

## Rollback Plan

Revert `lsp_proxy_service.rs` changes. Delete integration tests. Unarchive the previous change if needed. All changes are isolated.

## Dependencies

- `rust-analyzer` binary (install: `rustup component add rust-analyzer`)
- `pyright` binary (install: `npm install -g pyright`)
- Tests skip automatically if binaries unavailable

## Success Criteria

- [ ] `route_operation()` delegates to `CompositeProvider` for hover/definition/references
- [ ] Integration test with rust-analyzer returns real hover info
- [ ] Integration test with pyright returns real go-to-definition
- [ ] `cargo test` passes (unit + integration where binaries available)
- [ ] `lsp-abstraction-layer-2026-03-31` archived with merged specs
