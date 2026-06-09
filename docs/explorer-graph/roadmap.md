# Roadmap: From Code-Aware Static Analyzer to Navigable Graph

This file is a four-phase plan. Each phase has a clear outcome, a list of
key tasks, dependencies on earlier phases, and the risks that can derail
it. The phases are designed to ship value on their own. A phase is not a
"done" state for the project; it is a stable plateau the next phase
builds on.

The order matters. Phase 1 stabilizes the data model. Phase 2 exposes
the model through a sharper MCP surface. Phase 3 turns the model into
a navigable product. Phase 4 extends the model itself with multimodal
and federated sources.

The stack that the phases build on is fixed: a single canonical
PostgreSQL store, a `petgraph` algorithmic layer, an `sqlx` async DB
adapter, a product-owned query language (ExplorerQL, evolved from
MoldQL), and a frontend graph stack led by Cytoscape.js with `elkjs` for
hierarchical layouts. The rationale and the rejected alternatives are
in `stack-recommendation.md`, `query-language-decision.md`, and
`visualization-stack.md`.

## Phase 1: Graph Model Hardening

The product on top of the model is only as good as the model. Phase 1
turns the existing graph into a typed, provenance-aware, persistence-
ready graph. Nothing in this phase changes the user-facing surface in
a visible way; it makes everything after it possible.

### Outcomes

- Every edge in the graph has a `provenance` field and a `confidence`
  field with documented semantics.
- The canonical graph state lives in PostgreSQL behind the
  `Repository` trait. The trait surface is stable; the in-memory,
  SQLite, and Postgres implementations are all reachable from it.
- A versioned JSON graph snapshot is a first-class export and
  debugging artifact, with a documented schema. It is not the
  primary store; the canonical state is in PostgreSQL.
- The `SymbolRepository` trait is the only supported read path; ad hoc
  graph access in the explorer is removed.
- The graph data model can represent `Component`, `Container`, and
  `System` nodes, even if the UI does not yet show them.
- The hotspot, dead-code, and impact analyses carry confidence into
  their results.

### Key tasks

- Add a `provenance` enum (`extracted`, `inferred`, `ambiguous`) and
  apply it to every existing edge kind.
- Add a `confidence` field with the rules from
  `target-product-model.md` enforced in one place.
- Land the `Repository` trait in `cognicode-core` with three
  implementations: `InMemory` (default in tests), `Sqlite`
  (compatibility, behind a feature flag), and `Postgres` (target).
- Define the versioned JSON export schema with forward-compatibility
  rules and a `save` / `load` API on the repository. JSON is export
  and import only; the canonical state of truth is in PostgreSQL.
- Introduce `Component`, `Container`, and `System` node kinds and the
  `part_of`, `deployed_as`, `in_system` edge kinds, with empty
  extraction (no source produces them yet).
- Add a `cites` and `references` edge kind with no extractor attached
  (deferred to Phase 4).
- Tighten the `SymbolRepository` trait surface; deprecate direct
  in-memory access from the explorer.

### Dependencies

- None. This is the foundation phase.

### Risks

- The JSON export schema becomes a migration burden if versioned too
  loosely. The mitigation is a strict, documented schema and a
  migration test suite from day one.
- Confidence rules are easy to over-fit to current heuristics. The
  mitigation is to keep the rules in a single, well-named module and
  cover them with golden tests.
- The `SymbolRepository` trait is already in use. Tightening it is
  invasive. The mitigation is to add the new methods as required, keep
  the old ones, and remove the old ones in a later release.
- The SQLite path is preserved for migration; the risk is that it
  leaks back into the product. The mitigation is to keep it behind a
  feature flag and to remove it from the default development
  configuration in Phase 3.

## Phase 2: MCP Graph Navigation API

Phase 2 turns the MCP server from a toolbox into a graph OS. The change
is on the surface, not in the engine: the engine is what Phase 1
hardened; the surface is what agents and the explorer will both use.

### Outcomes

- A small, stable set of lower-level graph primitives is exposed as
  MCP tools: `path`, `neighbors`, `subgraph`, `cluster`, `explain`.
- A single "ask" entry point routes a natural-language question to
  the right primitive and returns an answer with provenance.
- A long-lived `brain_session` tool exists, so an agent can open a
  brain, attach to it, ask several questions, and close it cleanly.
- Every MCP tool result carries provenance and confidence metadata
  in a stable envelope.
- The Postgres-backed `Repository` implementation is wired to the
  MCP server; SQLite is still available behind a feature flag.

### Key tasks

- Implement the lower-level primitives on top of the Phase 1 model.
- Implement a question router that maps the curated question set from
  `target-product-model.md` to a chain of primitive calls.
- Define the `brain_session` lifecycle and its MCP representation.
- Define the standard result envelope: payload, provenance, confidence,
  suggested follow-up questions.
- Update the existing single-purpose tools to return the same envelope.
- Write contract tests for every tool.

### Dependencies

- Phase 1. The envelope and the routing both depend on provenance and
  confidence being real fields on edges.

### Risks

- The "ask" router can become a black box if it is not transparent.
  The mitigation is to always return the chain of primitive calls it
  used, so the caller can audit it.
- Adding a session model is a wire-protocol change. The mitigation is
  to ship the session tools as opt-in and keep the existing one-shot
  tools working.
- The envelope becomes a moving target. The mitigation is a versioned
  schema and a compatibility test in CI.

## Phase 3: Explorer UX and Dynamic Views

Phase 3 turns the model into a product users can navigate. The
audience here is human, but the verbs are the same as the agent-
facing surface in Phase 2, so the two reinforce each other.

