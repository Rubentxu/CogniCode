# CogniCode: Why I Built an IDE for AI Agents (and Why You Should Care)

*AI agents read code like humans — line by line, sequentially, blindly. CogniCode gives them structural intelligence: the call graph, the symbol index, the architecture analysis. This is the story of why I built it and how it changes everything about how AI agents work with codebases.*

---

## The Problem: AI Agents Are Flying Blind

Let me paint you a picture. It's 2 AM. You're debugging a production incident at a company you've worked at for three months. The on-call alert is screaming about a failed payment processing. You've narrowed it down to something in the authentication layer, and you turn to your AI coding assistant for help.

You ask: "What functions call `validate_session_token`? I need to understand the blast radius before I touch anything."

Here's what happens next in a world without CogniCode:

Your AI assistant starts reading files. One by one. It reads `auth/mod.rs` (312 lines). Then `auth/jwt.rs` (489 lines). Then `auth/session.rs` (267 lines). Then it decides to check `middleware/auth_check.rs` (156 lines). Then `services/user_service.rs` (423 lines). Then `api/routes/admin.rs` (198 lines).

Twelve files later — after 40 seconds of sequential reading, burning through roughly 18,000 tokens of context — your AI tells you that changing `validate_session_token` "might affect some authentication-related components, particularly around token validation in the auth module."

Three of those "affected" files? They don't even exist in the codebase. Hallucinated dependencies. Meanwhile, the AI completely missed the actual caller in `api/routes/webhook.rs` that would silently break if you changed the function signature.

This isn't the AI being stupid. This is the AI working exactly as designed: reading code the way a human reads a novel. Sequentially. Without any structural understanding of what calls what.

### The Token Economics Problem

Here's the deeper issue that nobody talks about: **token economics**.

Every file your AI reads costs money. Not just in API credits — though that's real too — but in context window space. A 50,000 line Rust project might have 2,000 functions across 150 files. If your AI tries to "understand" the project by reading files, it burns through its context window reading files it doesn't need while still missing the critical connections.

A senior developer would never do this. They'd right-click on `validate_session_token` and select "Find Usages." In Visual Studio Code with the Rust Analyzer extension, this takes 200 milliseconds and returns an exact list of every call site. No guessing. No hallucinating. Just facts.

The senior developer isn't reading more code — they're reading it *structurally*. They have the IDE as a force multiplier that gives them instant access to the call graph.

So here's my question: **Why are we letting our AI agents stumble through codebases like it's 1995, when we solved this problem for humans thirty years ago?**

### Why LLMs Struggle With Code

Large language models process text sequentially. They're magnificent at generating fluent text, understanding natural language, and even writing plausible-looking code. But when it comes to understanding the *structure* of a codebase — which functions call which, what's the entry point, where are the circular dependencies — sequential text processing is fundamentally the wrong tool for the job.

Consider what happens when you ask an LLM: "Is there any circular dependency in this codebase?"

Without structural tools, the LLM has to:
1. Read every file in the codebase
2. Try to remember which functions call which
3. Mentally construct a graph of dependencies
4. Look for cycles in that mental graph

This is asking the LLM to do what computers do exceptionally well (graph algorithms) using what LLMs do exceptionally poorly (remembering precise structural relationships across thousands of files).

The result is predictable: the LLM either tells you "I don't see any obvious circular dependencies" (which might be wrong), or it finds a few and presents them with low confidence, or it invents dependencies that don't exist.

This is the problem CogniCode solves.

---

## The Vision: What If Agents Had IDE Superpowers?

Let me take you back to 1995. You're a C++ developer. You want to understand what `processOrder` does and where it's called. What do you do?

You use "Find in Files." You search for `processOrder`. You get a list of matches. You open each file. You read the code. You try to keep it all in your head.

Then, around 2000, something magical happened. IDEs started building symbol indexes. They started tracking which function calls which. "Find Usages" became a thing. "Go to Definition" became a thing. The IDE maintained a running model of your codebase's structure, and you could query it instantly.

