# Visualizations: Current and Target

This file catalogs the visualizations CogniCode Explorer ships today
and the ones the target product model requires. The table below is
the contract for the views layer: each row says what the view
shows, what level it operates at, and what user value it delivers.
The "current / target" column is honest. Where the current state is
good, the row says so; where it is missing, the row says so too.

The level column uses the C4 vocabulary from
`target-product-model.md`: code, component, container, system. A
view that spans levels is marked "cross-level".

The library assignments for these views are decided in
`visualization-stack.md`. The summary: Cytoscape.js renders the
main interactive graph; `elkjs` produces hierarchical and
architecture layouts that Cytoscape.js consumes; D3.js powers
specialized analytic views. Mermaid covers the export and
sharing surface.

## Visualization Catalog

| Visualization                  | Current / Target   | Level          | What it shows                                                  | User value                                        |
| ------------------------------ | ------------------ | -------------- | -------------------------------------------------------------- | ------------------------------------------------- |
| Object Inspector               | Current, extend    | Code           | Definition, callers, callees, complexity, evidence, rationale.  | The "what is this thing" anchor.                  |
| Caller / Callee table          | Current, extend    | Code           | One-level list of direct callers or callees.                   | Quick local navigation.                           |
| SVG call graph                 | Current, extend    | Code           | A focused call subgraph rendered as an SVG graph.              | Visual sense of structure for small regions.      |
| Mermaid viewer                 | Current, keep      | Code, any      | Mermaid export of any subgraph.                                | Shareable, portable, pasteable diagrams.          |
| Quality card                   | Current, keep      | Code           | Per-symbol complexity and smell summary.                       | At-a-glance health check.                         |
| Evidence block                 | Current, extend    | Code, any      | Sources backing the current view.                              | Trust signal.                                     |
| Exploration path history       | Current, keep      | Any            | Back / forward stack of views.                                 | Recoverable navigation.                           |
| Lens selector                  | Current, keep      | Any            | Switch the lens on the current view.                           | Same model, different story.                      |
| Contextual view (cross-level)  | Target             | Cross-level    | The focused node, its same-level neighbors, and its parents    | The "where am I" panel inspired by GT.            |
|                                |                    |                | and children at adjacent levels.                              |                                                   |
| Community map                  | Target             | Code           | Leiden-style clusters of symbols, sized by community mass.     | Architectural seams and god modules.              |
| Component graph                | Target             | Component      | Components as nodes, dependency edges between them, styled by  | "What is the shape of the system at this level?"  |
|                                |                    |                | `part_of` and coupling strength.                               |                                                   |
| Container graph                | Target             | Container      | Containers as nodes, with the components they contain as a     | "What runs together? Where is it deployed?"       |
|                                |                    |                | sub-layer, plus the `in_system` edges to systems.             |                                                   |
| System projection              | Target             | System         | The system as a single panel with its containers, their        | "What are the moving parts of the product?"       |
|                                |                    |                | components, and the cross-container edges.                     |                                                   |
| Corroboration view             | Target             | Any            | A subgraph filtered to edges backed by more than one source of | "Where is the evidence strongest? Where is it      |
|                                |                    |                | evidence, with each edge styled by the number of sources.      | weakest?"                                         |
| Rationale graph                | Target             | Code, doc      | A sub-graph of code nodes, the decisions that justify them,   | "What is the why of this code?"                   |
|                                |                    |                | the issues they resolve, and the docs that cite them.          |                                                   |
| Dependency heatmap              | Target             | Component,     | A matrix of dependencies between components or containers,     | Hot spots and isolation problems in one glance.   |
|                                |                    | container      | shaded by coupling strength.                                   |                                                   |
| Architecture projection panel  | Target             | Cross-level    | A single panel showing the focused node, its component, its    | The "where does this fit" answer, in one view.    |
|                                |                    |                | container, and its system, with the climb-up path highlighted. |                                                   |
| "What changed" timeline        | Target             | Any            | A time axis with edges added and removed in the window,        | "What moved recently? What should I look at?"     |
|                                |                    |                | grouped by level.                                              |                                                   |
| Risk overlay                   | Target             | Any            | An overlay on any graph view that styles nodes and edges by    | "What is risky to change here?"                   |
|                                |                    |                | a risk score (fan-in, complexity, churn, coverage).            |                                                   |
| Named view landing             | Target             | Any            | The page a shared view link opens to, with the projection,     | A team-shareable navigation primitive.            |
|                                |                    |                | lens, and focused node restored.                               |                                                   |
| Suggested questions panel      | Target             | Any            | A small panel bound to the focused object kind, listing the    | "What can I do here?" and a non-trivial on-ramp.  |
|                                |                    |                | curated questions the user can ask next.                       |                                                   |

## How the Views Compose

The views are not separate products. They are projections of one
graph, with a level filter, a lens, and a focus node. The same
node, rendered through a different combination, becomes a
different view in the table.

The composition rules:

- One view has one level, one lens, one focus node, and a bounded
  radius. The view is the four-tuple.
- A panel is a layout of one or more views. The contextual view
  panel is a layout of three views at three levels, sharing the
  focus node by climb-up projection.
- A named view is a saved four-tuple plus a name, a description,
  and a creation time. A shared link resolves to a named view.

The renderer for a view is determined by the shape of the view
according to the decision table in `visualization-stack.md`. The
discipline is: navigable interactive graphs go to Cytoscape.js;
hierarchical layouts go to `elkjs` plus Cytoscape.js; matrices,
heatmaps, and small-multiple charts go to D3.js. Mermaid covers
the export and paste surface only.

This composition is what keeps the view layer tractable. The
product grows by adding lenses and by adding layouts, not by
adding bespoke screens.

## Current State Notes

The current explorer is honest about what it has. The notes below
are the bridge to the target.

- The SVG call graph is good. The target keeps it as a rendering
  backend for any `subgraph` primitive, not as a standalone view.
- The Mermaid export is good. The target keeps it as a sharing
  primitive; every named view can export to Mermaid.
- The Object Inspector is the seed of the contextual view. The
  target grows it into a cross-level panel without losing the
  current focus on definition and callers.
- The lens selector and the exploration path history are seeds
  of the view composition model. The target grows them into the
  named-view mechanism.
- The quality card is a good code-level lens. The target grows it
  into a generalized "any metric, any level" lens.

## What Is Explicitly Out of Scope

Some visualizations look attractive but are not on the target list.
The product is a navigation tool, not a dashboarding tool. The
list below is what we are choosing not to build, and why.

- A general-purpose metrics dashboard. Out of scope because it
  is a different product. A user who wants a dashboard exports
  the graph to a tool that is one.
- A code-coverage heatmap of the repository. Out of scope because
  it does not depend on the graph; it depends on coverage data
  the graph does not own.
- A 3D graph. Out of scope because it does not pay for its
  cognitive cost. The 2D projections, sized and styled well, are
  enough.
- A "live edit" view that watches the user type and re-renders.
  Out of scope because the graph is a snapshot; live editing is
  a different model.