### Outcomes

- The explorer can render any object as a contextual view: the node,
  its same-level neighbors, and its parents and children at adjacent
  levels, in one panel.
- The main interactive graph is rendered with Cytoscape.js.
  Hierarchical, C4-style, and architecture projections are laid out
  with `elkjs`. D3.js powers the specialized analytic views.
- Component, container, and system views are real, navigable, and
  shareable.
- Named views exist. A user can save the current projection as a
  named, shareable artifact.
- ExplorerQL is promoted to a first-class surface: a real grammar,
  real autocomplete, real error messages that teach the model.
- "What can I do here?" surfaces per object kind, with suggested
  questions bound to actual MCP verbs.
- The default development and CI configuration is PostgreSQL.
  SQLite is behind a feature flag only.

### Key tasks

- Define the view model declaratively: a view is a projection (node
  kinds, edge kinds, level, depth) plus a lens.
- Implement the contextual view renderer on top of the view model,
  with Cytoscape.js as the main renderer and `elkjs` for layouts.
- Implement the named view store and the share-by-link mechanism.
- Grow the ExplorerQL grammar: full boolean support, filters on
  provenance and confidence, joins across levels. Compile
  persistent traversals to PostgreSQL and algorithmic analyses to
  `petgraph`.
- Add the contextual help and suggested-questions surfaces, bound to
  the focused object's kind.
- Wire the explorer to the Phase 2 MCP tools so the UI and the
  agents share the same primitives.

### Dependencies

- Phase 2 for the shared verbs.
- Phase 1 for the model.

### Risks

- The contextual view can become cluttered. The mitigation is a
  density control and a "focus" mode that strips to the current node
  and its direct edges.
- Named views are a product surface, not just a feature. The
  mitigation is to scope them as light, shareable, link-stable
  artifacts in v1; no editing history in v1.
- ExplorerQL is a small language in its own right. The mitigation is
  to grow it in step with the curated question set, not ahead of it,
  and to keep its grammar in one well-named module.
- Migrating the default to PostgreSQL can surface performance
  surprises. The mitigation is to set a per-operation performance
  budget in CI from day one of the migration.

## Phase 4: Multimodal and Advanced Federation

Phase 4 is where CogniCode starts to look like a brain. The model
gains non-code nodes, the ingestion gains multimodal sources, and
federation becomes a first-class operation.

### Outcomes

- The graph can represent `Decision`, `Doc`, `Issue`, and `Evidence`
  nodes with their edge kinds (`cites`, `justifies`, `resolves`,
  `corroborated_by`).
- A docs source (Markdown, ADRs, runbooks) can be ingested and joined
  to the code graph with `cites` and `justifies` edges.
- An issue-tracker source can be ingested and joined with `references`
  and `resolves` edges.
- A brain can federate multiple spaces (multiple repos, multiple docs
  sources) and present them as one navigable graph.
- Corroboration is visible: edges backed by more than one source are
  styled and ranked differently.
- An opt-in, advanced query surface may be added, inspired by
  Cypher's idioms but not coupled to Cypher, and never using
  Apache AGE as a foundational dependency.

### Key tasks

- Implement the multimodal source adapters (docs, ADRs, issues).
- Implement the multimodal edge extractors with explicit
  `provenance` and `confidence` rules.
- Implement the brain layer: the model that joins spaces, computes
  corroboration, and presents the federated graph.
- Add the rationale view: a sub-graph of code nodes, the decisions
  that justify them, and the issues they resolve.
- Add the corroboration view: edges ranked and styled by the number
  and quality of their evidence.

### Dependencies

- Phase 1 for the model.
- Phase 2 for the verbs.
- Phase 3 for the views and the named view mechanism that the
  rationale and corroboration views are saved as.

### Risks

- Multimodal ingest is the most likely place for non-determinism to
  sneak in. The mitigation is to require deterministic output for
  every extractor and to test the snapshot against a fixture on
  every change.
- Federation can degrade the user experience if spaces disagree
  about the same node (different definitions of `User`, for
  example). The mitigation is a per-space identity model and a
  visible "merge candidate" UI when ambiguity is detected.
- The brain layer is a new architectural element. The mitigation is
  to land it as a thin crate that depends on core and on which the
  explorer and the MCP server both depend, so the seams are obvious.

## Cross-Phase Concerns

A few items span the whole roadmap and should not be left to a single
phase.

- **Documentation.** This set is the seed. Every phase that introduces a
  new public surface (a new edge kind, a new MCP tool, a new view)
  must extend the relevant file here. The set is the source of
  truth, not a post-hoc summary.
- **Storage discipline.** The canonical state of truth is in
  PostgreSQL. JSON graph snapshots are export and import only.
  SQLite is a compatibility layer behind a feature flag; it is
  removed from the default configuration in Phase 3 and is not
  reintroduced. Cypher and Apache AGE are not adopted; an
  opt-in, Cypher-inspired advanced surface is allowed but must not
  couple the product to either.
- **Testing.** The graph's value is its trustworthiness. Snapshot
  golden tests, contract tests on every MCP tool, and a small set of
  end-to-end "can the user answer this question" tests are part of
  the bar for each phase.
- **Performance.** The model is only useful if it stays responsive. A
  small performance budget per operation should be set in Phase 1
  and tracked in CI from Phase 2 onward. PostgreSQL-driven
  traversals and `petgraph`-driven analyses have separate budgets
  because they live on different machines.
- **Glossary.** New terms appear in every phase. Each new public term
  updates `glossary.md` in the same change.