The productivity gain was not about reading more code. It was about reading code *structurally*. Instead of searching blindly, you could ask precise questions and get precise answers.

**CogniCode brings this same paradigm shift to AI agents.**

The key insight is this: we now have the MCP (Model Context Protocol) which lets AI agents call external tools. This means we can give AI agents the same structural intelligence that IDEs give human developers. Not by making the LLM smarter (it's already as smart as it's going to get for structural reasoning), but by giving it tools that do structural reasoning *for* it.

The paradigm shift is from:

```
"Read everything and guess" 
```

to:

```
"Query the graph and know"
```

Instead of asking an AI to read 50 files and hope it understands the call graph, we give it a tool that has *already* built the call graph and can answer questions about it instantly.

This is not about making AI smarter. It's about giving AI the right tool for the job.

---

## What Is CogniCode?

CogniCode is a Rust-based code intelligence server that speaks the MCP (Model Context Protocol). In plain English: it's a server that AI agents can call to get structural information about codebases.

Here's what it offers:

**32 MCP tools** for code intelligence — everything from building call graphs to analyzing change impact to checking for circular dependencies.

**6 languages supported**: Rust, Python, TypeScript, JavaScript, Go, and Java. These are parsed using Tree-sitter, which produces accurate ASTs (Abstract Syntax Trees) rather than relying on fragile regex matching.

**4 graph strategies**: You can build a full call graph (comprehensive but slower), a lightweight index (fast symbol-only lookups), an on-demand graph (build as needed), or a per-file graph (for single-file analysis).

**Persistent cache**: Graphs are stored in an embedded redb database (a Rust embedded key-value store). Build once, query forever. The cache lives inside your project at `.cognicode/graph.redb`.

**Architecture analysis**: Uses the Tarjan algorithm for Strongly Connected Components (SCC) detection to find circular dependencies. Provides an architecture health score that starts at 100 and drops 5 points per symbol involved in a cycle.

**Mermaid export**: Generate visual call graph diagrams as SVG or Mermaid code.

### Why Rust?

I built CogniCode in Rust for specific reasons that matter for this use case:

**Performance**: Building call graphs for large codebases requires parsing thousands of files. Rust's zero-cost abstractions and lack of garbage collection pauses mean consistent, fast performance.

**Memory safety**: Code intelligence tools parse untrusted code. Rust's ownership model prevents entire categories of memory bugs that would be catastrophic in a tool that processes arbitrary input.

**Embedding**: The redb embedded database means no separate server process. The tool runs as a single binary that your AI agent spawns as a subprocess.

### Why Tree-sitter?

Tree-sitter is a parser generator that produces ASTs incrementally. Here's why that matters:

**Accuracy over regex**: Regex-based code analysis (think "find all lines that look like `function calls`") is fragile. It misses cases where the pattern is split across lines, gets confused by strings that look like code, and can't understand the actual structure.

**Language-agnostic**: Tree-sitter has grammars for all supported languages. The same structural analysis works regardless of whether you're parsing Rust or Python.

**Incrementality**: When a file changes, Tree-sitter can update the AST for just that file rather than re-parsing the entire codebase.

---

## Real-World Examples: Before and After

This is the meat of the post. Let me show you four scenarios where CogniCode transforms how an AI agent works with code. Each example shows the full conversation: the user's question, the AI's reasoning, the tool calls it makes, and the response it gets back.

### Example 1: Onboarding to a New Codebase

**The Situation**

It's Monday morning. You've just joined a new company. They've given you access to a 200-file Rust project that's been in development for two years. Your task: understand the architecture well enough to add a new feature by Friday.

Your manager says: "Just ask the AI to help you understand the codebase."

You turn to your AI assistant and ask:

> "I just cloned this repo. Help me understand what it does, what the main entry points are, and which functions are called the most."

**What a Traditional AI Agent Does**

