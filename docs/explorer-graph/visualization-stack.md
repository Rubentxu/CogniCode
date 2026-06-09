# Visualization Stack

## Bottom Line Up Front

The Explorer's frontend baseline stays: React 19, TypeScript,
and Tailwind 4. The main interactive graph is rendered with
Cytoscape.js. Hierarchical, C4-style, and architecture
projections are laid out with `elkjs`. D3.js stays in the
toolbox as a supporting library for specialized analytic
visualizations, not as the primary graph engine. This file
explains the choice and gives a decision table for new
views.

The visualization decision is coupled with the storage
decision in `stack-recommendation.md` (the data the views
render) and with the query decision in
`query-language-decision.md` (the verbs that drive the
views). Read the three together.

## Current Frontend Baseline

The current explorer is a React 19 plus TypeScript
single-page application. Styling is Tailwind 4. The view
layer is built on a small set of components: the Object
Inspector, the lens selector, the Mermaid viewer, the
quality card, the exploration path history, and the
evidence block. The audit in `current-state-audit.md`
lists the gaps; the target visualization catalog is in
`visualizations.md`.

The gap the new visualization stack closes is the
rendering of the main interactive graph and the C4-style
projections. Mermaid covers export and small diagrams; the
main interactive surface needs a real graph engine.

## What This File Decides

The visualization layer has three concerns:

1. **The main interactive graph.** The user pans, zooms,
   expands, collapses, and follows edges. The interaction
   model is "you are in a graph; everything else is a view
   on it".
2. **Hierarchical and architecture layouts.** C4 levels,
   component dependency graphs, container graphs, and
   system projections. The user wants the layout to be
   legible, not just correct.
3. **Specialized analytic visualizations.** Dependency
   heatmaps, community maps styled as chord diagrams,
   corroboration views, time series, and one-off analytic
   charts. These are smaller, sharper, and rarer than the
   main graph.

The decision below assigns each concern to a library. The
discipline is that each library plays the role it is good
at; no library is forced into a role it is bad at.

## Libraries Considered

### D3.js

D3.js is a low-level visualization toolkit. It binds data
to the DOM and offers a set of reusable modules for scales,
axes, hierarchies, and forces.

| Aspect                | Assessment                                              |
| --------------------- | ------------------------------------------------------- |
| Main graph rendering  | Possible; force layouts and selections are workable.    |
| Hierarchical layouts  | Possible; D3's hierarchy module is solid.               |
| Specialized analytics | Excellent. The strongest option.                        |
| Performance at scale  | Acceptable up to a few thousand nodes.                  |
| Engineering cost      | High. D3 is a toolkit, not a graph engine. The team    |
|                       | owns the data model, the interaction model, the        |
|                       | layout, and the rendering.                              |
| Reuse                 | Low for the main graph; high for analytics.             |
| Risk                  | The team rebuilds a graph engine to render the main    |
|                       | graph; the rebuild eats the budget for the analytic    |
|                       | views.                                                  |

**Verdict.** D3 stays as a supporting library for
specialized analytic views. It does not become the primary
graph engine. The team uses D3 where D3 is the strongest
option, not where it is the only option.

### Cytoscape.js

Cytoscape.js is a graph theory library and a renderer. It
ships with a graph data model, an event model, selectors, a
collection API, layouts, and style rules. It is built for
the "main interactive graph" use case.

| Aspect                | Assessment                                              |
| --------------------- | ------------------------------------------------------- |
| Main graph rendering  | First-class. The engine it ships is the engine we want. |
| Hierarchical layouts  | Possible; some layouts (breadthfirst, grid) are usable. |
|                       | Less rich than dedicated layout engines.                |
| Specialized analytics | Possible but not the strength.                          |
| Performance at scale  | Good. Handles tens of thousands of nodes with care.     |
| Engineering cost      | Low. The library owns the data model, the interaction  |
|                       | model, and the rendering. The team owns the views,     |
|                       | the lenses, and the integration.                        |
| Reuse                 | High for the main graph.                                |
| Risk                  | Layout quality for hierarchical projections needs a    |
|                       | companion. That companion is `elkjs`.                   |

**Verdict.** Cytoscape.js is the recommended primary graph
engine. It owns the main interactive surface. The team's
effort goes into views, lenses, and integration, not into
rebuilding a graph engine.

### React Flow

React Flow is a node-based UI library for building editors
and diagrams. Its model is "nodes you place and connect",
not "a graph you traverse".

| Aspect                | Assessment                                              |
| --------------------- | ------------------------------------------------------- |
| Main graph rendering  | Good for editor-shaped graphs. Less good for graph-    |
|                       | shaped exploration.                                     |
| Hierarchical layouts  | Possible via custom code.                               |
| Specialized analytics | Possible but not the strength.                          |
| Engineering cost      | Low for editor-shaped graphs; high for graph-shaped     |
|                       | exploration.                                            |
| Reuse                 | Mixed.                                                  |
| Risk                  | The model is wrong for the use case. The Explorer is   |
|                       | not a node editor; it is a graph navigator.            |

**Verdict.** React Flow is the right tool for editor-shaped
graphs (for example, building a custom view, designing a
template, or wiring named views by hand). It is the wrong
tool for the main interactive graph and for the C4
projections. It is not adopted for the main engine; it
remains an option for editor-shaped surfaces in the
product.

### ELK (`elkjs`)

ELK is the Eclipse Layout Kernel. `elkjs` is its JavaScript
build. It produces high-quality layouts for hierarchical,
orthogonal, and layered graphs. It is a layout engine, not
a renderer; the team feeds the layout result to a renderer.

