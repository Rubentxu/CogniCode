# CogniCode Explorer Documentation Audit

This audit lists the documents currently created for the CogniCode Explorer ideas and flags the ambiguities that must be resolved before continuing implementation.

## Current Documents

### Product Context

| Document | Role | Current Status |
|----------|------|----------------|
| `MISSION.md` | Learning/product mission for visual software understanding | Active background context |
| `CONTEXT.md` | Canonical product language for CogniCode Explorer | Active; needs review for terms that should stay product-only vs implementation-specific |
| `GLOSSARY.md` | Learning glossary | Intentionally mostly empty; only populate after concepts are demonstrated and understood |
| `RESOURCES.md` | Research/resource list | Active background context |

### Core Proposals

| Document | Role | Current Status |
|----------|------|----------------|
| `proposals/cognicode-explorer-mvp.md` | Main MVP proposal and current contract | Primary working document aligned with roadmap |
| `proposals/cognicode-explorer-roadmap.md` | Sequenced MVP and evolutive roadmap | Newly created; must be reviewed and accepted before more implementation |
| `proposals/ideal-visual-interface.md` | Earlier ideal interface proposal | Superseded in parts by the Miller Columns direction; still useful for concepts like lenses/evidence/agent sidecar |
| `proposals/query-language-and-engine-strategy.md` | MoldQL and graph/query engine strategy | Active evolutive context; MVP defers Kuzu and MoldQL |
| `proposals/mcp-agent-visualizer-proposals.md` | MCP/agent visualizer architecture | Partially superseded; still useful for agent roles and extension points |

### ADRs

| ADR | Decision | Current Status |
|-----|----------|----------------|
| `docs/adr/0001-symbol-inspector-on-cognicode-evidence.md` | MVP starts with Symbol Inspector on CogniCode evidence behind Explorer contracts | Accepted |
| `docs/adr/0002-independent-cognicode-explorer-application.md` | Explorer is independent from `cognicode-dashboard` | Accepted |
| `docs/adr/0003-explorer-api-and-mcp.md` | Explorer owns API and MCP | Accepted |
| `docs/adr/0004-explorer-product-contracts-over-cognicode-tools.md` | Explorer exposes product contracts over raw CogniCode tools | Accepted |
| `docs/adr/0005-cognicode-explorer-crate-in-cognicode-workspace.md` | Explorer starts as crate in CogniCode workspace | Accepted; existing scaffold is a seed, not the source of truth |
| `docs/adr/0006-ddd-solid-extensible-building-blocks.md` | Explorer follows DDD/SOLID with explicit extensible building blocks | Accepted as architecture constraint to review with roadmap |

### Explainers

| Document | Role | Current Status |
|----------|------|----------------|
| `explainers/visual-understanding-workflow.html` | Initial visual thinking explainer | Background learning artifact |
| `explainers/moldable-navigation-workflow.html` | Moldable navigation concepts | Active conceptual reference |
| `explainers/mcp-agent-visualizer.html` | MCP/agent visualizer explanation | Background conceptual reference |
| `explainers/query-language-strategy.html` | MoldQL/query strategy explanation | Background evolutive reference |
| `explainers/ideal-visual-interface.html` | Earlier interface explainer | Partially superseded by prototype 05 |

### Prototypes

| Document | Role | Current Status |
|----------|------|----------------|
| `prototypes/index.html` | Prototype index | Active; recommends prototype 05 |
| `prototypes/01-inspector-workbench.html` | Early dashboard/workbench prototype | Rejected direction |
| `prototypes/02-pr-review-cockpit.html` | Early PR cockpit prototype | Rejected direction |
| `prototypes/03-architecture-communities.html` | Early architecture communities prototype | Rejected direction |
| `prototypes/04-agent-exploration-flow.html` | Early agent-guided prototype | Rejected as primary direction |
| `prototypes/05-moldable-inspector.html` | GT-inspired Miller Columns prototype | Current preferred UX reference |

## Current Decision Chain

```text
Mission
  -> moldable code exploration for developer decisions
  -> not dashboard-first, not graph-first, not chat-first
  -> CogniCode Explorer as independent CogniCode-family application
  -> MVP starts with Symbol Inspector
  -> UI talks to Explorer API
  -> agents talk to Explorer MCP
  -> DDD/SOLID boundaries and extensible building blocks guide implementation
  -> Explorer aggregates CogniCode code evidence behind product contracts
  -> later evolutives add modules, quality, connascence, SOLID, runtime, MCP tools, agents, MoldQL, Kuzu
```

## Resolved Alignment Decisions

| Area | Resolution |
|------|------------|
| Product name | Canonical name is `CogniCode Explorer`; use `Explorer` only as shorthand after context is clear |
| ADR 0002 filename | Renamed to `0002-independent-cognicode-explorer-application.md` |
| MVP data path | Evidence comes from CogniCode core/db/tools behind Explorer-owned ports/adapters; the public UI contract is Explorer API |
| Implementation timing | Existing crate scaffold is a seed; roadmap/specification controls future implementation |
| First route to implement | Phase 1A is known-symbol inspection; Phase 1B adds Spotter search |
| Artifact format | JSON replay is canonical; Markdown is first human renderer; HTML, queries, tables, and diagrams are future renderers |
| UI stack | React 19 + Tailwind CSS; visual-thinking libraries are replaceable building blocks |
| Evidence model | Phase 1 `EvidenceBlock` minimum accepted; UI claims require `evidence_ids[]` |
| Module model | Module starts as `ModuleCandidate` derived scope; it becomes a real inspectable object only with boundary/evidence |
| Graph identity | MVP can use `symbol:{file}:{name}:{line}` for UI/API, but persisted paths/artifacts require versioned `ObjectIdentity` |

## Implementation Status

| Area | Status | Next Step |
|------|--------|-----------|
| Roadmap acceptance | Roadmap accepted as the controlling implementation sequence | Continue with Phase 1A: known-symbol inspection |

## Roadmap Status

`proposals/cognicode-explorer-roadmap.md` now defines a sequenced roadmap with phases, dependencies, acceptance criteria, and cut lines.

It has been reviewed and accepted as the controlling implementation sequence.

## Review Order

Review the documents in this order:

1. `CONTEXT.md`
2. `docs/adr/0001-symbol-inspector-on-cognicode-evidence.md`
3. `docs/adr/0002-independent-cognicode-explorer-application.md`
4. `docs/adr/0003-explorer-api-and-mcp.md`
5. `docs/adr/0004-explorer-product-contracts-over-cognicode-tools.md`
6. `docs/adr/0005-cognicode-explorer-crate-in-cognicode-workspace.md`
7. `docs/adr/0006-ddd-solid-extensible-building-blocks.md`
8. `docs/adr/0007-known-symbol-inspection-before-spotter.md`
9. `docs/adr/0008-json-replay-artifacts-with-extensible-renderers.md`
10. `docs/adr/0009-react-tailwind-visual-thinking-ui.md`
11. `proposals/cognicode-explorer-mvp.md`
12. `proposals/cognicode-explorer-roadmap.md`
13. `docs/adr/0010-minimum-evidence-block-contract.md`
14. `docs/adr/0011-module-candidate-before-module.md`
15. `docs/adr/0012-versioned-object-identity-for-persisted-artifacts.md`
16. `proposals/query-language-and-engine-strategy.md`
17. `prototypes/05-moldable-inspector.html`

Product implementation may continue by following Phase 1A: known-symbol inspection.