Without CogniCode, the AI starts reading files. It reads `main.rs`, then `lib.rs`, then `mod.rs` files across the project. It tries to piece together an understanding from what it reads. This takes 2-3 minutes and gives you a vague, possibly inaccurate picture of the architecture.

**What a CogniCode-Powered Agent Does**

The agent reasons: "I need a 360° view of this codebase. Entry points show the public API surface. Leaf functions show the low-level primitives. Hot paths show where changes ripple widest. Let me call these three tools in sequence, then synthesize."

Tool call 1 — Build the graph:
```json
{
  "tool": "build_graph",
  "arguments": {
    "strategy": "full"
  }
}
```
Response:
```json
{
  "status": "success",
  "nodes": 1247,
  "edges": 3891,
  "languages": ["rust"],
  "cache_hit": false,
  "build_time_ms": 1247
}
```

Tool call 2 — Get entry points:
```json
{
  "tool": "get_entry_points",
  "arguments": {
    "compressed": true
  }
}
```
Response:
```json
{
  "entry_points": [
    "handle_request (src/api/mod.rs:12)",
    "process_payment (src/payment/mod.rs:34)",
    "authenticate (src/auth/mod.rs:8)",
    "generate_report (src/reports/mod.rs:19)"
  ],
  "summary": "4 public entry points. HTTP handler delegates to service layer. Payment processing is a separate subsystem. Auth is a middleware component."
}
```

Tool call 3 — Get leaf functions:
```json
{
  "tool": "get_leaf_functions",
  "arguments": {
    "compressed": true
  }
}
```
Response:
```json
{
  "leaf_functions": [
    "hash_password (src/auth/crypto.rs:45) - pure utility, no outgoing calls",
    "format_currency (src/utils/money.rs:12) - pure formatting, no outgoing calls",
    "parse_timestamp (src/utils/time.rs:8) - ISO8601 parsing utility"
  ],
  "summary": "Low-level primitives. These are leaves in the call graph — they call nothing else. Safe to understand but not critical for architecture comprehension."
}
```

Tool call 4 — Get hot paths:
```json
{
  "tool": "get_hot_paths",
  "arguments": {
    "limit": 10,
    "min_fan_in": 3
  }
}
```
Response:
```json
{
  "hot_paths": [
    {"symbol": "validate_token", "fan_in": 47, "location": "src/auth/jwt.rs:89"},
    {"symbol": "log_request", "fan_in": 31, "location": "src/middleware/logging.rs:34"},
    {"symbol": "check_permission", "fan_in": 28, "location": "src/auth/permissions.rs:56"},
    {"symbol": "db_query", "fan_in": 24, "location": "src/db/mod.rs:112"}
  ],
  "summary": "validate_token is called from 47 places — this is your most critical function. Any changes here will have massive blast radius. log_request is called from virtually every handler."
}
```

**The Agent's Synthesis**

The agent synthesizes this into a coherent picture:

> "This is a web API project with 4 public entry points. `handle_request` is the HTTP gateway — it routes to service layer handlers. `process_payment` is a separate payment subsystem. `authenticate` is middleware that validates tokens before handlers execute.
>
> The most critical function is `validate_token` (src/auth/jwt.rs:89) — it's called from 47 places. This is the function you should understand best and change least. If you need to modify auth, `check_permission` (called from 28 places) is also high-impact.
>
> The low-level primitives like `hash_password` and `format_currency` are leaf functions — they're utilities that call nothing else. They're safe to read for understanding but changes there don't ripple upward.
>
> Start by reading `src/api/mod.rs:12` to understand how HTTP requests flow, then `src/auth/jwt.rs:89` to understand the auth layer. Those two files will give you 80% of what you need."

**The Difference**

The traditional approach: 2-3 minutes of reading, vague understanding, possible hallucinations.

The CogniCode approach: 1.2 seconds of graph building, 4 precise tool calls, comprehensive understanding with real numbers.

---

### Example 2: Impact Analysis Before a PR

**The Situation**

