# Tasks: e12h-decision-trace — DocsExtractor Hardening

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 150–250 |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Chain strategy | size-exception |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

## Phase 1: Foundation

- [ ] 1.1 Create branch `feat/e12h-decision-trace` from `main` (Phase 1.8 trigger).
- [ ] 1.2 Baseline: `cargo test -p cognicode-core --features multimodal` is green; capture current test count.

## Phase 2: Bold-Markdown Status Extraction

- [ ] 2.1 RED: In `crates/cognicode-core/src/infrastructure/extraction/docs_extractor.rs` test mod, add 4 failing tests — `**Status**: ACCEPTED`, `_Status_: proposed`, `*Status*: superseded`, and `**Status**: Superseded` (lowercased).
- [ ] 2.2 GREEN: Refactor `extract_status()` (line ~591) — strip leading `*`/`_` markers via `trim_matches(['*', '_'])` BEFORE the lowercase prefix check; lowercase the value; trim whitespace.
- [ ] 2.3 REFACTOR: Extract a small `strip_md_marker()` helper if duplication emerges; keep plain `Status:` path backwards-compatible.

## Phase 3: Cross-Document ADR Citations

- [ ] 3.1 RED: Add 4 failing tests — `[ADR-002](./ADR-002-moldable-exploration-parity-program.md)` → `Cites` to `decision:adr-002-moldable-exploration-parity-program#<heading>` conf 0.9; `[Architecture](./architecture.md#context)` → `doc:architecture#context` conf 0.9; `[spec](https://example.com)` → no edge; `[bar](src/foo.rs:bar:1)` → symbol-shape preserved.
- [ ] 3.2 GREEN: Add `classify_doc_link()` returning a `BodyCites`-shaped candidate with target `decision:<stem>#<heading>` or `doc:<stem>#<anchor>`, tier `ConfidenceTier::LinkExact` (0.9, `Extracted`); wire into `flush_trailing_body` (line ~124) and the heading-end edge loop (line ~161) of `parse_markdown`.
- [ ] 3.3 REFACTOR: Share the `[text](target)` parser between `classify_body_line` and `classify_doc_link` to avoid duplication.

## Phase 4: Decision→Doc Justifies Edge

- [ ] 4.1 RED: Add test — per ADR file exactly one `Justifies` edge (confidence 1.0, `Extracted`) from `decision:<stem>#<heading>` → `doc:<stem>#<heading>`; plain markdown emits zero.
- [ ] 4.2 GREEN: After the parse loop in `parse_markdown`, when `is_adr && nodes.len() >= 2`, find the first `NodeKind::Decision` id and the file-level `NodeKind::Doc` id; push one `GraphEdge::new(decision_id, doc_id, EdgeKind::Justifies, Provenance::Extracted, 1.0)` onto the Decision's `potential_edges`.

## Phase 5: Spec Reconciliation

- [ ] 5.1 Edit `openspec/specs/docs-source-adapter/spec.md`: drop YAML front-matter requirement; clarify `Justifies` is emitted at confidence 1.0; add bold-markdown status scenarios.
- [ ] 5.2 Edit `openspec/changes/e12h-decision-trace/spec.md` `REMOVED Requirements` block: confirm rationale cites corpus actual format (`**Status**:` in `docs/adr/ADR-001..008.md`).

## Phase 6: Corpus Regression Fixture

- [ ] 6.1 Add `docs_extractor_corpus_regression` test — read all 8 files under `docs/adr/ADR-00*.md`; assert exactly 8 `Decision` nodes each with non-empty `metadata.status`; assert ADR-005 emits ≥3 cross-ADR `Cites` edges to `decision:adr-NNN-*`; assert total `Justifies` edges equals 8.
- [ ] 6.2 Add edge-case test — `**Status**: DRAFT-WIP` lowercases to `draft-wip` without panic (covers "Unknown status does not crash" scenario).

## Phase 7: Cleanup & Verification

- [ ] 7.1 Run `cargo fmt --workspace && cargo clippy -p cognicode-core --features multimodal -- -D warnings && cargo test --workspace`; all green, no regressions.
- [ ] 7.2 Conventional commit: `feat(docs-extractor): accept bold-markdown status, emit cross-ADR cites + Justifies edges` with body summarising the 5 spec changes.