| Aspect                | Assessment                                              |
| --------------------- | ------------------------------------------------------- |
| Main graph rendering  | Not a renderer; no event model, no selection model.    |
| Hierarchical layouts  | First-class. The strongest option.                     |
| Architecture and C4   | First-class. Layered and orthogonal layouts are         |
|                       | exactly what architecture diagrams want.               |
| Specialized analytics | Not applicable.                                         |
| Performance           | Adequate for the size of the projections we expect.    |
| Engineering cost      | Low for the layout. The team owns the integration with |
|                       | a renderer.                                             |
| Reuse                 | High for projections.                                   |
| Risk                  | A second library to integrate. Mitigated by a clear    |
|                       | role: `elkjs` produces layouts, Cytoscape.js renders.  |

**Verdict.** `elkjs` is the recommended layout engine for
hierarchical, C4, and architecture projections. It pairs
with Cytoscape.js: `elkjs` produces the layout,
Cytoscape.js renders and handles interaction.

## The Recommended Stack

| Concern                        | Library            | Role                                       |
| ------------------------------ | ------------------ | ------------------------------------------ |
| Frontend baseline              | React 19 plus TS   | Component model and application shell.     |
| Styling                        | Tailwind 4         | Design tokens and layout primitives.       |
| Main interactive graph         | Cytoscape.js       | Data model, interaction, rendering.        |
| Hierarchical and C4 layouts    | `elkjs`            | Layouts only.                              |
| Specialized analytic views     | D3.js              | Heatmaps, time series, chord diagrams.     |
| Sharing and export             | Mermaid            | Portable, pasteable, shareable diagrams.   |

The integration rule is fixed. `elkjs` produces a layout.
Cytoscape.js consumes the layout and renders. The same
data model feeds both. D3.js receives data shaped for the
analytic view it renders; it does not share the graph data
model with Cytoscape.js.

## When D3.js Is the Right Tool

D3.js is the right tool when the view is not the main
graph. The list below is the working set.

- A dependency heatmap between components. The data is a
  matrix; the rendering is a grid of cells. D3's scales
  and axes are the strongest tool for the job.
- A community map styled as a chord diagram. The data is
  pairwise coupling; the rendering is a circular layout.
  D3 ships the idiom.
- A "what changed" timeline. The data is time-windowed
  additions and removals; the rendering is a small-
  multiples chart. D3's time scales and transitions are
  the right fit.
- A corroboration view styled as a stacked bar. The data
  is evidence counts per edge; the rendering is a small
  bar chart per edge. D3 is the right tool.

D3.js is the wrong tool when the view is the main graph.
The team does not rebuild a graph engine to render a
graph.

## When Cytoscape.js Is the Right Tool

Cytoscape.js is the right tool for any view whose primary
user action is "follow an edge in an interactive graph".

- The Object Inspector's neighbor lists, when they become
  clickable, navigable mini-graphs.
- The community map when it is rendered as a navigable
  graph (the heatmap and chord-diagram views are D3
  alternatives for the same data).
- The component, container, and system graphs.
- The corroboration view when it is rendered as a
  navigable graph (the stacked-bar view is a D3
  alternative).
- The rationale graph and the architecture projection
  panel.

## When `elkjs` Is the Right Tool

`elkjs` is the right tool for any view that needs a
hierarchical, layered, or orthogonal layout.

- The C4-style architecture projection.
- The component dependency graph.
- The container graph.
- The system projection.
- Any "tree of" view where the user reads top-to-bottom
  or left-to-right.

The integration pattern is fixed. The explorer calls
`elkjs` with a graph description, receives positioned
nodes and edges, and feeds them to Cytoscape.js for
rendering. The two libraries do not duplicate work; they
divide work along the layout-versus-render line.

## A Decision Table for New Views

When a new view is proposed, the table below answers
"which library?".

| Shape of the view                                  | Use                  |
| -------------------------------------------------- | -------------------- |
| Interactive navigable graph                        | Cytoscape.js         |
| Hierarchical, layered, or orthogonal layout        | `elkjs` plus Cytoscape |
| Matrix or heatmap                                  | D3.js                |
| Time series, small multiples, bar / line / chord   | D3.js                |
| Editor (place and connect nodes by hand)           | React Flow           |
| Portable, pasteable, shareable diagram (export)    | Mermaid              |

The discipline is to pick the row that matches the shape
of the view. Do not generalize D3.js into the main engine.
Do not generalize Cytoscape.js into an analytic tool. Do
not generalize Mermaid into the interactive surface. Each
library plays the role it is good at.

## Performance Notes

- Cytoscape.js handles the interactive surface up to tens
  of thousands of nodes with proper styling and a bounded
  layout radius.
- `elkjs` is heavier per layout call. The team caches
  layouts per named view; recomputation happens on view
  re-open, not on every render.
- D3.js is per-view. Each analytic view renders the slice
  of data it needs; it does not re-render the main graph.

A performance budget per view is set in `roadmap.md` and
tracked in CI.

## What This File Rejects

- D3.js as the primary graph engine. D3 is a toolkit; the
  product does not pay to rebuild a graph engine.
- React Flow for the main interactive graph. The Explorer
  is a navigator, not a node editor.
- Mermaid as the interactive surface. Mermaid is an export
  format; the interactive surface needs selection,
  filtering, and lens changes that Mermaid does not
  provide.

## Related Documents

- `stack-recommendation.md` - the storage and engine
  choice.
- `query-language-decision.md` - the query model.
- `visualizations.md` - the catalog of views.
- `target-product-model.md` - the model the views render.
- `core-mcp-boundaries.md` - where rendering lives.
- `roadmap.md` - the staged growth of the view layer.