You're a backend developer. You've been asked to change the signature of `UserRepository::find_by_email` to return `Result<User, UserNotFound>` instead of `Option<User>`. This is a more Rust-idiomatic error handling pattern.

But before you make this change, you need to know: what breaks?

You ask your AI assistant:

> "I'm about to change the signature of `UserRepository::find_by_email`. What's the blast radius? Which files will I need to update?"

**What a Traditional AI Agent Does**

The AI reads files that look like they might use the repository pattern. It searches for `find_by_email` in likely files. It makes guesses about what might break. It might miss some callers. It might hallucinate some. It spends 30 seconds and gives you a list that may or may not be accurate.

**What a CogniCode-Powered Agent Does**

The agent reasons: "I need to find every caller of this function and assess the risk. The `analyze_impact` tool will give me a risk level and file list. Then I can dig into the call hierarchy to understand the full picture."

Tool call 1 — Analyze impact:
```json
{
  "tool": "analyze_impact",
  "arguments": {
    "symbol_name": "UserRepository::find_by_email"
  }
}
```
Response:
```json
{
  "risk_level": "High",
  "impacted_files": [
    "src/application/auth_service.rs",
    "src/application/user_service.rs",
    "src/api/routes/auth.rs",
    "src/api/routes/admin.rs",
    "src/api/routes/user_profile.rs",
    "tests/integration/auth_test.rs",
    "tests/integration/user_service_test.rs",
    "tests/unit/user_repository_test.rs"
  ],
  "total_affected_symbols": 12,
  "estimated_change_surface": "8 files across 3 modules"
}
```

Tool call 2 — Get call hierarchy (incoming):
```json
{
  "tool": "get_call_hierarchy",
  "arguments": {
    "symbol_name": "UserRepository::find_by_email",
    "direction": "incoming",
    "depth": 4
  }
}
```
Response:
```json
{
  "hierarchy": [
    {
      "symbol": "login_user (src/api/routes/auth.rs:45)",
      "depth": 1,
      "callers": ["authenticate_request"]
    },
    {
      "symbol": "get_user_profile (src/api/routes/user_profile.rs:23)",
      "depth": 1,
      "callers": ["extract_user_id"]
    },
    {
      "symbol": "update_user (src/api/routes/admin.rs:67)",
      "depth": 2,
      "callers": ["admin_update_email"]
    }
  ],
  "total_callers": 8
}
```

**The Agent's Synthesis**

> "High risk — this change affects 8 files across 3 modules. Here's the complete blast radius:
>
> **Files you'll need to update:**
> - `src/application/auth_service.rs` — calls `find_by_email` to look up users during login
> - `src/application/user_service.rs` — uses it for user profile lookups
> - `src/api/routes/auth.rs` — login endpoint handler
> - `src/api/routes/admin.rs` — admin user management endpoints
> - `src/api/routes/user_profile.rs` — profile retrieval
> - 3 test files that mock the repository
>
> **The pattern across all callers:** They currently handle `Option<User>` with `.unwrap()` or `.expect()`. You'll need to update each call site to handle the `Result<User, UserNotFound>` case — typically changing `user.unwrap()` to `user?` or `match user { Ok(u) => u, Err(_) => return Err(...) }`.
>
> **Recommendation:** Before changing the signature, I'd suggest you add a wrapper method `find_by_email_opt` that preserves the old behavior. Then update call sites one by one. This is safer than changing everything at once.
>
> **Alternative:** Consider using the `?` operator in all call sites. Since `UserNotFound` implements `std::error::Error`, it will convert to a boxed error trait object if needed."

**The Difference**

The traditional approach: vague guess, likely missing some callers, no specific guidance on how to update each call site.

The CogniCode approach: exact file list, risk assessment, specific guidance on the change pattern needed at each call site.

---

### Example 3: Architecture Health Review

**The Situation**

You're a tech lead. Your team has been building fast, and you're worried some architectural debt has crept in. Specifically, you want to know: do we have circular dependencies? What's our architecture health score?

You ask your AI:

> "Is there any circular dependency in this codebase? Give me an architecture health score."

