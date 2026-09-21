# Delta Spec for docs-source-adapter

## Purpose
Harden `DocsExtractor` ADR parsing for the real corpus (bold-markdown `**Status**:`), emit cross-ADR `Cites` edges, and reconcile spec/impl drift on `EdgeKind::Justifies` and front-matter detection.

## MODIFIED Requirements

### Requirement: DocsExtractor Markdown Parsing

`DocsExtractor::extract(source)` MUST:
1. Read the file (or git blob) as UTF-8.
2. Detect ADR markers (`# ADR-NNNN:`, `# Decision: NNNN`) in headings — YAML front-matter is NOT required.
3. Parse Markdown for H1/H2 headings, links `[text](url)`, code fences, inline backticks.
4. For each heading → emit a `GraphNode { kind: NodeKind::Doc, label, source_path, metadata: { section, line } }`.
5. For each link → resolve via `DocsConfidenceRules` → emit a `GraphEdge { kind: EdgeKind::Cites }`.
6. For an ADR heading → emit `GraphNode { kind: NodeKind::Decision, metadata: { status, date } }` parsed from the body using bold/italic/plain markdown forms.
7. Return `Vec<ExtractedNode>` — one per top-level node.

(Previously: mandated YAML front-matter and unconditional `EdgeKind::Justifies` — both were spec drift; corpus uses bold-markdown and `Justifies` was never emitted.)

#### Scenario: Plain markdown produces Doc nodes
- GIVEN `docs/guide.md` containing `# Guide\nSee [render](src/render.rs#L10)`
- WHEN extracted
- THEN one `GraphNode` with `kind == NodeKind::Doc` is produced
- AND one `GraphEdge` with `kind == EdgeKind::Cites` targets the resolved `Symbol`

#### Scenario: ADR heading produces Decision node from bold status
- GIVEN `docs/adr/0007.md` with `# ADR-0007: Adopt GraphQL\n\n**Status**: ACCEPTED\n…`
- WHEN extracted
- THEN a `GraphNode { kind: NodeKind::Decision, metadata: { status: "accepted" } }` is produced (no YAML front-matter required)

#### Scenario: Code fence with language is preserved
- GIVEN a ` ```rust\nfn main(){}\n``` ` code block in a doc
- WHEN extracted
- THEN the code block's language is recorded in the Doc node's metadata (`code_block_lang: "rust"`)

## ADDED Requirements

### Requirement: Bold-Markdown Status Extraction

ADR `Status:` extraction MUST accept `**Status**: V`, `_Status_: V` / `*Status*: V`, and plain `Status: V`. The value MUST be trimmed and lowercased before storage.

#### Scenario: Bold and italic status are captured
- GIVEN an ADR body containing `**Status**: ACCEPTED` OR `_Status_: proposed`
- WHEN parsed
- THEN the Decision node has `metadata.status == "accepted"` or `"proposed"` respectively

#### Scenario: Missing status leaves no property
- GIVEN an ADR body with no `Status:` line in any form
- WHEN parsed
- THEN no `status` property is set AND extraction succeeds without error

#### Scenario: Mixed-case value is normalised
- GIVEN an ADR body containing `**Status**: Superseded`
- WHEN parsed
- THEN the Decision node has `metadata.status == "superseded"`

### Requirement: Cross-Document ADR Citations

When a body line is a Markdown link resolving to another ingested `.md` file, the extractor MUST emit a `GraphEdge { kind: EdgeKind::Cites, target: decision:<stem>#<heading> | doc:<stem>#<anchor>, confidence: 0.9, provenance: Extracted }`.

#### Scenario: Cross-ADR link emits Decision→Decision Cites
- GIVEN an ADR body containing `[ADR-002](./ADR-002-moldable-exploration-parity-program.md)`
- WHEN parsed
- THEN a `Cites` edge is emitted with target `decision:adr-002-moldable-exploration-parity-program#adr-002-moldable-exploration-parity-program`, `confidence == 0.9`

#### Scenario: Cross-doc link emits Doc→Doc Cites with anchor
- GIVEN a doc body containing `[Architecture](./architecture.md#context)`
- WHEN parsed
- THEN a `Cites` edge is emitted with target `doc:architecture#context`, `confidence == 0.9`

#### Scenario: Symbol-shaped link still resolves
- GIVEN a body line containing `[bar](src/foo.rs:bar:1)`
- WHEN parsed
- THEN a `Cites` edge targets `src/foo.rs:bar:1` (no regression)

#### Scenario: External URL is skipped
- GIVEN `[spec](https://example.com/spec.md)`
- WHEN parsed
- THEN no `Cites` edge is emitted

### Requirement: Decision→Doc Justifies Edge

For every ingested ADR file, the extractor MUST emit one `GraphEdge { kind: EdgeKind::Justifies, source: decision:<stem>#<heading>, target: doc:<stem>#<heading>, confidence: 1.0, provenance: Extracted }`.

#### Scenario: Decision justifies its document
- GIVEN an ADR file producing one `Decision` node AND one file-level `Doc` node
- WHEN extracted
- THEN exactly one `Justifies` edge (confidence 1.0) connects Decision→Doc

#### Scenario: Justifies absent on plain markdown
- GIVEN `docs/guide.md` with no ADR marker
- WHEN extracted
- THEN no `Justifies` edge is emitted

### Requirement: Corpus Regression Coverage

The test suite MUST include a fixture that ingests `docs/adr/ADR-001..ADR-008.md` and asserts: all 8 ADRs produce a `Decision` node with non-empty `metadata.status`; ADR-005's `## References` produces ≥3 cross-ADR `Cites` edges; total `Justifies` edges equals total ADRs.

#### Scenario: Full corpus parses
- GIVEN `docs/adr/` contains ADR-001 through ADR-008
- WHEN the corpus regression test runs
- THEN 8 Decision nodes are emitted with non-empty status values AND ≥3 cross-ADR Cites edges come from ADR-005

#### Scenario: Unknown status does not crash
- GIVEN an ADR body containing `**Status**: DRAFT-WIP`
- WHEN parsed
- THEN the status is stored verbatim (lowercased) AND no panic occurs

## REMOVED Requirements

### Requirement: ADR Front-matter Detection (YAML)

(Reason: the real corpus (`docs/adr/ADR-001..008.md`) uses bold-markdown `**Status**:` format, not YAML front-matter. All 8 ADRs in the corpus have `**Status**: ACCEPTED` or `**Status**: SUPERSEDED` in the first paragraph after the H1 heading. Implementation now accepts bold/italic/plain status forms as specified in the ADDED requirements above. YAML front-matter detection is no longer a contractual requirement.)
