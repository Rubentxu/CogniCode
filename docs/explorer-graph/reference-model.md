# Reference Model: What We Are Borrowing and Why

This file distills the concepts we are pulling in from three external
influences - Graphify, gbrain, and the Glamorous Toolkit - into a set of
borrowable ideas. For each idea, it states what we are taking, why it matters
for developer utility, and where we are explicitly choosing not to copy.

The goal is not to clone any of these systems. The goal is to learn from
their shape and apply what fits CogniCode's domain: understanding code at
scale, for both humans and agents.

## Graphify

Graphify is a research-grade pipeline that ingests code, documentation, and
multimedia, extracts a persistent graph, and serves question-oriented
commands on top of it. The relevant ideas are about the pipeline shape and
the outputs, not the implementation.

### Borrowable ideas

| Idea from Graphify                              | Why it matters for CogniCode                                 |
| ----------------------------------------------- | ------------------------------------------------------------ |
| Multimodal ingest (code, docs, images, audio)   | The "why" of code is rarely in the code. ADRs, PRs, runbooks, |
|                                                 | and screenshots carry decisions and rationale that pure     |
|                                                 | static analysis cannot reach.                                |
| Deterministic AST and semantic extraction       | Repeatability. A graph that changes only when the world     |
|                                                 | changes is one users can trust and diff.                     |
| Persistent graph outputs (`graph.json`,         | A graph that can be stored, versioned, and shared is the    |
| `graph.html`, report)                           | difference between a tool and a product.                     |
| Provenance on edges: extracted, inferred,        | Users stop trusting black-box links. They can audit and      |
| ambiguous                                       | challenge them.                                              |
| Confidence on edges                             | Lets the UI rank strong links first and lets the user       |
|                                                 | filter or fade weak ones.                                    |
| Topological clustering (Leiden, community       | Surfaces architectural seams and god modules without the    |
| detection)                                      | user having to know the structure in advance.                |
| Question-oriented commands: query, path,        | Matches how developers actually think. "What connects these |
| explain, watch, update                          | two things?" is the unit of work, not "give me all callers".|
| Graph report: god nodes, surprising             | A first run on a new codebase should produce insight, not   |
| connections, suggested questions                | just data.                                                   |

### Do not copy blindly

Graphify is a research pipeline. Its scaling characteristics, its dependency
on large language models for inference, and its single-user, single-run
shape are not what we want. CogniCode is a long-lived workspace, used by
teams, with a deterministic core. We borrow the idea of a persistent,
question-oriented graph, not the pipeline's runtime assumptions.

## gbrain

gbrain's central move is to separate "source" from "brain". A brain is a
queryable model that sits on top of one or more sources, with explicit
routing and a hybrid retriever that can explain its ranking.

### Borrowable ideas

| Idea from gbrain                          | Why it matters for CogniCode                                |
| ----------------------------------------- | ----------------------------------------------------------- |
| Brain and source as separate axes         | Lets us federate multiple repositories, language adapters,  |
|                                           | and decision logs without baking them into a single blob.   |
| Hybrid retrieval with explainable ranking | "Why did this node surface?" is a first-class question.     |
|                                           | The UI and the agent both need that answer.                 |
| Graph signals (backlinks, corroboration,   | A symbol that is reached by many paths, cited in many       |
| source-aware routing)                     | ADRs, and flagged in code review is a more confident lead   |
|                                           | than one that is only topologically central.                |
| Multi-source and multi-brain federation   | Engineering reality is multi-repo. The graph should follow. |
| Thin CLI, rich model                      | The surface is small. The model underneath is what scales.  |

### Do not copy blindly

gbrain's brain model is general-purpose across arbitrary content. CogniCode
is code-shaped. We want the discipline of the source/brain separation, but
the kinds of sources we federate are narrower (code, ADRs, PRs, docs), and
the retriever should be tuned to the structure of code, not to free-form
text. We will not adopt every retrieval trick gbrain uses; we will adopt
the discipline of ranking with explanation.

## Glamorous Toolkit (GT)

GT is less a tool than a philosophy: a moldable system is one where every
object can be queried, viewed, and acted upon in context. The view is not
fixed; the user (or the tool) shapes it to the question.

### Borrowable ideas

| Idea from GT                                | Why it matters for CogniCode                                |
| ------------------------------------------- | ----------------------------------------------------------- |
| Start from high-value objects, not files    | A user who lands on a file is already lost in the maze.     |
|                                             | A user who lands on a question or a hot symbol is oriented. |
| Contextual views and actions                | The same node can be viewed as a hotspot, a dependency, a   |
|                                             | refactor candidate, or a documentation anchor. The model    |
|                                             | is the same; the lens changes.                              |
| Progressive drill-down and climb-up across  | Lets a user move from "what does this symbol do?" to "what  |
| abstraction levels                          | system does it belong to?" without losing context.           |
| C4-style navigation via level projections   | Code, component, container, system. Each level answers a    |
|                                             | different class of question, and the user moves between     |
|                                             | them deliberately.                                          |
| "What can I do here?" as a first-class      | Reduces the cost of entering the tool for new users and    |
| surface                                     | for agents that have to decide their next move.            |

### Do not copy blindly

GT is built on Pharo and a deeply dynamic object model. We are in Rust, with
a static type system and a web front end. The moldable philosophy survives
the translation; the literal mechanisms (GT's examples, custom smalltalk
scripts) do not. We will express the same ideas through lenses, ExplorerQL
queries (evolved from MoldQL), and named views. The language and grammar
decisions are in `query-language-decision.md`; the visualization library
decisions that frame those queries are in `visualization-stack.md`.

## Synthesis: What CogniCode Keeps

Across the three influences, the recurring shape is the same:

- A persistent graph is the artifact.
- Edges carry provenance and confidence.
- Communities and levels structure the graph.
- Questions are the user-facing verbs.
- The UI is a set of contextual views, not a single dashboard.
- A brain or a workspace federates multiple sources.

The rest of this documentation set operationalizes that shape for CogniCode.
