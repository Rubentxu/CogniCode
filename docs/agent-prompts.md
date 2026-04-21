# AI Agent Prompt Examples for CogniCode MCP

This document contains ready-to-use prompt examples for AI agents integrating
with CogniCode via MCP. Prompts are organized by scenario — what the agent is
trying to accomplish — rather than by tool.

Each example includes:
- A **natural language prompt** the agent receives from the user
- The **reasoning chain** the agent should follow
- The **MCP tool calls** to make, in order
- A **sample interpretation** of the results

> **Prerequisites:** The MCP server must be running and `build_graph` called
> at least once before most analysis tools will return meaningful results.

---

## Table of Contents

1. [Onboarding a New Codebase](#1-onboarding-a-new-codebase)
2. [Understanding a Feature Before Changing It](#2-understanding-a-feature-before-changing-it)
3. [Safe Rename Refactoring](#3-safe-rename-refactoring)
4. [Architecture Health Review](#4-architecture-health-review)
5. [Finding Who Calls a Function](#5-finding-who-calls-a-function)
6. [Analyzing Change Impact Before a PR](#6-analyzing-change-impact-before-a-pr)
7. [Navigating to a Definition](#7-navigating-to-a-definition)
8. [Finding Dead Code](#8-finding-dead-code)
9. [Measuring Code Complexity](#9-measuring-code-complexity)
10. [Extracting a Function Safely](#10-extracting-a-function-safely)
11. [Moving a Symbol to Another File](#11-moving-a-symbol-to-another-file)
12. [Semantic Symbol Search](#12-semantic-symbol-search)
13. [Generating a Call Graph Diagram](#13-generating-a-call-graph-diagram)
14. [Understanding a File at a Glance](#14-understanding-a-file-at-a-glance)
15. [Validating Syntax Before Committing](#15-validating-syntax-before-committing)
16. [Finding the Most Critical Functions](#16-finding-the-most-critical-functions)
17. [Tracing an Execution Path](#17-tracing-an-execution-path)
18. [Searching for a Code Pattern](#18-searching-for-a-code-pattern)
19. [Reviewing a Symbol's Full Implementation](#19-reviewing-a-symbols-full-implementation)
20. [Incremental Re-analysis After Changes](#20-incremental-re-analysis-after-changes)

---

## 1. Onboarding a New Codebase

**User prompt:**
> "I just cloned this repo. Help me understand what it does, what the main
> entry points are, and which functions are called the most."

**Agent reasoning:**
Build the full graph first, then get entry points (public API surface), leaf
functions (low-level primitives), and hot paths (most interconnected code).
Together these three give a 360° view of any unfamiliar codebase.

**Tool calls:**

```
1. build_graph
   directory: "/path/to/project"
   strategy: "full"
```

```
2. get_entry_points
   compressed: true
```

```
3. get_leaf_functions
   compressed: true
```

```
4. get_hot_paths
   limit: 10
   min_fan_in: 3
```

**How to interpret the results:**

- **Entry points** → public surface area; these are the functions users or
  other systems call into. Start reading here.
- **Leaf functions** → primitives; pure utility code with no further
  dependencies. Usually safe to ignore initially.
- **Hot paths** → functions called from many places. Changes here ripple
  widely. Pay special attention to these.

---

## 2. Understanding a Feature Before Changing It

**User prompt:**
> "I need to modify the `process_payment` function. Before I touch anything,
> show me everything that calls it and everything it calls."

**Agent reasoning:**
Get both the incoming call hierarchy (who depends on it) and outgoing
(what it depends on). Then fetch the actual source code of the function
to read it in context.

**Tool calls:**

```
1. get_call_hierarchy
   symbol_name: "process_payment"
   direction: "incoming"
   depth: 3
```

```
2. get_call_hierarchy
   symbol_name: "process_payment"
   direction: "outgoing"
   depth: 2
```

```
3. find_usages
   symbol_name: "process_payment"
   include_declaration: true
```

```
4. get_symbol_code
   file: "src/payments/processor.rs"
   line: 42
   col: 0
```

**How to interpret the results:**

- If `incoming` depth-1 shows 8+ callers, this is a high-risk function to
  change — consider the extract approach instead.
- If `outgoing` shows calls to I/O, network, or database functions, changing
  the signature will require updating mocks in tests.
- `find_usages` reveals call sites in tests — these show you the expected
  contract.

---

## 3. Safe Rename Refactoring

**User prompt:**
> "Rename `calc_total` to `calculate_order_total` everywhere in the project.
> Make sure nothing breaks."

**Agent reasoning:**
First analyze impact to understand scope, then preview the refactoring
before applying it. Never rename blindly.

**Tool calls:**

```
1. analyze_impact
   symbol_name: "calc_total"
```

```
2. find_usages
   symbol_name: "calc_total"
   include_declaration: true
```

```
3. safe_refactor
   action: "rename"
   symbol_name: "calc_total"
   new_name: "calculate_order_total"
   file_path: "src/order/calculator.rs"
   line: 15
   column: 7
   preview: true
```

```
4. safe_refactor
   action: "rename"
   symbol_name: "calc_total"
   new_name: "calculate_order_total"
   file_path: "src/order/calculator.rs"
   line: 15
   column: 7
   preview: false
```

**How to interpret the results:**

- Step 1: If `risk_level` is `High` or `Critical` (more than 5–10 call sites),
  warn the user before proceeding.
- Step 3 (preview): Read the `affected_files` list and show it to the user
  for confirmation.
- Step 4: Only execute after user confirms the preview is correct.

---

## 4. Architecture Health Review

**User prompt:**
> "Is there any circular dependency in this codebase? Give me an architecture
> health score."

**Agent reasoning:**
Run `check_architecture` which uses the Tarjan SCC algorithm to detect cycles.
The score starts at 100 and drops 5 points per symbol involved in a cycle.

**Tool calls:**

```
1. build_graph
   directory: "/path/to/project"
   strategy: "full"
```

```
2. check_architecture
```

**How to interpret the results:**

| Score | Meaning |
|-------|---------|
| 100 | No cycles. Clean architecture. |
| 80–99 | Minor cycles, worth addressing. |
| 50–79 | Significant coupling issues. |
| < 50 | Architecture needs refactoring. |

- Each `cycle` in the result lists the symbols forming the loop.
- For each cycle: use `trace_path` between the two endpoints to understand
  why the dependency exists.
- Recommended fix: introduce a trait or a shared abstraction to break the cycle.

---

## 5. Finding Who Calls a Function

**User prompt:**
> "Who calls `authenticate_user`? I want to know all the callers and whether
> any of them are in the public API layer."

**Agent reasoning:**
Get incoming call hierarchy with enough depth to reach the top-level callers.
Then cross-reference with entry points to see which callers are on the
public surface.

**Tool calls:**

```
1. get_call_hierarchy
   symbol_name: "authenticate_user"
   direction: "incoming"
   depth: 5
   compressed: false
```

```
2. get_entry_points
   compressed: false
```

**How to interpret the results:**

- Intersect the callers from step 1 with the entry points from step 2.
- Any caller that is also an entry point means `authenticate_user` is
  reachable from the public API — changes must be backward compatible.
- If no callers are found at depth 1, the function may be dead code.

---

## 6. Analyzing Change Impact Before a PR

**User prompt:**
> "I'm about to change the signature of `UserRepository::find_by_email`.
> What's the blast radius? Which files will I need to update?"

**Agent reasoning:**
`analyze_impact` gives a risk-categorized list of all symbols that transitively
depend on the target. This is the single most important tool to run before
any non-trivial change.

**Tool calls:**

```
1. analyze_impact
   symbol_name: "UserRepository::find_by_email"
```

```
2. get_call_hierarchy
   symbol_name: "UserRepository::find_by_email"
   direction: "incoming"
   depth: 4
```

**How to interpret the results:**

- `risk_level` values: `Low` (≤2 symbols), `Medium` (≤5), `High` (≤10),
  `Critical` (>10).
- `impacted_files` in the result gives you the exact list of files to update.
- If `risk_level` is `Critical`, suggest to the user to introduce a
  compatibility shim rather than a direct signature change.

---

## 7. Navigating to a Definition

**User prompt:**
> "Where is `PaymentGateway` defined? Take me to the source."

**Agent reasoning:**
Use the LSP-backed `go_to_definition`. If the user gives a location in a file,
use that. Otherwise use `query_symbol_index` to find a call site first.

**Tool calls:**

```
1. query_symbol_index
   symbol_name: "PaymentGateway"
   directory: "/path/to/project"
```

```
2. go_to_definition
   file_path: "src/payments/gateway.rs"
   line: 8
   column: 4
```

**How to interpret the results:**

- Step 1 gives you a file and line where `PaymentGateway` appears.
- Step 2 jumps to its actual definition (which may be in a different file
  if it's from an imported crate).
- If `go_to_definition` returns nothing, the symbol is from an external
  dependency — suggest using `cargo doc --open` instead.

---

## 8. Finding Dead Code

**User prompt:**
> "Are there any functions in this project that nobody calls? I want to clean
> up unused code."

**Agent reasoning:**
Leaf functions with zero callers are candidates for dead code. Cross-reference
with entry points (they have no callers by design but are not dead).

**Tool calls:**

```
1. build_graph
   directory: "/path/to/project"
   strategy: "full"
```

```
2. get_leaf_functions
   compressed: false
```

```
3. get_entry_points
   compressed: false
```

**Agent post-processing logic:**

```
dead_code_candidates = leaf_functions - entry_points

For each candidate:
  - If it's a test function (#[test], test_*) → skip
  - If it's a trait implementation → verify via get_call_hierarchy incoming
  - If get_call_hierarchy incoming returns empty → confirmed dead code
```

```
4. get_call_hierarchy
   symbol_name: "<candidate_function>"
   direction: "incoming"
   depth: 1
```

**How to interpret the results:**

- Functions with 0 incoming callers that are not entry points, not tests,
  and not trait implementations are safe to remove.
- Always confirm with the user before deleting — some functions may be
  called via reflection, FFI, or dynamic dispatch.

---

## 9. Measuring Code Complexity

**User prompt:**
> "Which functions in `src/billing/invoicer.rs` are the most complex?
> I want to prioritize what to refactor."

**Agent reasoning:**
Get complexity metrics for the whole file, then sort by cyclomatic complexity.
Functions above 10 are high priority for refactoring.

**Tool calls:**

```
1. get_complexity
   file_path: "src/billing/invoicer.rs"
```

**How to interpret the results:**

| Cyclomatic complexity | Risk |
|----------------------|------|
| 1–5 | Low. Simple and easy to test. |
| 6–10 | Moderate. Consider simplifying. |
| 11–20 | High. Hard to test fully. Refactor. |
| > 20 | Critical. Must be broken down. |

- Sort `functions` by `cyclomatic` descending.
- For each function above threshold, call `get_symbol_code` to read it and
  suggest extract refactoring opportunities.

```
2. get_symbol_code
   file: "src/billing/invoicer.rs"
   line: <line_of_complex_function>
   col: 0
```

---

## 10. Extracting a Function Safely

**User prompt:**
> "The `generate_invoice` function is 200 lines long. Extract the tax
> calculation part into a separate function called `calculate_tax`."

**Agent reasoning:**
Read the function first to identify the extraction range. Preview the
extraction, then apply it.

**Tool calls:**

```
1. get_symbol_code
   file: "src/billing/invoicer.rs"
   line: 88
   col: 0
```

```
2. get_complexity
   file_path: "src/billing/invoicer.rs"
   function_name: "generate_invoice"
```

```
3. safe_refactor
   action: "extract"
   symbol_name: "generate_invoice"
   new_name: "calculate_tax"
   file_path: "src/billing/invoicer.rs"
   line: 88
   column: 0
   preview: true
```

```
4. safe_refactor
   action: "extract"
   symbol_name: "generate_invoice"
   new_name: "calculate_tax"
   file_path: "src/billing/invoicer.rs"
   line: 88
   column: 0
   preview: false
```

**How to interpret the results:**

- The preview in step 3 shows the new function signature and where the
  call site will be inserted.
- After applying, run `validate_syntax` on the file to confirm the result
  is syntactically valid.

```
5. validate_syntax
   file_path: "src/billing/invoicer.rs"
```

---

## 11. Moving a Symbol to Another File

**User prompt:**
> "Move the `EmailFormatter` struct from `utils.rs` to a new dedicated
> `email.rs` file."

**Agent reasoning:**
Check what depends on `EmailFormatter` first. Then preview the move to
see all the `use` imports that will be updated.

**Tool calls:**

```
1. analyze_impact
   symbol_name: "EmailFormatter"
```

```
2. safe_refactor
   action: "move"
   symbol_name: "EmailFormatter"
   source_path: "src/utils.rs"
   target_path: "src/email.rs"
   preview: true
```

```
3. safe_refactor
   action: "move"
   symbol_name: "EmailFormatter"
   source_path: "src/utils.rs"
   target_path: "src/email.rs"
   preview: false
```

**How to interpret the results:**

- The preview lists every file where `use crate::utils::EmailFormatter` must
  change to `use crate::email::EmailFormatter`.
- If the target file doesn't exist, the tool will create it.
- After the move, confirm with `find_usages` that all references resolve.

---

## 12. Semantic Symbol Search

**User prompt:**
> "Find all repository-related structs and traits in this project."

**Agent reasoning:**
Use `semantic_search` with a fuzzy query and filter by kind. Try multiple
queries if the first doesn't cover all results.

**Tool calls:**

```
1. semantic_search
   query: "repository"
   kinds: ["struct", "trait"]
   max_results: 30
```

```
2. semantic_search
   query: "repo"
   kinds: ["struct", "trait", "impl"]
   max_results: 20
```

**How to interpret the results:**

- Results include file path, line, and kind for each match.
- Deduplicate by symbol name across both queries.
- For each trait found, call `get_call_hierarchy outgoing` to see what
  concrete operations it defines.

---

## 13. Generating a Call Graph Diagram

**User prompt:**
> "Give me a visual diagram of everything that calls `OrderService`."

**Agent reasoning:**
Build a focused subgraph around `OrderService`, then export it as Mermaid.
The subgraph is more readable than the full project graph.

**Tool calls:**

```
1. build_call_subgraph
   symbol_name: "OrderService"
   direction: "both"
   depth: 2
   directory: "/path/to/project"
```

```
2. export_mermaid
   root_symbol: "OrderService"
   max_depth: 2
   format: "code"
   include_external: false
```

**How to interpret the results:**

- Paste the Mermaid code into any Mermaid renderer (GitHub markdown,
  mermaid.live, VS Code extension) to visualize.
- Use `format: "svg"` to get a rendered image directly.
- If the diagram is too large (>30 nodes), reduce `max_depth` to 1.

---

## 14. Understanding a File at a Glance

**User prompt:**
> "Give me a quick summary of what `src/api/handlers.rs` contains."

**Agent reasoning:**
Use `get_outline` for the structure, then `get_file_symbols` with
`compressed: true` for a natural language summary. Both together give a
complete picture without reading the raw source.

**Tool calls:**

```
1. get_outline
   file_path: "src/api/handlers.rs"
   include_private: false
   include_tests: false
```

```
2. get_file_symbols
   file_path: "src/api/handlers.rs"
   compressed: true
```

**How to interpret the results:**

- `get_outline` shows the symbol tree — which functions are inside which
  structs or modules.
- `get_file_symbols` compressed returns a prose description suitable for
  presenting to the user without overwhelming them with JSON.

---

## 15. Validating Syntax Before Committing

**User prompt:**
> "I just edited three files. Make sure they all parse correctly before I
> commit."

**Agent reasoning:**
Run `validate_syntax` on each modified file. Cheaper than a full compile
and catches structural errors immediately.

**Tool calls:**

```
1. validate_syntax
   file_path: "src/domain/order.rs"
```

```
2. validate_syntax
   file_path: "src/application/order_service.rs"
```

```
3. validate_syntax
   file_path: "src/interface/api/order_handler.rs"
```

**How to interpret the results:**

- `valid: true` → file parses correctly.
- `valid: false` → `errors` array contains the line, column, and description
  of each syntax error.
- Present errors to the user with the exact location before they commit.

---

## 16. Finding the Most Critical Functions

**User prompt:**
> "Which functions should I never break? I want to know the most depended-upon
> code in the project."

**Agent reasoning:**
Hot paths by fan-in are the functions with the most callers — breaking them
has the widest blast radius. Combine with impact analysis on the top ones.

**Tool calls:**

```
1. get_hot_paths
   limit: 15
   min_fan_in: 2
```

```
2. analyze_impact
   symbol_name: "<top_function_from_step_1>"
```

**How to interpret the results:**

- Sort results by `fan_in` descending.
- The top 3–5 functions are your most critical — add them to a list of
  "never-break" symbols.
- For each top function, verify it has test coverage by searching for its
  name with `find_usages` and checking for `test_` prefixed callers.

---

## 17. Tracing an Execution Path

**User prompt:**
> "How does the code get from `main` to `send_notification`? Show me the
> execution path."

**Agent reasoning:**
`trace_path` uses BFS to find the shortest call chain between any two symbols.
This is invaluable for understanding how a user action triggers deep behavior.

**Tool calls:**

```
1. trace_path
   source: "main"
   target: "send_notification"
   max_depth: 15
```

**How to interpret the results:**

- The result is an ordered list of function names forming the call chain.
- If no path is found, either the functions are in different disconnected
  components, or `send_notification` is called indirectly (via trait object,
  callback, or async task).
- In that case, try with `find_usages` to find who calls `send_notification`
  directly, then trace from `main` to that intermediate caller.

---

## 18. Searching for a Code Pattern

**User prompt:**
> "Find all places in the codebase where we call `unwrap()` on a Result.
> I want to audit error handling."

**Agent reasoning:**
`structural_search` does AST-aware pattern matching, which is more accurate
than text search for code constructs. Fall back to `search_content` for
simpler text patterns.

**Tool calls:**

```
1. search_content
   pattern: "\\.unwrap()"
   path: "/path/to/project"
   case_sensitive: true
```

```
2. structural_search
   pattern: "?.unwrap()"
   directory: "/path/to/project"
```

**How to interpret the results:**

- Each result includes file path, line, and the matching line content.
- Group results by file.
- Flag any `unwrap()` that is not inside a `#[test]` function or not on a
  value that is provably `Some`/`Ok` — these are potential panics in
  production.

---

## 19. Reviewing a Symbol's Full Implementation

**User prompt:**
> "Show me the complete implementation of `TokenValidator::validate`, including
> its documentation."

**Agent reasoning:**
Use `query_symbol_index` to locate the symbol, then `get_symbol_code` to
retrieve its full source with docstrings.

**Tool calls:**

```
1. query_symbol_index
   symbol_name: "TokenValidator"
   directory: "/path/to/project"
```

```
2. get_symbol_code
   file: "src/auth/token_validator.rs"
   line: 34
   col: 4
```

```
3. hover
   file_path: "src/auth/token_validator.rs"
   line: 34
   column: 4
```

**How to interpret the results:**

- `get_symbol_code` returns the raw source between the function's start and
  end braces, including any `///` doc comments above it.
- `hover` (LSP) returns the type signature and rendered documentation as the
  language server sees it.
- Together they give the full picture: implementation + documentation.

---

## 20. Incremental Re-analysis After Changes

**User prompt:**
> "I just modified several files. Update the analysis without rebuilding
> everything from scratch."

**Agent reasoning:**
Use `build_graph` with `strategy: "lightweight"` for a fast re-index that
updates the symbol table without the cost of full edge computation. Then
trigger a full rebuild only if the user needs impact or hot-path analysis.

**Tool calls:**

```
1. build_lightweight_index
   directory: "/path/to/project"
   strategy: "lightweight"
```

```
2. query_symbol_index
   symbol_name: "<recently_modified_symbol>"
   directory: "/path/to/project"
```

**If deeper analysis is needed after the lightweight pass:**

```
3. build_graph
   directory: "/path/to/project"
   strategy: "full"
```

**How to interpret the results:**

- The lightweight index is always fast (seconds vs. tens of seconds for full).
- Use it for quick "does this symbol exist / where is it now" questions after
  file edits.
- Only trigger `strategy: "full"` when the user explicitly asks for impact
  analysis, hot paths, architecture check, or call graph traversal.

---

## Combining Tools: Full Refactoring Workflow

The following is a complete example of how an agent should chain tools
for a non-trivial refactoring task.

**User prompt:**
> "I want to refactor `UserService` — it's doing too much. Help me split
> it safely."

**Recommended agent workflow:**

```
Step 1 — Understand the current state
  → get_file_symbols("src/services/user_service.rs", compressed=false)
  → get_outline("src/services/user_service.rs")
  → get_complexity("src/services/user_service.rs")

Step 2 — Understand dependencies
  → analyze_impact("UserService")
  → get_call_hierarchy("UserService", direction="incoming", depth=3)
  → get_call_hierarchy("UserService", direction="outgoing", depth=2)

Step 3 — Identify extraction candidates
  Sort methods by cyclomatic complexity.
  Group methods by domain concern (auth vs. profile vs. notifications).

Step 4 — Plan the split
  Present the user with a proposed split:
    UserService → AuthService + UserProfileService + NotificationService

Step 5 — Execute extractions (one at a time, with validation)
  For each extracted method:
    → safe_refactor(action="extract", preview=true)
    → [user confirms]
    → safe_refactor(action="extract", preview=false)
    → validate_syntax(file_path)

Step 6 — Move to new files
  → safe_refactor(action="move", preview=true) for each new service
  → [user confirms]
  → safe_refactor(action="move", preview=false)

Step 7 — Verify final state
  → check_architecture()
  → find_usages("UserService")  ← should now show fewer direct usages
  → get_hot_paths()             ← confirm new services have reasonable fan-in
```

---

## Tips for AI Agents

### Always build the graph first

Most tools require a built graph. At the start of any session, call:

```
build_graph(directory=".", strategy="full")
```

If time is a constraint, use `strategy: "lightweight"` for symbol-only
operations and upgrade to `"full"` only when needed.

### Use `compressed: true` to save context

When exploring large codebases, prefer compressed output:

```
get_file_symbols(file_path="...", compressed=true)
get_entry_points(compressed=true)
get_leaf_functions(compressed=true)
```

This returns prose summaries instead of full JSON, preserving context window.

### Sequence for safe changes

1. `analyze_impact` → understand the scope
2. `safe_refactor preview=true` → show the user what will change
3. User confirmation
4. `safe_refactor preview=false` → apply
5. `validate_syntax` → confirm the result is valid

### Risk thresholds

| Risk level | Recommended action |
|------------|--------------------|
| `Low` | Apply directly after preview |
| `Medium` | Show impact list, ask for confirmation |
| `High` | Warn the user, suggest smaller increments |
| `Critical` | Recommend a compatibility shim instead |

### Use subgraphs for readability

For large projects, `build_call_subgraph` centered on a symbol of interest
gives a focused view instead of the overwhelming full graph:

```
build_call_subgraph(symbol_name="target", direction="both", depth=2)
```
