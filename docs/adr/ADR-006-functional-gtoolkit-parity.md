# ADR-006: Functional GToolkit parity through MoldQL, ViewSpecs, and evolving graph

**Status**: SUPERSEDED (promoted 2026-08-10)
**Date**: 2026-07-02  
**Deciders**: User, OpenCode orchestrator grill-with-docs session 2026-07-02

## Context

CogniCode targets functional parity with GToolkit and moldable development,
but it runs on a different architecture: Rust services, PostgreSQL-backed graph
state, and a React Explorer UI. We do not want to copy Pharo/Smalltalk image
semantics. We want the same practical affordances:

- software objects are inspectable;
- each object has useful contextual views;
- exploration preserves narrative through pane-based navigation;
- users create micro-tools in situ from the object they are inspecting;
- narratives link code, architecture, decisions, evidence, and queries;
- the graph improves over time without losing provenance.

ADR-002 established the moldable exploration parity program. Since then,
CogniCode shipped a GtPager-like pane stack, a broader Spotter, ViewSpec
authoring, Investigation Mode, EvidencePack, ComposedNarrative, and several
additional ViewExecutors. The remaining risk is conceptual drift: MoldQL,
ViewSpecs, Spotter, C4, investigations, and graph enrichment could grow as
separate mechanisms instead of one coherent moldable system.

This ADR fixes the product and architecture direction for that system.

## Decision

We will pursue functional GToolkit-style moldability through one coherent
model: **MoldQL + ViewSpec + inspectable objects + relation candidates**.

### 1. MoldQL is the single product query language

CogniCode exposes one user-facing query language: `MoldQL`.

Historical or internal names such as `ExplorerQL` may remain in parser modules
or comments while the code is migrated, but they are not separate product
languages. Documentation, UI copy, scaffolds, ViewSpec authoring, and MCP
surfaces should present the language as MoldQL.

MoldQL is not SQL, Cypher, GQL, or Gremlin. It borrows mature property-graph
concepts where useful:

- paths;
- neighbors;
- subgraphs;
- clusters;
- labels and types;
- edge kinds;
- direction and depth;
- path constraints;
- explainability.

However, MoldQL maps those concepts to CogniCode's software-object vocabulary
and typed Rust execution plans.

### 2. MoldQL has two syntax levels inside one language

MoldQL supports two user-facing levels:

1. **Intent-first syntax** for everyday exploration and scaffolds:
   - `vertical slice from use_case "PinEvidence"`
   - `tests covering symbol "pin_evidence_handler"`
   - `decisions affecting c4_component "Investigation API"`

2. **Graph-primitive syntax** for precise graph operations:
   - `path from symbol "A" to symbol "B" through calls max depth 3`
   - `neighbors of c4_component "Explorer API" direction outgoing`
   - `subgraph around use_case "PinEvidence" depth 2`

Both levels compile to typed MoldQL AST/query plans. ViewKind-oriented intents
compile to deterministic CogniCode operations and ViewExecutor/data-source
pipelines; they are not LLM magic and do not invent unsupported relations.

### 3. ViewSpec is the canonical moldable view unit

A query is not itself a reusable view. A query becomes a contextual tool only
when captured in a `ViewSpec` with:

- semantic intent (`view_kind`);
- data source (`MoldQL` query);
- optional transform (`JSONata`);
- rendering strategy (`renderer_kind`);
- applicability metadata.

`JSONata` remains an optional post-query transform. It is not a competing
language for selecting inspectable software objects.

### 4. ViewSpec applicability uses `applies_to` plus `applies_when`

Runtime views should not be forced into only object-specific or type-wide
scope. A ViewSpec uses:

- `applies_to`: the primary inspectable object type;
- `applies_when`: an optional predicate-only MoldQL subset over the active
  object.

Implementation is phased:

1. Persist and display `applies_when` while filtering remains based on
   `applies_to`.
2. Evaluate `applies_when` in `ViewRegistry.list_for_with_store()` once the
   predicate evaluator exists.

### 5. ViewSpecs created in situ capture origin metadata

When a user creates a ViewSpec from an inspection pane, CogniCode captures the
origin context:

- `seed_object_id`;
- `seed_view_id`.

This metadata explains where a micro-tool came from, enables better query
scaffolding, and lets narratives reference the context that produced a view.

