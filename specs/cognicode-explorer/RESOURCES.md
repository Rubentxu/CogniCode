# Visual Software Understanding Resources

## Knowledge

- [Graphify — safishamsi/graphify](https://github.com/safishamsi/graphify)
  Tool for turning code, schemas, docs, and other artifacts into a queryable knowledge graph with HTML visualization, graph JSON, reports, queries, paths, explanations, and Mermaid call-flow export. Use for: repository evidence extraction and graph-backed questions.
- [Model Context Protocol — Server Concepts](https://modelcontextprotocol.io/docs/learn/server-concepts)
  Defines MCP servers as providers of tools, resources, and prompts. Use for: designing the capability boundary between visualizer, graph backend, and AI agents.
- [Model Context Protocol — Client Concepts](https://modelcontextprotocol.io/docs/learn/client-concepts)
  Explains roots, elicitation, and sampling. Use for: workspace scoping, human-in-the-loop questions, and agentic workflows controlled by the host application.
- [Model Context Protocol — Architecture](https://modelcontextprotocol.io/specification/2025-06-18/basic)
  Describes MCP's host-client-server architecture, lifecycle, capability negotiation, and modular protocol design. Use for: separating application shell, MCP clients, and analysis servers.
- [Codebase-Memory: Tree-Sitter-Based Knowledge Graphs for LLM Code Exploration via MCP](https://arxiv.org/html/2603.27277v1)
  Research-style system showing codebase knowledge graphs exposed through MCP, with structural queries, impact analysis, and lower token/tool usage. Use for: validating graph-first code exploration.
- [GitCortex](https://github.com/bharath03-a/gitcortex)
  Branch-aware code knowledge graph with MCP, visual graph, branch diff overlay, blast-radius queries, and prompts. Use for: product ideas around branch-aware visual developer workflows.
- [CodeBoarding](https://github.com/codeboarding/codeboarding)
  Interactive architecture diagrams for codebases using static analysis and LLM reasoning, with Mermaid/docs outputs. Use for: comparison against generated architecture documentation products.
- [Kuzu](https://kuzudb.com/)
  Embedded property graph database with Cypher, columnar storage, vectorized/factorized execution, full-text/vector search, and Rust bindings. Use for: evaluating a high-performance graph backend for local developer tooling.
- [SurrealDB Graph Traversal](https://surrealdb.com/docs/learn/data-models/graph/graph-traversal)
  Shows readable arrow-based graph traversal syntax over records and relations. Use for: inspiration for developer-friendly path navigation syntax.
- [Apache DataFusion](https://datafusion.apache.org/)
  Extensible Rust query engine over Apache Arrow with custom logical/physical plan support. Use for: analytics, tabular aggregation, and custom query engine architecture ideas.
- [Souffle Datalog Tutorial](https://souffle-lang.github.io/tutorial)
  Datalog language and engine for recursive program analysis. Use for: derived facts, static-analysis rules, and declarative coupling/design checks.
- [Differential Dataflow](https://github.com/TimelyDataflow/differential-dataflow/)
  Rust data-parallel incremental computation framework. Use for: future incremental recomputation of graph-derived lenses when repository facts change.
- [Contextual Search — Glamorous Toolkit Book](https://book.gtoolkit.com/contextual-search-98p05389q8o71a8fh6zcaedac)
  Explains custom contextual searches over domain objects. Use for: designing search as scope-aware navigation rather than global text lookup.
- [Moldable Tool — Glamorous Toolkit Book](https://book.gtoolkit.com/moldable-tool-98p0537dgjllrko3jqmdk8243)
  Describes moldable tools reacting to moldable objects with contextual views, actions, searches, and playgrounds. Use for: designing the visual application's core interaction model.
- [Contextual View — Glamorous Toolkit Book](https://book.gtoolkit.com/contextual-view-9jkob0898tctthh5o3m23fv4e)
  Explains lifting repeated navigation steps into direct contextual views. Use for: reducing developer effort in repeated codebase exploration tasks.
- [Contextual Action — Glamorous Toolkit Book](https://book.gtoolkit.com/contextual-action-7ed0nuwfqc5qp8b1b257cavgy)
  Explains packaging repeated navigation or operations as object-specific buttons. Use for: designing one-click actions such as export, inspect impact, open in IDE, or record decision.
- [The Eyes Have It — Ben Shneiderman](https://www.cs.umd.edu/~ben/papers/Shneiderman1996eyes.pdf)
  Introduces the visual information-seeking mantra: overview first, zoom and filter, then details-on-demand. Use for: structuring the exploration flow.
- [Interactive Diagrams for Software Documentation](https://arxiv.org/html/2407.21621v1)
  Explores interactive node-link diagrams as generated code documentation with filtering and details-on-demand. Use for: validating interactive diagrams as a primary navigation surface.
- [Facilitating Program Comprehension with Call Graph Multilevel Hierarchical Abstractions](https://www.sciencedirect.com/science/article/pii/S016412122100042X)
  Presents multi-level call graph abstractions across packages, classes, and functions. Use for: zooming between scopes without losing architectural context.
- [DevLens](https://devlens.io/)
  Product reference for interactive codebase maps, blast radius, AI summaries, PR impact, and graph-backed code questions. Use for: competitive UX patterns.
- [CodeSee](https://www.codesee.io/)
  Product reference for code maps, dependency visibility, visual PR review, onboarding, and shareable code knowledge views. Use for: workflow and value proposition comparison.
- [GitNexus](https://github.com/shanglt/GitNexus)
  Code knowledge graph with web UI, MCP, contextual resources, impact analysis, process traces, and agent integration. Use for: local-first graph explorer plus MCP inspiration.
- [A Philosophy of Software Design: My Take — The Pragmatic Engineer](https://blog.pragmaticengineer.com/a-philosophy-of-software-design-review/)
  Review and summary of John Ousterhout's design ideas: fighting complexity, deep modules, information hiding, tactical vs strategic programming, designing twice, and layers that remove complexity. Use for: criteria that turn visualizations into design decisions.
- [Moldable Development](https://moldabledevelopment.com/)
  Method and ecosystem around creating custom views and tools adapted to the object or problem being inspected. Use for: thinking beyond fixed diagrams toward context-specific developer views.
- [How to get started with Moldable Development — Glamorous Toolkit Book](https://book.gtoolkit.com/how-to-get-started-with-moldable-developme-lor2271nbpvdqfx46d0prlk2)
  Explains contextual views, contextual actions, contextual searches, and live objects. Use for: designing navigable developer inspectors.
- [Moldable Development Patterns — Glamorous Toolkit Book](https://book.gtoolkit.com/moldable-development-patterns-vuflnrgp5r5o4m1szatoo4e2)
  Pattern language for explainable systems, moldable objects, contextual views, contextual actions, contextual searches, blind spots, and throwaway analysis tools. Use for: structuring the visualizer workflow.
- [connascence.io](https://connascence.io/)
  Defines connascence as a coupling vocabulary measured by strength, degree, and locality. Use for: identifying coupling hotspots and prioritizing refactors.
- [Using Connascence to Understand Coupling — Andy Hansen](https://andyhansen.co.nz/posts/understanding-coupling-with-connascence)
  Practical explanation of static and dynamic connascence types, including name, type, meaning, position, algorithm, execution, timing, value, and identity. Use for: mapping code relationships into visual diagnostic lenses.
- [The Principles of OOD — Robert C. Martin](http://www.butunclebob.com/ArticleS.UncleBob.PrinciplesOfOod)
  Source summary of SOLID and package dependency principles as dependency management tools. Use for: interpreting class/module/package design risks.
- [DIP in the Wild — Martin Fowler site](https://martinfowler.com/articles/dipInTheWild.html)
  Production-oriented discussion of Dependency Inversion as depending on domain-level abstractions rather than low-level details. Use for: visualizing dependency direction and abstraction level.
- [Cognitive Diagram Understanding and Task Performance in Systems Analysis and Design — MIS Quarterly](https://research.wu.ac.at/de/publications/cognitive-diagram-understanding-and-task-performance-in-systems-a-4/)
  Research on how diagrams support task performance when relevant information is preserved for a specific goal. Use for: grounding visual thinking in task-fit and cognitive load.
- [Mermaid Documentation](https://mermaid.js.org/)
  Text-based diagramming language for flowcharts, sequence diagrams, class diagrams, state diagrams, C4-style diagrams, and more. Use for: lightweight diagrams embedded in Markdown and generated documentation.
- [PlantUML Documentation](https://plantuml.com/)
  Text-based diagramming tool supporting many UML and non-UML diagram types. Use for: more formal or varied diagram generation where PlantUML support already exists.
- [The C4 Model](https://c4model.com/)
  Architecture diagramming model focused on Context, Containers, Components, and Code. Use for: explaining software architecture at different zoom levels.

## Wisdom (Communities)

- [Graphify GitHub Issues](https://github.com/safishamsi/graphify/issues)
  Use for: checking real user problems, limitations, and integration patterns around Graphify.
- [Mermaid GitHub Discussions](https://github.com/mermaid-js/mermaid/discussions)
  Use for: practical diagram DSL questions and rendering constraints.
- [PlantUML Forum](https://forum.plantuml.net/)
  Use for: syntax, rendering, and modeling tradeoff questions.

## Gaps

- Need a concrete example repository to run through the workflow.
