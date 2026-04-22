CogniCode: MCP server for AI code intelligence (built in Rust, 763 tests)

Hey r/rust,

I wanted to share CogniCode, an MCP server I built to give AI coding assistants real code intelligence. It's written entirely in Rust, and I thought this community might appreciate both the problem it solves and the implementation choices.

**The problem I was trying to solve:**

AI coding assistants navigate codebases the way humans navigate novels — line by line, starting at the top. The result is slow analysis (40+ seconds to "read" a project), hallucinated dependencies, and missed callers.

**The solution:**

CogniCode exposes 32 MCP tools that give AI agents IDE-level capabilities:
- Call graph building (4 strategies: full, lightweight, on_demand, per_file)
- Impact analysis with risk assessment
- Cycle detection using Tarjan SCC algorithm
- Hot path identification (most-called functions)
- Semantic search across the codebase
- Safe refactoring operations (rename, extract, inline, move)

**Rust implementation highlights:**

- DDD + Clean Architecture throughout
- Tree-sitter for parsing (6 languages supported)
- Embedded redb database for persistent graph cache
- 763 tests with a sandbox orchestrator for automated testing
- Mermaid export for visualization
- Context compression for natural language summaries

The architecture is organized around bounded contexts:
```
src/
├── domain/          # Aggregates, value objects, domain events
├── application/     # Use cases, command handlers
├── infrastructure/  # Tree-sitter integration, redb persistence
└── interfaces/      # MCP protocol implementation
```

**Why Rust?**

Performance was critical — building call graphs for large codebases needed to be fast enough to not interrupt the AI agent's workflow. Rust's zero-cost abstractions and fine-grained control over memory layout (especially important for the redb embedded storage) made it the obvious choice.

The persistent graph cache was another factor. Having 1200+ nodes and 3800+ edges in memory per project meant we needed something more efficient than a hash map. redb's embedded key-value store with B-Tree storage under the hood gives us exactly that.

**Questions for the community:**

1. For those who've built MCP servers — any patterns you'd recommend for handling concurrent graph builds across multiple projects?

2. The redb embedded database has been solid, but curious if others have experience with alternative embedded DBs in Rust that might be better suited for graph-like access patterns?

3. Any thoughts on the DDD structure? I've seen a lot of "DDD in Rust" examples that feel forced — happy to share more details about what worked and what didn't.

GitHub: https://github.com/Rubentxu/CogniCode

Happy to dive deeper into any part of the implementation — the Tree-sitter integration was particularly interesting to get right.