### 6. ViewSpec authoring is object-first and scaffold-first

The primary authoring flow starts from an active inspection pane, not from a
generic dashboard-builder screen.

The wizard flow is:

1. Inspect an object.
2. Select **Create custom view**.
3. Choose an intent/scaffold.
4. CogniCode pre-fills MoldQL, ViewKind, and RendererKind.
5. The user edits, previews, and saves.

Manual MoldQL remains available as an advanced escape hatch.

### 7. Query scaffolds live in a versioned registry

MoldQL scaffolds are product semantics, not ad-hoc UI strings. The source of
truth is a versioned YAML/JSON registry, initially located at:

```text
crates/cognicode-explorer/assets/moldql-scaffolds.yaml
```

Rust validates the registry. TypeScript consumes generated or validated
definitions. The same registry feeds:

- ViewSpecWizard;
- Spotter intent actions;
- SuggestionStrip;
- typed overview views;
- ProjectDiary and ComposedNarrative snippets;
- MCP surfaces.

Each scaffold carries semantic metadata, including:

- `id`;
- `object_type`;
- `intent`;
- `label`;
- `description`;
- `query_template`;
- `view_kind`;
- `renderer_kind`;
- optional `applies_when`;
- relation-candidate metadata when applicable.

Scaffold coverage ships in waves:

1. Wave 1: `symbol`, `file`, `scope`, `investigation`, `viewspec`.
2. Wave 2: `route`, `use_case`, `c4_component`.
3. Wave 3: `adr`, `doc`, `evidence`, `relation_candidate`.

### 8. MoldQL primary results are inspectable

The primary result of a MoldQL query must be navigable. Primary items carry:

- object identity;
- object type;
- label;
- available views.

Anonymous JSON may exist only as secondary block data inside a view, not as the
conceptual center of MoldQL.

### 9. CogniCode uses a shared inspectable object catalog

Spotter, MoldQL, ViewSpecs, panes, and graph enrichment share one object
vocabulary. The catalog is closed enough for consistency and extensible through
explicit architecture decisions.

Base families are:

- Code: `workspace`, `scope`, `module`, `file`, `symbol`, `route`,
  `use_case`, `test`.
- Architecture: `c4_system`, `c4_container`, `c4_component`, `c4_code`,
  `boundary`, `dependency`.
- Knowledge: `adr`, `decision`, `doc`, `evidence`, `issue`, `quality_issue`,
  `rule`.
- Exploration: `investigation`, `exploration_session`, `viewspec`,
  `artifact`, `relation_candidate`.

Rules:

- a Spotter family should be inspectable;
- an inspectable object should be eligible for views;
- ViewSpecs should target only catalogued object types;
- RelationCandidate endpoints should be catalogued objects.

### 10. UseCase is an object; vertical_slice is a view

`UseCase` represents a user-visible or system-visible behavior. It may be
inferred from routes, commands, handlers, tests, naming conventions, or
explicit metadata.

`vertical_slice` is a ViewKind that explains the flow for a use case or entry
point. It is not a separate object type.

### 11. C4 elements are inspectable graph objects

C4 systems, containers, components, and code-level elements are inspectable
objects, not only diagram nodes. They should link to:

- implementation scopes/modules/symbols;
- use cases crossing them;
- routes entering them;
- ADRs and boundary decisions;
- tests;
- evidence;
- relation candidates.

C4-to-code links may be explicit, inferred, or proposed as
RelationCandidates. Inferred links require confidence/provenance and must be
promoted before becoming durable graph edges.

### 12. CogniCode's graph evolves through promoted RelationCandidates

Queries, scaffolds, ViewSpecs, investigations, and LLM analysis may produce
`RelationCandidate`s. They do not mutate the durable graph automatically.

Only explicit promotion creates a durable `GraphEdge` for semantic links.
Promotion records provenance:

- source;
- source query;
- source scaffold id;
- source ViewSpec id;
- source investigation id;
- evidence object ids;
- confidence;
- promoter;
- promotion timestamp.

In v1, semantic candidates are never auto-promoted. Deterministic structural
edges may still be created by ingest.

### 13. Lepiter-lite starts with composed narratives and MoldQL snippets

