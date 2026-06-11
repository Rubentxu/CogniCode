# CogniCode Explorer Graph: Vision and Gap Analysis

## Bottom Line Up Front

This documentation set captures the design vision, current state audit, and
phased roadmap for evolving CogniCode from a code-aware static analyzer into a
navigable, multi-level graph product. It is grounded in three reference points:

- The current CogniCode implementation across `cognicode-core`,
  `cognicode-explorer`, and `cognicode-mcp`.
- Three external influences studied as concept models: Graphify (persistent
  multimodal code-and-docs graph), gbrain (multi-source brain with hybrid
  retrieval and explainable ranking), and the Glamorous Toolkit moldable
  exploration philosophy (contextual views and actions over high-value
  objects).
- A target product model centered on questions, projections across
  abstraction levels, and a persistent graph with explicit provenance and
  confidence.

The set also captures three coupled stack decisions: a single canonical
database (PostgreSQL), a product-owned query language (ExplorerQL, evolved
from MoldQL), and a frontend graph stack led by Cytoscape.js. Those
decisions live in the dedicated decision files called out below and are
referenced from the rest of the set.

The goal of the set is to give engineers, contributors, and reviewers a single
place to understand: where we are, what we are borrowing, what we are
becoming, and in what order we are building it.

## How to Read This Set

Read the files in this order on first contact. The dependency chain is:

1. `current-state-audit.md` - ground truth about what exists today.
2. `reference-model.md` - what we are pulling in from external influences and
   why.
3. `target-product-model.md` - the shape of the system we are aiming for.
4. `stack-recommendation.md` - the storage and engine choice. Read this
   before `core-mcp-boundaries.md` so the persistence story is clear.
5. `query-language-decision.md` - why Cypher and Apache AGE are not the
   primary surface, and what ExplorerQL is.
6. `visualization-stack.md` - which frontend library plays which role.
7. `core-mcp-boundaries.md` - which concerns live in which crate, with
   persistence and the query compiler located explicitly.
8. `query-and-navigation.md` - the user-facing verbs and the API surface
   that supports them.
9. `visualizations.md` - the views that make the graph legible.
10. `roadmap.md` - how we get there, in phases.
11. `help-and-onboarding.md` - how users learn the model.
12. `glossary.md` - shared vocabulary; reference as needed.

If you are reviewing the design, start at `target-product-model.md` and use
the audit to verify the gap claim. Then read the three decision files
(`stack-recommendation.md`, `query-language-decision.md`, and
`visualization-stack.md`) to confirm the choices behind the model. If you
are implementing, start at `roadmap.md` and read the other files when a
phase references them.

## File Map

| File                          | Purpose                                                          |
| ----------------------------- | ---------------------------------------------------------------- |
| README.md                     | This file. Orientation, BLUF, reading order.                     |
| current-state-audit.md        | Inventory of current core, explorer, and MCP features.           |
| reference-model.md            | Concepts borrowed from Graphify, gbrain, GT.                     |
| target-product-model.md       | Node kinds, edge kinds, levels, questions, C4 mapping.           |
| stack-recommendation.md       | Storage, engine, and migration direction. PostgreSQL plus       |
|                               | `petgraph` plus ExplorerQL. Rejects SQLite-as-target, JSON-as-   |
|                               | primary-store, Cypher-as-primary, and AGE-as-foundation.         |
| query-language-decision.md    | User-facing query model. Why Cypher and AGE are not the primary  |
|                               | surface; what ExplorerQL is.                                     |
| visualization-stack.md        | Frontend library roles. Cytoscape.js for the main graph;         |
|                               | `elkjs` for hierarchical layouts; D3.js for analytics.          |
| core-mcp-boundaries.md        | Crate responsibilities and anti-patterns, including the location |
|                               | of persistence and the query compiler.                           |
| query-and-navigation.md       | Low-level primitives, UX verbs, MCP and ExplorerQL ideas.        |
| visualizations.md             | Current vs target views, mapped to user value.                   |
| roadmap.md                    | Four-phase plan with outcomes, tasks, deps, risks.               |
| help-and-onboarding.md        | In-app help, prompts, progressive disclosure.                    |
| glossary.md                   | Definitions of terms used across this set.                       |

## Current State in One Paragraph

CogniCode today has a solid first layer of graph primitives: it builds a
symbol-level call graph with callers, callees, fan-in and fan-out, path
tracing, cycle detection, dead-code surfacing, hotspot ranking, impact
analysis, complexity metrics, and Mermaid export. The explorer turns some of
that into interactive views: an Object Inspector, a few lenses, a limited
MoldQL query field, an exploration path history, and evidence blocks. The MCP
server exposes a toolbox of single-purpose tools, each answering one focused
question well. What is missing is the product layer on top: there is no
multi-level architecture projection (code, component, container, system), no
first-class provenance and confidence model on edges, no multimodal graph
that joins code with docs and decisions, no community or cluster detection,
no persistent graph with a single canonical store, no strong cross-source
federation, and no in-app help or glossary that teaches the model. This set
describes how to close that gap without throwing away the working pieces.
