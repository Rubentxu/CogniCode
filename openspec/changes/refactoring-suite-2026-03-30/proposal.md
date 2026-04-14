# Proposal: IntelliJ-Style Refactoring Suite for CogniCode

## Intent

Implement the four missing IntelliJ-style refactoring features (Extract Method, Inline Method, Move Symbol, Change Signature) to enable AI agents to perform safe, automated code transformations. Currently only "Rename" refactoring is implemented. This suite is critical for AI-driven code improvement workflows.

## Scope

### In Scope
1. **Extract Method/Function** - Extract selected code into new function
2. **Inline Method/Function** - Replace calls with method body, remove original
3. **Move Symbol** - Move function/class to different file with import updates
4. **Change Signature** - Rename/reorder/add/remove parameters

### Out of Scope
- Extract Interface (future)
- Pull Up / Push Down members (future)
- Cross-project refactoring
- Refactorings across git history

## Approach

Extend existing DDD architecture with strategy pattern. Each refactoring type gets its own strategy implementation following the `RenameStrategy` pattern.

```
src/infrastructure/refactor/
├── mod.rs                    # Export all strategies
├── rename_strategy.rs        # EXISTING
├── extract_strategy.rs       # NEW
├── inline_strategy.rs        # NEW
├── move_strategy.rs          # NEW
└── change_signature_strategy.rs  # NEW
```

---

## Feature 1: Extract Method/Function

### Description
Extracts a selection of code statements into a new function, replacing the original code with a call to the new function.

**Why for AI Agents:** Enables automatic code decomposition, reducing function complexity without manual intervention.

**IntelliJ Behavior:**
- Analyzes selected code for local variables used
- Creates parameters for variables read but not defined in selection
- Returns values for variables modified and used after selection
- Generates appropriate function signature

### Use Case Example

**Before:**
```rust
fn process_order(order_id: u32, items: Vec<Item>, customer_id: u32) -> Order {
    // === SELECTION START ===
    let mut total = 0.0;
    for item in &items {
        total += item.price * item.quantity as f64;
    }
    let tax = total * 0.1;
    total += tax;
    // === SELECTION END ===
    
    Order { order_id, total, customer_id }
}
```

**After:**
```rust
fn process_order(order_id: u32, items: Vec<Item>, customer_id: u32) -> Order {
    let total = calculate_total_with_tax(&items);
    Order { order_id, total, customer_id }
}

fn calculate_total_with_tax(items: &Vec<Item>) -> f64 {
    let mut total = 0.0;
    for item in items {
        total += item.price * item.quantity as f64;
    }
    let tax = total * 0.1;
    total += tax;
    total
}
```

**AI Agent Call:**
```json
{
  "action": "extract",
  "target": "process_order",
  "params": {
    "file_path": "/src/order.rs",
    "start_line": 2,
    "end_line": 8,
    "new_function_name": "calculate_total_with_tax"
  }
}
```

### Technical Implementation

**Files to Modify:**
| File | Change |
|------|--------|
| `src/domain/aggregates/refactor.rs` | Add extraction params to `RefactorParameters` |
| `src/infrastructure/refactor/extract_strategy.rs` | NEW - ExtractStrategy implementation |
| `src/application/services/refactor_service.rs` | Add `extract_method()` method |
| `src/interface/mcp/handlers.rs` | Handle `RefactorAction::Extract` |
| `src/interface/mcp/schemas.rs` | Add ExtractInput schema |

**Algorithm:**
1. Parse selection to identify:
   - Variables defined in selection (local)
   - Variables read from outer scope (become parameters)
   - Variables modified and used after (become return values)
2. Generate function signature with inferred types
3. Create TextEdits:
   - Insert new function before/after containing function
   - Replace selection with function call + return assignment

**Tree-sitter Queries:**
```scheme
; Find local variable declarations
(variable_declarator name: (identifier) @local_var)

; Find identifier usages
(identifier) @usage

; Find function containing selection
(function_item name: (identifier) @fn_name) @fn_node
```

### Safety Considerations

| Risk | Mitigation |
|------|------------|
| Incorrect variable capture | Validate all identifiers resolve correctly |
| Type inference failure | Require explicit type annotation or reject |
| Scope escape | Verify no references to locals outside selection |

---

