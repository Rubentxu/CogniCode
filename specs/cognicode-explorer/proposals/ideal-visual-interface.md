# Ideal Visual Interface Proposal

Status: Superseded as the primary UX direction by `prototypes/05-moldable-inspector.html` and `proposals/cognicode-explorer-roadmap.md`. Keep this document as a concept archive for lenses, evidence, agent sidecar, and report ideas.

## Product Positioning

The application should be a moldable exploration workbench for developers.

It should not feel like:

- A static architecture diagram generator.
- A generic graph viewer.
- A chat-first AI coding assistant.
- A traditional file explorer with extra metrics.

It should feel like:

```text
An explainable system inspector where every code object can show the views, actions, searches, and reports that make sense for that object.
```

## Is Moldable Development a Good Starting Point?

Yes, but as an interaction model, not as a UI to copy.

The valuable ideas are:

- Moldable object: repository, module, file, class, function, route, PR, runtime trace, decision.
- Contextual view: the right representation for the selected object.
- Contextual action: one-click operation relevant to that object.
- Contextual search: scoped search over related objects.
- Composed narrative: saved exploration path/report explaining what was learned.
- Contextual playground: advanced area where the user/agent can run MoldQL and build a new view.

The application should adapt these ideas to modern developer UX: command palette, graph canvas, inspector panels, IDE links, MCP agents, report export, and local-first indexing.

## Core UX Principle

Use Shneiderman's mantra as the interaction spine:

```text
Overview first -> zoom and filter -> details on demand -> extract/share decision
```

For this product:

```text
System Overview -> Scope Focus -> Lens View -> Evidence Detail -> Decision Artifact
```

## Recommended Interface Shape

### 1. Command Center

Purpose: fast entry point.

Elements:

- Repo selector.
- Graph freshness/status.
- Global command palette.
- Suggested starting questions.
- Recent explorations.
- Top hotspots: impact, connascence, cycles, churn, untested critical paths.

Why it matters:

The user should get value in under 30 seconds. A blank canvas is scary; suggested entry points are better.

### 2. Scope Map

Purpose: orient the user.

Views:

- C4-style architecture overview.
- Module/community map.
- File/folder tree only as secondary navigation.
- Dependency graph with progressive disclosure.
- Changed-scope overlay for PR/branch mode.

Interaction:

- Click any node to inspect.
- Hover shows evidence summary.
- Zoom changes granularity: repo -> module -> file -> symbol.
- Filters hide noise by edge type, depth, layer, language, owner, risk.

### 3. Moldable Inspector

Purpose: the central interaction primitive.

For any selected object, show tabs generated from its type and available lenses.

Example for a module:

- Overview
- Dependencies
- Connascence
- SOLID
- Runtime flows
- Tests
- Decisions
- Reports

Example for a function:

- Signature
- Callers/callees
- Effects
- Parameters
- Connascence
- Tests
- Source
- Impact

The inspector is the most important UI. The graph is orientation; the inspector is understanding.

### 4. Lens Switcher

Purpose: change the question without losing the selected scope.

Lenses:

- Architecture
- C4
- SOLID
- Connascence
- Runtime
- Tests
- Domain vocabulary
- Ownership/churn
- Security
- PR impact

Design rule:

The same scope should look different under different lenses.

Example:

```text
Module billing + Architecture lens -> container/component map
Module billing + Connascence lens -> coupling hotspot table + graph
Module billing + Test lens -> tested/untested impact paths
Module billing + Runtime lens -> route/event/process traces
```

### 5. Evidence Drawer

Purpose: prevent AI hallucination and keep trust.

Every explanation, warning, and diagram should expose:

- Source nodes.
- Files and line ranges.
- Query used.
- Lens rule used.
- Confidence.
- Staleness.
- Agent notes.

No evidence, no claim.

### 6. Agent Sidecar

Purpose: guide, not dominate.

The agent should sit beside the visual exploration, not replace it.

Agent responsibilities:

- Explain the current view.
- Suggest next useful views.
- Convert vague questions into MoldQL.
- Summarize evidence.
- Generate reports.
- Ask for missing intent through MCP elicitation.
- Record decisions.

Anti-pattern:

Do not make the user type everything into chat. The interface should be directly manipulable.

### 7. Report Studio

Purpose: turn exploration into reusable artifacts.

Outputs:

- HTML report.
- Mermaid/PlantUML/C4.
- PNG/SVG.
- Audio walkthrough.
- PR comment.
- ADR.
- Notebook-style narrative.

Important:

Reports should be reproducible from the saved query, lens, evidence snapshot, and renderer version.

## Proposed Primary Layout

```text
┌────────────────────────────────────────────────────────────┐
│ Top Bar: Repo · Branch · Graph status · Command Palette     │
├───────────────┬─────────────────────────────┬──────────────┤
│ Scope Rail    │ Main Visual Canvas           │ Agent Sidecar│
│ Repo          │ C4 / Graph / Matrix / Flow    │ Explain      │
│ Modules       │ Linked highlighting           │ Suggest      │
│ Files         │ Zoom + filters                │ Generate     │
│ PR/Branch     │                               │              │
├───────────────┴─────────────────────────────┴──────────────┤
│ Moldable Inspector: contextual tabs for selected object      │
├──────────────────────────────────────────────────────────────┤
│ Evidence Drawer / Query Trace / Report Builder               │
└──────────────────────────────────────────────────────────────┘
```