**What a Traditional AI Agent Does**

The AI reads through the module structure, trying to trace imports. It might find a few cycles, but it's essentially doing Tarjan's algorithm mentally, which is error-prone. It gives you an answer with low confidence and no quantitative score.

**What a CogniCode-Powered Agent Does**

The agent reasons: "The `check_architecture` tool uses Tarjan's algorithm for SCC (Strongly Connected Components) detection. This is a well-defined graph algorithm — not something an LLM should try to do mentally. Let me call it directly."

Tool call 1 — Build the graph (prerequisite):
```json
{
  "tool": "build_graph",
  "arguments": {
    "strategy": "full"
  }
}
```

Tool call 2 — Check architecture:
```json
{
  "tool": "check_architecture",
  "arguments": {}
}
```
Response:
```json
{
  "architecture_score": 85,
  "cycles_detected": 3,
  "cycle_details": [
    {
      "symbols": ["Order -> Payment -> Billing -> Order"],
      "length": 3,
      "severity": "minor"
    },
    {
      "symbols": ["User -> Auth -> Session -> User"],
      "length": 3,
      "severity": "minor"
    },
    {
      "symbols": ["Config -> Settings -> Preferences -> Config"],
      "length": 3,
      "severity": "minor"
    }
  ],
  "algorithm": "Tarjan SCC",
  "analysis_time_ms": 156
}
```

**The Agent's Synthesis**

> "Architecture score: 85 out of 100. You have 3 minor circular dependencies — not critical, but worth addressing.
>
> **The cycles:**
>
> 1. **Order → Payment → Billing → Order** — This is a domain logic cycle. The `Order` module calls `Payment`, which calls `Billing`, which calls back to `Order`. This suggests these three modules are too tightly coupled. Recommended fix: introduce a shared `BillingService` trait that both `Order` and `Payment` depend on, rather than calling each other directly.
>
> 2. **User → Auth → Session → User** — User management, auth, and session management forming a cycle. This is common in auth-heavy systems. Recommended fix: extract a `SessionStore` trait that `Auth` depends on, and have `User` depend on `Session` through an interface rather than directly.
>
> 3. **Config → Settings → Preferences → Config** — Configuration system cycle. Minor, likely just module organization.
>
> **Score interpretation:**
> - 100 = clean architecture, no cycles
> - 80-99 = minor cycles, worth addressing when convenient
> - 50-79 = significant coupling, plan refactoring
> - <50 = serious architectural problems
>
> At 85, you're in good shape. I'd recommend addressing the Order/Payment/Billing cycle before it grows — cycles have a tendency to attract more code into them over time."

**The Difference**

The traditional approach: "I don't see any obvious circular dependencies" (probably wrong).

The CogniCode approach: Exact algorithm, exact cycles, exact severity, specific refactoring recommendations.

---

### Example 4: Safe Rename Refactoring

**The Situation**

You're refactoring a legacy codebase. You've found a function called `calc_total` that's used across the project. It should be called `calculate_order_total` — the name was shortened 10 years ago and nobody fixed it.

You have 47 files in this project. You need to rename everywhere, but you don't want to break anything.

You ask your AI:

> "Rename `calc_total` to `calculate_order_total` everywhere in the project. Make sure nothing breaks."

**What a Traditional AI Agent Does**

The AI uses search-and-replace across the codebase. It might miss some cases. It might accidentally replace `calc_total` inside strings or comments where it shouldn't. It doesn't know which usages are actual code calls versus documentation. It might miss test files. It might not update import/export declarations properly.

**What a CogniCode-Powered Agent Does**

The agent reasons: "I need to follow the safe refactoring sequence: (1) analyze impact to understand scope, (2) preview the rename to see exactly what will change, (3) wait for user confirmation, (4) apply. Never rename blind."