## Feature 2: Inline Method/Function

### Description
Replaces all calls to a function with the function body, then removes the original function definition.

**Why for AI Agents:** Removes unnecessary abstraction layers automatically, simplifying code when functions become trivial.

**IntelliJ Behavior:**
- Finds all call sites
- Inlines body at each call site
- Handles parameter substitution
- Removes original function if no remaining calls

### Use Case Example

**Before:**
```rust
fn is_valid_email(email: &str) -> bool {
    email.contains('@') && email.contains('.')
}

fn register_user(email: String) -> Result<User, Error> {
    if !is_valid_email(&email) {
        return Err(Error::InvalidEmail);
    }
    // ...
}
```

**After:**
```rust
fn register_user(email: String) -> Result<User, Error> {
    if !(email.contains('@') && email.contains('.')) {
        return Err(Error::InvalidEmail);
    }
    // ...
}
// is_valid_email removed
```

**AI Agent Call:**
```json
{
  "action": "inline",
  "target": "is_valid_email",
  "params": {
    "file_path": "/src/user.rs",
    "delete_original": true
  }
}
```

### Technical Implementation

**Files to Modify:**
| File | Change |
|------|--------|
| `src/infrastructure/refactor/inline_strategy.rs` | NEW - InlineStrategy implementation |
| `src/application/services/refactor_service.rs` | Add `inline_method()` method |
| `src/interface/mcp/handlers.rs` | Handle `RefactorAction::Inline` |

**Algorithm:**
1. Parse function definition to extract body and parameters
2. Find all call sites across project
3. For each call site:
   - Map arguments to parameters
   - Substitute parameter references with argument expressions
   - Handle return values appropriately
4. Delete original function definition

**Tree-sitter Queries:**
```scheme
; Find function definition
(function_item name: (identifier) @name parameters: (parameters) @params body: (block) @body)

; Find call sites
(call_expression function: (identifier) @callee (#eq? @callee target_name))
```

### Safety Considerations

| Risk | Mitigation |
|------|------------|
| Side effects in parameters | Warn if arguments have side effects |
| Multiple returns in body | Reject or transform to expression |
| Recursive calls | Detect and reject inlining |
| Used in other modules | Find all usages across project |

---

## Feature 3: Move Symbol

### Description
Moves a function, struct, or class to a different file, updating all imports and qualified references.

**Why for AI Agents:** Enables automatic code organization and module restructuring.

**IntelliJ Behavior:**
- Moves symbol definition to target file
- Adds import to target file
- Updates/creates imports in source file if needed
- Updates qualified references (e.g., `module::symbol`)

### Use Case Example

**Before (src/utils.rs):**
```rust
pub fn format_currency(amount: f64) -> String {
    format!("${:.2}", amount)
}
```

**After (src/finance/currency.rs):**
```rust
// NEW FILE
pub fn format_currency(amount: f64) -> String {
    format!("${:.2}", amount)
}
```

**Updated imports in src/order.rs:**
```rust
// Before:
use crate::utils::format_currency;

// After:
use crate::finance::currency::format_currency;
```

**AI Agent Call:**
```json
{
  "action": "move",
  "target": "format_currency",
  "params": {
    "source_file": "/src/utils.rs",
    "target_file": "/src/finance/currency.rs",
    "visibility": "pub"
  }
}
```

### Technical Implementation

**Files to Modify:**
| File | Change |
|------|--------|
| `src/infrastructure/refactor/move_strategy.rs` | NEW - MoveStrategy implementation |
| `src/application/services/refactor_service.rs` | Add `move_symbol()` method |
| `src/interface/mcp/handlers.rs` | Handle `RefactorAction::Move` |

**Algorithm:**
1. Find symbol definition and all its usages
2. Create target file if needed
3. Generate TextEdits:
   - Delete from source file
   - Insert into target file
   - Add/update imports in files using the symbol
4. Handle visibility modifiers

**Tree-sitter Queries:**
```scheme
; Find use statements
(use_declaration argument: (scoped_identifier) @import_path)

; Find qualified usages
(scoped_identifier path: (identifier) @module name: (identifier) @symbol)
```

### Safety Considerations

| Risk | Mitigation |
|------|------------|
| Circular imports | Detect cycles before moving |
| Name conflicts in target | Check for existing symbols with same name |
| Private symbol used externally | Validate visibility requirements |
| Target directory missing | Create directories or reject |