This is not a dashboard. The final direction is a moldable Miller Columns inspector.

## Alternative Interface Concepts

### Concept A: Inspector-First Workbench

Best for serious developer workflows.

The selected object is always primary. The map, lenses, agent, and reports orbit around the inspector.

Strength:

- Best match for Moldable Development.
- Scales across scopes.
- Keeps details and evidence close.

Weakness:

- Less visually spectacular than a full-canvas graph.

Recommendation: primary product direction.

### Concept B: Infinite Architecture Canvas

Best for exploration, demos, onboarding, and team walkthroughs.

Users pan around views as cards: module maps, flow diagrams, report snippets, findings, queries.

Strength:

- Memorable and shareable.
- Good for narratives.

Weakness:

- Can become messy if not strongly constrained.

Recommendation: use as report/story mode, not default operational mode.

### Concept C: Code City / Spatial Metaphor

Best for first impression and hotspot discovery.

Files/modules become buildings/districts; complexity/churn/risk become visual dimensions.

Strength:

- Excellent spatial intuition.
- Great for onboarding and demos.

Weakness:

- Weak for precise engineering decisions.
- 3D navigation can add cognitive overhead.

Recommendation: optional overview lens, not core UX.

### Concept D: PR Review Cockpit

Best for daily high-value workflow.

Starts from changed files, shows blast radius, risky paths, missing tests, architecture drift, and suggested review order.

Strength:

- Immediate business value.
- Easier to justify adoption.

Weakness:

- Narrower than general exploration.

Recommendation: build early as a focused workflow.

## Proposed First User Flows

### Flow 1: Understand a Module

1. User opens repo.
2. App suggests top modules by centrality/churn/risk.
3. User selects `billing`.
4. Inspector opens module overview.
5. User applies connascence lens.
6. Hotspots appear as table + graph.
7. Agent explains top three risks with evidence.
8. User exports Mermaid or records an ADR.

### Flow 2: Review a PR

1. User selects current branch/PR.
2. App detects changed symbols.
3. Scope map highlights impacted modules.
4. Inspector shows blast radius and missing tests.
5. Agent proposes review order.
6. User generates PR report.

### Flow 3: Onboard to an Unknown Repo

1. User opens repo.
2. App shows system overview: entry points, modules, external dependencies.
3. User chooses "guided tour".
4. Agent creates a narrative path: architecture -> critical flows -> hotspots -> safe first files.
5. User saves tour as HTML/audio.

### Flow 4: Create a Custom View

1. User asks: "show modules that depend on infrastructure from domain code".
2. Agent translates to MoldQL.
3. User inspects generated query and result.
4. User saves it as a contextual view for repositories/modules.
5. View becomes available next time.

## Maximum Utility Rules

- Start with useful questions, not visual novelty.
- Keep graph canvas limited by progressive disclosure.
- Always show details on demand, never all details up front.
- Make every warning actionable.
- Make every agent claim evidence-backed.
- Let users save any useful exploration as a reusable contextual view.
- Preserve direct manipulation: click, filter, zoom, search, inspect.
- Make keyboard-first flows excellent: command palette, shortcuts, quick open.
- Integrate with IDE and PR tools so findings are not trapped in the app.
- Default local-first; make sharing explicit.

## MVP Interface Recommendation

Build the Inspector-First Workbench with these pieces:

- Repo command center.
- Scope map with module graph.
- Moldable inspector for modules/functions.
- Lens switcher with connascence and impact first.
- Agent sidecar.
- Evidence drawer.
- HTML/Mermaid report export.

Do not start with 3D city, full infinite canvas, or all lenses. They are attractive, but they delay the core value.

## First Screen Wireframe

```text
Open Repository
    ↓
Graph Health: Fresh / Stale / Indexing
    ↓
Suggested Questions:
  - What are the top risk modules?
  - What changed on this branch?
  - Where is coupling strongest?
  - Show me the main runtime flows.
    ↓
Architecture Overview:
  module communities + entry points + external systems
    ↓
Click module -> Moldable Inspector
```

## Design Direction

Visual style should be calm, dense, and instrument-like.

Avoid:

- Toy-like graph neon everywhere.
- Huge undifferentiated node clouds.
- Chat taking over the whole screen.
- Decorative diagrams with no evidence path.

Prefer:

- Dark technical workbench.
- High-contrast semantic colors.
- Strong typographic hierarchy.
- Small multiples for comparisons.
- Linked highlighting across views.
- Evidence badges and confidence/staleness indicators.

## Product Bet

The winning interface is not one visualization. It is a system for creating the right visualization at the right scope with the right evidence.

That is exactly where Moldable Development should influence the product.
