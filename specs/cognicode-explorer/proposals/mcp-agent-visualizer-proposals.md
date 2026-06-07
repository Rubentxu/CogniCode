# MCP + Agent Visualizer Proposals

Status: Concept archive. The final direction is `CogniCode Explorer`: an independent CogniCode-family application with its own Explorer API and Explorer MCP. Keep this document for early agent roles and extension-point ideas, not as the controlling architecture.

## Product Thesis

The application should not be "chat with your repo" and should not be "a big graph viewer". It should be a moldable software inspector: the developer selects a scope, applies a lens, receives a task-fit view, and gets a decision-oriented explanation backed by repository evidence.

Core formula:

```text
Scope + Lens + Evidence + View + Agent Explanation + Decision
```

## Building Blocks

- Code graph backend: Graphify, Codebase-Memory-like MCP, GitCortex-like graph, or a custom indexed graph.
- MCP capability layer: exposes graph resources, query tools, diagram generators, impact analysis, and prompts.
- Dedicated agent: interprets developer intent, chooses tools, explains evidence, asks for missing context, and proposes actions.
- Visual application: provides inspectable objects, contextual views, overlays, navigation, and decision capture.
- Diagram DSL exporters: Mermaid, PlantUML, C4, DOT, JSON graph, and eventually custom WebGL views.
- Decision memory: ADRs, learning records, design notes, review findings, and historical architecture snapshots.

## Proposal A: MCP-First Assistant Plugin

### Shape

Start as an MCP server plus prompts/tools usable from OpenCode, Claude Code, Cursor, or any MCP host.

### Capabilities

- `index_repository(root)`
- `get_project_state()`
- `inspect_scope(scope_id, lens)`
- `trace_impact(symbol_or_file)`
- `detect_connascence(scope)`
- `evaluate_solid(scope)`
- `generate_view(scope, lens, format)`
- `record_decision(scope, decision)`

### Value

Fastest path to usefulness because developers already live in AI coding tools.

### Tradeoff

The UX remains chat-first. Good for agents, weaker for human spatial navigation.

## Proposal B: Visual App With Embedded MCP Client

### Shape

A dedicated desktop/web app acts as MCP host/client. It connects to one or more MCP servers: graph server, git server, test server, docs server, runtime trace server.

### Capabilities

- Repository entry point.
- Scope explorer: repo, module, file, class, function, route, runtime flow.
- Lens switcher: architecture, SOLID, connascence, runtime, domain, change impact.
- Inspector tabs per object.
- Agent sidecar that explains the current view and proposes next actions.
- One-click export to Mermaid, PlantUML, C4, Markdown, ADR.

### Value

Best human experience. This is closest to Moldable Development: objects become inspectable through contextual tools.

### Tradeoff

More product surface. Needs careful UX discipline or it becomes a noisy dashboard.

## Proposal C: Hybrid IDE + Visual Workbench

### Shape

MCP server powers the IDE, while a separate visual workbench opens deep views when needed.

### Capabilities

- IDE commands: `Understand module`, `Trace impact`, `Explain route`, `Generate architecture view`.
- Visual workbench: opens focused inspector for the selected code scope.
- Bidirectional links: click node opens file; selected code opens visual scope.
- PR/review mode: changed files, impact radius, SOLID/connascence warnings, diagrams, suggested tests.

### Value

Best balance. The developer stays in the IDE for coding and opens the visual workbench for understanding/deciding.

### Tradeoff

Requires good integration polish across editor, MCP, graph store, and UI.

## Recommended Direction

Build Proposal C, but phase it as:

1. Explorer-owned API/MCP contracts over CogniCode evidence.
2. Minimal visual inspector.
3. IDE bridge.
4. PR/review mode.
5. Team/shared architecture memory.

This keeps the first version useful while preserving the bigger product vision.

## Extensibility Model

Everything should be plugin-shaped around four extension points:

### 1. Extractors

Turn raw artifacts into graph evidence.

Examples: TypeScript AST, Python AST, SQL schema, OpenAPI, Terraform, Docker, CI, logs, traces, ADRs.

### 2. Lenses

Compute a perspective over graph evidence.

Examples: SOLID, connascence, C4, runtime flow, domain vocabulary, test coverage, ownership, churn.

### 3. Views

Render a lens for a task.

Examples: Mermaid graph, C4 container diagram, Sankey flow, force graph, heatmap, matrix, table, timeline.

### 4. Agents

Guide interpretation and workflow.

Examples: architecture reviewer, connascence scout, SOLID critic, onboarding guide, PR risk reviewer, test impact assistant.

## Agent Roles

### Navigator Agent

Helps the developer move from a vague question to a concrete scope and lens.

### Evidence Agent

Queries graph, source, docs, tests, git history, and traces. It must cite evidence.

### Design Critic Agent

Applies SOLID, Ousterhout, connascence, and architecture heuristics. It should propose alternatives, not just warnings.

### Diagram Agent

Chooses the right representation and generates Mermaid, PlantUML, C4, or custom graph data.

### Review Agent

Runs in PR mode: changed scopes, blast radius, risk, missing tests, architectural drift, suggested review questions.

## MVP User Journey

1. User opens a repository.
2. App indexes it or connects to an existing graph.
3. App shows a state card: architecture health, hotspots, stale graph status, top risks, entry points.
4. User clicks a module.
5. Inspector opens with tabs: Overview, Dependencies, Connascence, SOLID, Flows, Tests, Decisions.
6. User asks: "Why is this module risky?"
7. Agent answers with graph-backed evidence and highlights the relevant nodes.
8. User exports a decision view or creates an ADR.

## Maximum Utility Principles

- Start from questions, not diagrams.
- Every visualization must end in a possible decision.
- Every agent claim must link to evidence.
- Show progressive disclosure: summary first, details on demand.
- Make graph freshness visible.
- Preserve developer flow: IDE links, keyboard palette, copyable Mermaid, PR comments.
- Treat warnings as hypotheses, not verdicts.
- Keep local-first indexing as the default trust posture.

## First Vertical Slice

Build one end-to-end flow:

```text
Inspect Module -> Connascence Lens -> View Hotspots -> Agent Explains -> Export Mermaid -> Record Decision
```

This slice proves the whole product loop without needing every language, metric, or diagram type.

## Open Questions

- Which code graph backend should be the first integration: Graphify, custom, or existing MCP graph server?
- Should the visual app be local-only first or team/server mode from day one?
- Which first lens gives the most value: impact analysis, connascence, SOLID, or C4 architecture?
- Should agent outputs be stored as ADRs, annotations on graph nodes, or both?
