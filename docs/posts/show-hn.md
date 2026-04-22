Show HN: CogniCode — MCP server that gives AI agents IDE-level code intelligence

Hey HN,

I built CogniCode because I kept watching AI coding assistants read code the way humans read novels — starting at page one, hoping to find relevant sections.

The result: Hallucinated dependencies, missed callers, 40-second analysis that tells you nothing useful.

CogniCode is an MCP server that gives AI agents the same code navigation tools that senior developers get from IntelliJ or VS Code:

- **build_graph**: Build complete call graphs (1247 nodes, 3891 edges in ~234ms for a mid-size project)
- **analyze_impact**: "What breaks if I change this function?" — returns exact files, risk level, callers
- **get_hot_paths**: Most-called functions in one tool call
- **check_architecture**: Cycle detection using Tarjan SCC algorithm
- **trace_path**: Find execution path between two symbols

Tech stack:
- Rust with DDD + Clean Architecture
- Tree-sitter for parsing (6 languages: Rust, Python, TS, JS, Go, Java)
- Embedded redb database for persistent graph cache
- 763 tests with sandbox orchestrator

Zero config required:
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

Works with Claude Desktop, Cursor, Windsurf, OpenCode, any MCP-compatible AI assistant.

GitHub: https://github.com/Rubentxu/CogniCode

Would love feedback from the community. Happy to answer questions about the architecture, the MCP protocol, or how the graph building works.

(Also: first Rust project of this scale — any Rust folks want to riff on the architecture, I'm all ears.)
