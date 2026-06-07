# CogniCode Explorer Context

This context defines the product language for CogniCode Explorer, an independent CogniCode-family application that helps developers explore code evidence and make software design decisions.

## Language

**Workspace**:
The complete local working environment being explored, usually one repository path plus its indexed analysis artifacts.
_Avoid_: vault, project brain, dashboard

**Scope**:
A meaningful slice of the workspace that limits the current exploration, such as a repository, module, folder, pull request, runtime trace, or feature area.
_Avoid_: namespace, domain, filter

**Module Candidate**:
A derived scope that groups files and symbols using repository structure or heuristics, but does not yet have enough boundary evidence to be treated as a real module.
_Avoid_: module, bounded context, component

**Module**:
An inspectable object with stable identity, explicit boundary rule, member files, member symbols, typed incoming/outgoing relations, and evidence blocks for membership and relations.
_Avoid_: folder, package, namespace

**Inspectable Object**:
Any software object that can be opened in the moldable inspector and shown through contextual views.
_Avoid_: node, page, item

**Symbol**:
A code-level inspectable object extracted from source, such as a function, method, struct, enum, trait, class, or interface.
_Avoid_: generic object, code item

**Property**:
Typed metadata attached to an inspectable object and used for filtering, ranking, coloring, or explaining.
_Avoid_: custom field, tag, annotation

**Typed Relation**:
A directed relationship between inspectable objects with explicit meaning, such as `CALLS`, `CALLED_BY`, `DEFINED_IN`, `IMPORTS`, `TESTED_BY`, or `VIOLATES`.
_Avoid_: link, edge, connection

**Evidence Block**:
A small piece of proof behind a claim, such as a file location, line range, metric, tool response, query result, test result, or source snippet.
_Avoid_: note, block, comment

**Contextual View**:
A representation available for an inspectable object because it makes sense for that object's type and available evidence.
_Avoid_: tab, panel, page view

**Lens**:
The active question applied to a scope or inspectable object, such as architecture, call graph, quality, connascence, tests, runtime, ownership, or security.
_Avoid_: mode, theme, visualization type

**Exploration Path**:
The ordered chain of inspectable objects opened during a session, usually represented as Miller columns from left to right.
_Avoid_: navigation history, breadcrumb only

**Decision Artifact**:
A reproducible output created from an exploration path and its evidence, such as an ADR, PR review comment, refactor proposal, test plan, report, or diagram.
_Avoid_: export, summary, document

**CogniCode Explorer**:
An independent application in the CogniCode family for moldable code exploration. It consumes CogniCode evidence but owns the Miller Columns, Spotter, contextual views, exploration path, and decision artifact workflow.
_Avoid_: dashboard, quality dashboard, generic graph UI

**Explorer API**:
The application API owned by CogniCode Explorer and optimized for visual interaction, pagination, evidence retrieval, Miller Columns state, and decision artifact creation.
_Avoid_: dashboard API, generic CogniCode API

**Explorer MCP**:
The MCP server owned by CogniCode Explorer and optimized for agent access to exploration workflows, inspectable objects, lenses, evidence, and decision artifacts.
_Avoid_: reuse-only cognicode-mcp, quality MCP

**Building Block**:
A stable domain capability that can be independently understood, tested, replaced, and extended, such as an extractor, evidence source, lens, contextual view, renderer, agent workflow, artifact generator, or graph backend adapter.
_Avoid_: helper, utility, feature blob

**Extension Point**:
A deliberate boundary where new building block implementations can be added without changing the core Explorer workflow.
_Avoid_: plugin as an afterthought, hardcoded switch, one-off integration

**Domain Service**:
A DDD-style service that coordinates domain concepts without owning UI, transport, or storage concerns.
_Avoid_: manager, god service, controller logic

**Port**:
A domain-facing interface that describes what Explorer needs from an external capability, such as code evidence, quality findings, runtime traces, graph storage, rendering, or artifact generation.
_Avoid_: direct dependency, concrete client, SDK leak

**Adapter**:
A concrete implementation of a port, such as a CogniCode MCP adapter, `cognicode-quality` adapter, `SqliteGraphStore` adapter, Kuzu adapter, or runtime trace adapter.
_Avoid_: core dependency, hidden integration, backend leak

## Example Dialogue

Developer: "I want to understand why `calculate_total` is risky."

Explorer: "Open the `calculate_total` **Symbol** as an **Inspectable Object**. Use the Call Graph **Lens** to see `CALLS` and `CALLED_BY` **Typed Relations**. Then open the Source and Evidence **Contextual Views** to inspect file locations and metrics. If the risk is real, save the **Exploration Path** as a **Decision Artifact**."

Developer: "Can I start from the whole repository?"

Explorer: "Yes, but treat the repository as a **Scope**. The MVP starts its deepest interaction at the **Symbol** level because CogniCode already exposes concrete symbol and call graph evidence."

Developer: "Should this live inside the existing dashboard?"

Explorer: "No. **CogniCode Explorer** should be an independent application in the CogniCode family, not a page or module inside `cognicode-dashboard`. It can reuse CogniCode data and services without sharing the dashboard's product shape."

Developer: "Should Explorer only consume the existing CogniCode MCP?"

Explorer: "No. Explorer should have its own **Explorer API** for the visual app and its own **Explorer MCP** for agents because it exposes a different use of the same evidence: moldable exploration and decision-making rather than generic code intelligence or quality scanning."

Developer: "How do we keep this extensible?"

Explorer: "Treat extractors, lenses, views, evidence sources, renderers, agents, artifact generators, and graph stores as **Building Blocks** behind explicit **Extension Points**. Use DDD boundaries and SOLID principles so new implementations extend Explorer without rewriting the core workflow."

Developer: "Is every folder a module?"

Explorer: "No. A folder can be a **Module Candidate**, but a **Module** must have boundary evidence: stable identity, explicit membership rules, typed relations, and evidence blocks. Folder is structure; module is a domain object with proof."