`ProjectDiary` and `ComposedNarrative` are the Lepiter-lite direction for
CogniCode. They are markdown narratives with embedded:

- ViewSpecs;
- inspectable objects;
- graph snapshots;
- evidence packs;
- relation candidates;
- code references;
- artifacts;
- decision traces.

When evaluable snippets are introduced, the first snippet type is MoldQL, not
arbitrary JavaScript, Rust, or shell. MoldQL snippets can render inspectable
results, produce RelationCandidates, and be saved as ViewSpecs.

## Alternatives considered

### Adopt Cypher/GQL as the product language

Cypher and GQL are mature property-graph languages and provide strong pattern
matching semantics. We rejected adopting them as the primary product language
because CogniCode needs software-object semantics, ViewKind intents,
inspection, relation candidates, and narrative integration. Those concepts are
not first-class in generic graph languages.

MoldQL may borrow graph-query concepts and may lower to graph backends in the
future, but users should not have to learn a second graph database language.

### Adopt Gremlin as the product language

Gremlin is powerful for graph traversal, but its imperative/traversal style is
too technical for the primary CogniCode UX. It also risks becoming a scripting
surface rather than a safe exploration language. We rejected it for the product
surface.

### Keep ViewSpec authoring as a blank MoldQL textarea

This is simple to implement but poor moldable-development UX. GToolkit-like
tools offer contextual affordances. CogniCode should start from object-aware
scaffolds and let users edit the generated MoldQL when needed.

### Allow queries to mutate the graph directly

This would make the graph evolve quickly, but it would also contaminate durable
knowledge with unreviewed exploratory output. We rejected automatic mutation.
Graph evolution happens through RelationCandidates and explicit promotion.

### Store scaffolds only in frontend code

This would be fast, but it would create divergent behavior across the wizard,
Spotter, SuggestionStrip, narratives, and MCP. We rejected frontend-only
scaffolds in favor of a shared YAML/JSON registry validated by Rust.

## Consequences

### Positive

- CogniCode gets a coherent moldable-development model across search, query,
  views, narratives, and graph enrichment.
- Users get contextual affordances instead of blank query surfaces.
- The graph can improve over time while preserving trust and provenance.
- Rust remains a good fit: MoldQL compiles to typed AST/query plans instead
  of executing arbitrary scripts.
- Future MCP and AI integrations can reuse the same scaffold and object
  catalog semantics as the Explorer UI.

### Negative

- The shared registry and `applies_when` model add upfront schema and
  validation work.
- The current code still exposes internal `ExplorerQL` terminology in places;
  docs and UI must converge on MoldQL.
- Predicate evaluation for `applies_when` requires a new safe evaluator over
  inspectable object properties.
- RelationCandidate review/promotion adds UI and persistence surface area.

### Mitigations

- Ship `applies_when` in two phases: persist/display first, evaluate later.
- Ship scaffold coverage in waves, starting with object types already close to
  implementation.
- Keep MoldQL execution deterministic and typed; do not add arbitrary code
  snippets until sandboxing is designed explicitly.
- Keep RelationCandidate promotion manual for semantic links in v1.

## Implementation order

The first implementation slice should prioritize UX and metadata before large
grammar expansion:

1. Add ViewSpec origin metadata: `seed_object_id`, `seed_view_id`.
2. Add `applies_when` to DTO/schema/store as persisted/displayed metadata.
3. Add `crates/cognicode-explorer/assets/moldql-scaffolds.yaml`.
4. Validate scaffold registry in Rust and expose generated/validated types to
   TypeScript.
5. Rework ViewSpecWizard to be scaffold-first.
6. Make SuggestionStrip and Spotter intent actions reference scaffold ids.
7. Add RelationCandidate metadata to candidate-producing scaffolds.
8. Later, extend the MoldQL parser with intent-first syntax.

## References

- ADR-002: Moldable exploration parity program
- ADR-003: Diagram representations
- ADR-004: C4 investigation model
- ADR-005: Investigation mode
- `CONTEXT.md` glossary updates from the 2026-07-02 grill-with-docs session
- Engram observations: #3804-#3845

## Implementation Log

- **2026-08-10 (E31-C)**: Vision-level parity document. Subsumed by ADR-031 (Release 1.0.0 definition) which codifies the practical acceptance criteria for production-ready moldable tooling.