Tool call 1 — Analyze impact:
```json
{
  "tool": "analyze_impact",
  "arguments": {
    "symbol_name": "calc_total"
  }
}
```
Response:
```json
{
  "risk_level": "Medium",
  "impacted_files": [
    "src/domain/order.rs",
    "src/application/order_service.rs",
    "src/api/routes/checkout.rs",
    "src/api/routes/invoice.rs"
  ],
  "total_affected_symbols": 6,
  "estimated_change_surface": "4 files"
}
```

Tool call 2 — Find usages:
```json
{
  "tool": "find_usages",
  "arguments": {
    "symbol_name": "calc_total",
    "include_declaration": true
  }
}
```
Response:
```json
{
  "usages": [
    {"file": "src/domain/order.rs", "line": 45, "context": "pub fn calc_total(&self) -> Money"},
    {"file": "src/application/order_service.rs", "line": 78, "context": "let total = self.order.calc_total()"},
    {"file": "src/application/order_service.rs", "line": 112, "context": "order.calc_total().into()"},
    {"file": "src/api/routes/checkout.rs", "line": 34, "context": "total: order.calc_total()"},
    {"file": "src/api/routes/invoice.rs", "line": 56, "context": "calc_total(order)"}
  ],
  "total_usages": 5
}
```

Tool call 3 — Preview the rename:
```json
{
  "tool": "safe_refactor",
  "arguments": {
    "action": "rename",
    "symbol_name": "calc_total",
    "new_name": "calculate_order_total",
    "file_path": "src/domain/order.rs",
    "line": 45,
    "column": 8,
    "preview": true
  }
}
```
Response:
```json
{
  "preview": true,
  "action": "rename",
  "affected_files": [
    {"file": "src/domain/order.rs", "changes": ["line 45: function definition"]},
    {"file": "src/application/order_service.rs", "changes": ["line 78: call site", "line 112: call site"]},
    {"file": "src/api/routes/checkout.rs", "changes": ["line 34: call site"]},
    {"file": "src/api/routes/invoice.rs", "changes": ["line 56: call site"]}
  ],
  "files_to_modify": 4,
  "total_changes": 5
}
```

**The Agent's Synthesis**

> "Medium risk rename — 4 files affected, 5 total changes. Here's what will be renamed:
>
> **src/domain/order.rs:45** — Function definition
> - `pub fn calc_total(&self) -> Money` → `pub fn calculate_order_total(&self) -> Money`
>
> **src/application/order_service.rs** — 2 call sites
> - Line 78: `self.order.calc_total()` → `self.order.calculate_order_total()`
> - Line 112: `order.calc_total().into()` → `order.calculate_order_total().into()`
>
> **src/api/routes/checkout.rs:34** — 1 call site
> - `total: order.calc_total()` → `total: order.calculate_order_total()`
>
> **src/api/routes/invoice.rs:56** — 1 call site
> - `calc_total(order)` → `calculate_order_total(order)`
>
> I notice one thing: `invoice.rs` calls it as a standalone function `calc_total(order)` rather than a method `order.calc_total()`. This suggests there might be a free function as well as the method on Order. Do you want me to rename both, or just the method?
>
> **To proceed:** Say 'yes' and I'll apply the rename to all 5 locations. Say 'no' and I'll stop here."

**What Happens Next (User Confirms)**

Tool call 4 — Apply the rename:
```json
{
  "tool": "safe_refactor",
  "arguments": {
    "action": "rename",
    "symbol_name": "calc_total",
    "new_name": "calculate_order_total",
    "file_path": "src/domain/order.rs",
    "line": 45,
    "column": 8,
    "preview": false
  }
}
```
Response:
```json
{
  "status": "success",
  "files_modified": 4,
  "total_changes": 5,
  "validation": {
    "syntax_valid": true,
    "no_breaks": true
  }
}
```

**The Agent's Confirmation**

> "Done. Renamed `calc_total` to `calculate_order_total` in 4 files (5 locations). Syntax validation passed — no broken references.
>
> I did NOT rename the free function `calc_total(order)` in `invoice.rs:56` since that appears to be a different function (standalone rather than method). If you want me to rename that too, let me know."