---

## Feature 4: Change Signature

### Description
Modifies a function's signature by renaming, reordering, adding, or removing parameters. Updates all call sites.

**Why for AI Agents:** Enables safe API evolution and parameter name improvements.

**IntelliJ Behavior:**
- Renames parameter at definition and updates usages
- Reorders arguments at call sites
- Adds new parameters with default values or marks call sites for review
- Removes parameters and updates call sites

### Use Case Example

**Before:**
```rust
fn create_user(name: String, email: String) -> User {
    User { name, email }
}

fn main() {
    let user = create_user("Alice".to_string(), "alice@example.com".to_string());
}
```

**After (rename + add parameter):**
```rust
fn create_user(full_name: String, email: String, age: u32) -> User {
    User { name: full_name, email, age }
}

fn main() {
    let user = create_user("Alice".to_string(), "alice@example.com".to_string(), 30);
}
```

**AI Agent Call:**
```json
{
  "action": "change_signature",
  "target": "create_user",
  "params": {
    "file_path": "/src/user.rs",
    "parameters": [
      {"old_name": "name", "new_name": "full_name", "type": "String"},
      {"name": "email", "type": "String"},
      {"name": "age", "type": "u32", "default_value": "0"}
    ]
  }
}
```

### Technical Implementation

**Files to Modify:**
| File | Change |
|------|--------|
| `src/domain/aggregates/refactor.rs` | Extend `RefactorParameters.new_signature` |
| `src/infrastructure/refactor/change_signature_strategy.rs` | NEW - ChangeSignatureStrategy |
| `src/application/services/refactor_service.rs` | Add `change_signature()` method |
| `src/interface/mcp/handlers.rs` | Handle `RefactorAction::ChangeSignature` |

**Algorithm:**
1. Parse current signature and new signature spec
2. Build parameter mapping (old → new position)
3. Find all call sites
4. Generate TextEdits:
   - Update function signature
   - Reorder/rename arguments at each call site
   - Add new arguments with defaults
   - Flag removals for review

**Tree-sitter Queries:**
```scheme
; Find function parameters
(parameters (parameter pattern: (identifier) @param_name type: (type_identifier) @param_type))

; Find call arguments
(arguments (argument) @arg)
```

### Safety Considerations

| Risk | Mitigation |
|------|------------|
| Breaking public API | Flag as breaking change, require confirmation |
| Type mismatches | Validate argument types match new signature |
| Missing default values | Require defaults for new required params |
| Call sites in other crates | Warn about external usages |

---

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/infrastructure/refactor/` | New | 4 new strategy files |
| `src/application/services/refactor_service.rs` | Modified | Add 4 new methods |
| `src/interface/mcp/handlers.rs` | Modified | Handle 4 new actions |
| `src/interface/mcp/schemas.rs` | Modified | Add input schemas |
| `src/domain/aggregates/refactor.rs` | Modified | Extend parameters |
| `tests/` | New | Integration tests for each refactoring |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Type inference errors | Medium | Conservative inference, require explicit types when ambiguous |
| Cross-file reference tracking | Medium | Leverage existing call graph infrastructure |
| Breaking changes slip through | Low | Multi-layer validation (syntax, semantic, graph-based) |
| Performance on large codebases | Low | Incremental parsing, lazy graph building |

## Rollback Plan

1. All refactoring operations are **preview-first** - user sees changes before applying
2. Changes applied to VirtualFileSystem first for validation
3. Git integration: suggest commit before applying large refactors
4. Each strategy has `undo_edits()` capability (inverse operations)
5. Failed operations leave codebase unchanged (atomic transactions)

## Dependencies

- Existing: tree-sitter, petgraph, safety module
- No new external dependencies required

## Success Criteria

- [ ] Extract Method works on Rust, Python, JavaScript, TypeScript
- [ ] Inline Method handles all call sites and removes original
- [ ] Move Symbol updates all imports correctly
- [ ] Change Signature updates all call sites with proper argument handling
- [ ] All refactorings validate syntax before applying
- [ ] All refactorings integrate with existing SafetyGate
- [ ] 80%+ test coverage for each strategy
- [ ] MCP tools return actionable error messages for failures
