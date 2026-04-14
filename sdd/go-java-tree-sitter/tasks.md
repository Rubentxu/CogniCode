# Tasks: go-java-tree-sitter

## Phase 1: Cargo Dependencies

- [ ] 1.1 Add `tree-sitter-go = "0.23"` to Cargo.toml
- [ ] 1.2 Add `tree-sitter-java = "0.23"` to Cargo.toml
- [ ] 1.3 Run `cargo build` to verify dependencies resolve

## Phase 2: Language Enum Extension

- [ ] 2.1 Add Go variant to Language enum
- [ ] 2.2 Add Java variant to Language enum
- [ ] 2.3 Implement `to_ts_language()` for both variants
- [ ] 2.4 Implement `name()` for both (Go="Go", Java="Java")
- [ ] 2.5 Implement `function_node_type()` for both (Go="function_declaration", Java="method_declaration")
- [ ] 2.6 Implement `class_node_type()` for both (Go="type_declaration", Java="class_declaration")
- [ ] 2.7 Implement `variable_node_type()` for both (Go/Java="variable_declarator")
- [ ] 2.8 Implement `call_node_type()` for both (Go="call_expression", Java="method_invocation")
- [ ] 2.9 Implement `call_has_function_field()` for both (Go=true, Java=false)
- [ ] 2.10 Implement `lsp_*` methods for both (return None/placeholder)

## Phase 3: file_operations.rs Updates

- [ ] 3.1 Update match block for outline extraction
- [ ] 3.2 Update match block for symbol extraction
- [ ] 3.3 Update match block for compressed symbols
- [ ] 3.4 Update match block for edit operations

## Phase 4: find_identifier_name Fix

- [ ] 4.1 Add field_identifier handling in per_file_graph.rs

## Phase 5: Verification

- [ ] 5.1 Build project
- [ ] 5.2 Run tests
- [ ] 5.3 Verify Go/Java parsing works