**The Difference**

The traditional approach: blind search-and-replace, likely to miss something, possible to break things, no validation.

The CogniCode approach: preview first, user confirmation, exact change list, syntax validation after, acknowledgment of edge cases (the standalone function).

---

## How It Works Under the Hood

You might be wondering: how does CogniCode actually work? Let me pull back the curtain.

### Parsing with Tree-sitter

The first step is parsing. CogniCode uses Tree-sitter to parse source files into Abstract Syntax Trees (ASTs). An AST is a tree representation of your code where each node represents a structural element: function definitions, function calls, type declarations, imports, etc.

For example, when Tree-sitter parses this Rust code:

```rust
pub fn calculate_order_total(&self) -> Money {
    self.items.iter().map(|i| i.price).sum()
}
```

It produces an AST where the function node has children for:
- The function name (`calculate_order_total`)
- The parameters (`&self`)
- The return type (`Money`)
- The body (the iterator chain)

This is important because it means CogniCode can distinguish between:
- A function *definition* (where `calc_total` is declared)
- A function *call* (where `calc_total()` is invoked)
- A string that happens to contain "calc_total" (which is not a reference at all)

Regex can't make this distinction. Only a proper parser can.

### Building the Call Graph

Once files are parsed into ASTs, CogniCode extracts the call graph. For each function definition, it finds all the function calls within it. This creates edges in the graph: "function A calls function B."

The call graph is stored in an embedded redb database. This is a persistent, disk-backed key-value store written in Rust. Once the graph is built, subsequent queries don't need to re-parse or re-analyze — they just query the pre-built graph.

### Staleness Detection

Here's a problem: if you edit a file, the cached graph becomes stale. CogniCode handles this with a `FileManifest` that tracks the `mtime` (modification time) of each file. Before returning results, CogniCode checks if any files have changed since the graph was built. If they have, it re-parses only those files and updates just the affected parts of the graph.

This is called incremental re-analysis, and it's critical for performance. You don't want to re-build a 2,000-file graph every time you make a small edit.

### The Four Graph Strategies

CogniCode offers four strategies for building the graph, because different tasks have different tradeoffs:

**Full (`strategy: "full"`)**: Parse every file in the project, build complete call graph with all edges. This is the most comprehensive but slowest. Use for architecture analysis, impact assessment, finding dead code.

**Lightweight (`strategy: "lightweight"`)**: Parse files for symbol definitions only, no edge analysis. Fast (seconds vs. tens of seconds). Use for symbol lookups, finding where a symbol is defined.

**On-demand (`strategy: "on_demand"`)**: Build the graph for a specific symbol and its immediate neighborhood. Use when you only care about one function and its direct callers/callees.

**Per-file (`strategy: "per_file"`)**: Build graph for a single file only. Use for file-level analysis.

### The Architecture Score

The architecture health score is computed using Tarjan's algorithm for finding Strongly Connected Components (SCCs). An SCC is a set of nodes where every node is reachable from every other node — in other words, a cycle.

Tarjan's algorithm runs in O(V+E) time (linear in the number of vertices and edges) and finds all SCCs in a single pass. The architecture score starts at 100 and subtracts 5 points for every symbol involved in a cycle. This gives you a quantitative measure of architectural health.

---

## Getting Started

CogniCode is designed to be zero-configuration. You point it at a directory and start querying.

### MCP Server Configuration

Add CogniCode to your MCP server configuration. The exact format depends on your AI assistant:

**Claude Desktop:**
```json
{
  "mcpServers": {
    "cognicode": {
      "command": "cognicode-mcp",
      "args": ["--cwd", "/path/to/your/project"]
    }
  }
}
```

**Cursor:**
```json
{
  "mcpServers": {
    "cognicode": {
      "command": "cognicode-mcp",
      "args": ["--cwd", "/path/to/your/project"]
    }
  }
}
```

**Windsurf:**
```json
{
  "mcpServers": {
    "cognicode": {
      "command": "cognicode-mcp",
      "args": ["--cwd", "/path/to/your/project"]
    }
  }
}
```

