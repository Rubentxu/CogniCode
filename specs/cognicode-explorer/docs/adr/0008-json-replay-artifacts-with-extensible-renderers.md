# JSON Replay Artifacts With Extensible Renderers

CogniCode Explorer will persist decision artifacts as canonical JSON replay files. Markdown is the first human-readable renderer generated from that replay, while HTML reports, replayable queries, evidence tables, Mermaid/PlantUML/C4 diagrams, and other output forms can be added later as artifact renderer implementations. This keeps artifacts reproducible, testable, versionable, and extensible: renderers can evolve without making Markdown, HTML, or any diagram format the source of truth.
