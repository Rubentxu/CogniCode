# CogniCode Diagrams

This directory contains Mermaid diagrams documenting the CogniCode architecture and workflows.

## Diagrams

### 1. Architecture Diagram (`architecture-diagram.mmd`)
High-level architecture showing the four main layers:
- **Interface Layer**: MCP Server, LSP Server, CLI
- **Application Layer**: NavigationService, RefactorService, AnalysisService
- **Domain Layer**: Symbol, CallGraph, Refactor, ImpactAnalyzer, CycleDetector, ComplexityCalculator
- **Infrastructure Layer**: TreeSitter Parser, PetGraph Store, Virtual File System, Safety Gate

### 2. MCP Workflow (`mcp-workflow.mmd`)
Sequence diagram showing the MCP request/response flow:
- LLM Agent sends tool_call requests
- Request flows through MCP Server, Application Services, Domain, and Infrastructure
- Response returns with parsed AST and symbols

### 3. Refactor Workflow (`refactor-workflow.mmd`)
Safe refactoring flow with Safety Gate validation:
- Agent request triggers parse and validation
- Invalid requests return errors immediately
- Valid requests generate edits and apply to VFS
- AST validation occurs before committing to disk
- Invalid ASTs trigger rejection and rollback

### 4. Call Graph (`call-graph.mmd`)
Cycle detection visualization:
- Visual representation of function call relationships
- Nodes involved in cycles are highlighted in red
- Demonstrates how cycle detection identifies circular dependencies

## Usage

These diagrams can be rendered in any Mermaid-compatible viewer:
- GitHub/GitLab markdown files
- VS Code with Mermaid extension
- Mermaid Live Editor (https://mermaid.live)
- MkDocs with mermaid2 plugin