**OpenCode:**
```json
{
  "mcpServers": {
    "cognicode": {
      "command": "cognicode-mcp",
      "args": ["--cwd", "/path/to/your/project"]
    }
  }
}
```

### Your First Session

The first thing you should do in any CogniCode session is build the graph:

```json
{
  "tool": "build_graph",
  "arguments": {
    "strategy": "full"
  }
}
```

This might take a few seconds for large projects, but it's a one-time cost per session. After that, all queries are instant.

Then try:
```json
{
  "tool": "analyze_impact",
  "arguments": {
    "symbol_name": "YOUR_MOST_CALLED_FUNCTION"
  }
}
```

Replace `YOUR_MOST_CALLED_FUNCTION` with whatever function you think is critical in your codebase. The tool will tell you exactly what would break if you changed it.

---

## What's Next — The Vision Forward

CogniCode is production-ready today with 32 tools across 6 languages. But this is just the beginning.

### Near-Term Roadmap

**More languages**: C++ and Kotlin support are on the roadmap. These are widely used in enterprise environments, and adding them expands CogniCode's addressable market significantly.

**Semantic diff**: Imagine asking "show me what changed architecturally between version 1.2 and version 1.3 of this codebase." Not just which lines changed, but how the call graph evolved. Which functions gained callers? Which dependencies were introduced? This is where we're heading.

**Test coverage integration**: Imagine merging the call graph with code coverage data. "This function has 94% test coverage but it's called from 47 places and has no integration tests." The combination of structural analysis and coverage data enables smarter test prioritization.

**IDE plugin**: Right now CogniCode works through MCP. A native IDE plugin (VS Code, IntelliJ) would give human developers access to the same intelligence, with zero prompt engineering required.

### The Bigger Picture

Here's where I think this is all heading.

AI coding assistants are becoming standard tools in software development. GitHub Copilot, Claude Code, Cursor — these are no longer experiments; they're production tools that developers use daily.

But here's the problem: these AI assistants are still reading code the way humans read code — sequentially, without structural understanding. They're getting better at generating code, but they're not getting better at understanding codebases.

This is a fundamental limitation that bigger context windows won't solve. You can give an LLM a 1 million token context window, and it still won't understand the call graph as well as a tool that has explicitly built and indexed it.

**The future is agents that have structural intelligence built in.** Not because the LLM itself becomes a better structural reasoner, but because it has tools that do structural reasoning for it.

CogniCode is the beginning of this. When every AI coding assistant ships with code intelligence tools, the quality of AI-assisted development goes up dramatically. Agents that can accurately assess impact before making changes. Agents that can find circular dependencies before they cause problems. Agents that can safely refactor without breaking things.

This isn't science fiction. It's what CogniCode enables today.

### Call for Contributors

CogniCode is open source. If this vision resonates with you — if you believe AI agents deserve better tools — I invite you to contribute.

- Star the repo
- Try it on your codebase
- File issues for bugs and feature requests
- Submit PRs for languages you want supported
- Spread the word

The repo is at: [https://github.com/Rubentxu/CogniCode](https://github.com/Rubentxu/CogniCode)

---

## Try It Today

You have nothing to lose and everything to gain. Your AI coding assistant is flying blind right now. Give it the gift of structural intelligence.

Go from this: "I read 12 files and I think changing this function might affect some things."

To this: "This function is called from 47 places across 8 files. Risk level: High. Here are the exact call sites you need to update."

The difference isn't just speed. It's accuracy. It's confidence. It's knowing instead of guessing.

CogniCode gives your AI agents the same code navigation superpowers that senior developers take for granted in their IDE. Stop letting them read code like it's 1995.

**GitHub**: [https://github.com/Rubentxu/CogniCode](https://github.com/Rubentxu/CogniCode)

Star the project. Try it on your codebase. Let me know what you build.

---

*This post is also available in: [Español](blog-post-es.md)*
