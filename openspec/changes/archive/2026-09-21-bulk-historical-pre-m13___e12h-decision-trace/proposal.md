# Proposal: e12h — Decision Trace Hardening

## Change ID: `e12h-decision-trace`

## Intent

`DocsExtractor` already emits `NodeKind::Decision` and `EdgeKind::Cites` for ADRs, but auditing it against the real 7-ADR corpus surfaces three defects that break decision archaeology (ADR → Code → Tests → Docs → Issues → Evidence): status is silently dropped, cross-ADR references produce no edges, and the spec describes frontmatter and `EdgeKind::Justifies` that the impl never shipped.

## Scope

### In Scope
- Fix `extract_status()` to accept `**Status**:`, `_Status_:`, `*Status*:` (bold/italic markdown)
- Extend link classification so `[ADR-002](./ADR-002-*.md)` emits a `Cites` Decision→Decision edge
- Reconcile spec: drop frontmatter requirement, clarify `Justifies` fate
- Add corpus-regression tests against the real 7 ADRs

### Out of Scope
- `DecisionTraceViewKind` executor (separate proposal)
- MCP `docs_ingest` / CLI `cognicode ingest-docs` surfaces
- ADR CRUD, frontmatter migration

## Capabilities

> CONTRACT with sddk-spec. Researched against `openspec/specs/`.

### New Capabilities
- None

### Modified Capabilities
- `docs-source-adapter`: (a) Status/Date extraction MUST accept bold-markdown form; (b) Cross-document `.md` links MUST emit `Cites` between Decision/Doc nodes; (c) Frontmatter requirement DROPPED (real corpus uses bold markdown); (d) `Justifies` for the Decision→Doc link either implemented or removed from spec.

## Approach

1. **Bold-status fix**: pre-process body lines through marker stripping (`trim_matches(['*', '_'])`) before the `status:` prefix check. Red tests for `**Status**: ACCEPTED`, `_Status_: proposed`, `*Status*: superseded`.
2. **Cross-doc citations**: add `classify_doc_link()` parallel to `classify_body_line()`. Resolve `.md` link targets to `decision:<stem>#<heading>` NodeIds; tier `LinkExact` (0.9).
3. **`Justifies` decision**: implement for the file-level Decision→Doc link (recommended — preserves provenance the spec mandated) or strike from spec.
4. **Corpus regression fixture**: read `docs/adr/*.md` directly; assert 7 Decision nodes, ≥3 cross-ADR edges from ADR-005's References, status property on all 7.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/infrastructure/extraction/docs_extractor.rs` | Modified | Bold-status fix; new `classify_doc_link()`; corpus tests |
| `openspec/specs/docs-source-adapter/spec.md` | Modified | Drop frontmatter; clarify Justifies; add bold-markdown scenarios |
| `crates/cognicode-core/src/infrastructure/extraction/docs_confidence_rules.rs` | Modified | Add `doc_link_exact` tier if needed for cross-ADR confidence |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Extending `classify_body_line()` regresses symbol-shaped matches | Medium | Add new classifier alongside; parity tests before any deprecation |
| Dropping `Justifies` loses Decision→Doc provenance the spec mandated | Medium | Implement `Justifies` for file-level link; keep `Cites` for body refs |
| Future ADRs adopt frontmatter | Low | Spec allows both formats; parser auto-detects frontmatter if present |

## Rollback Plan

Revert `docs_extractor.rs`, `docs_confidence_rules.rs`, and `openspec/specs/docs-source-adapter/spec.md` to the pre-change commit. No DB migrations — `Decision` nodes and `Cites` edges already have stable schemas. Re-ingest required to repopulate corrected `status` properties on existing rows.

## Dependencies

- Existing `multimodal` feature gate (no feature-flag changes)
- Existing `EdgeKind::Justifies` variant (already in enum, unused)
- No new external crates

## Success Criteria

- [ ] All 7 real ADRs produce `Decision` nodes with non-empty `status` property
- [ ] ADR-005 emits ≥3 cross-ADR `Cites` edges (to ADR-002, ADR-003, ADR-004)
- [ ] Corpus-regression test (`docs_extractor_corpus`) is green
- [ ] No existing `docs_extractor` unit test regresses
- [ ] `cargo test -p cognicode-core --features multimodal` is green
- [ ] Updated `docs-source-adapter/spec.md` reflects the actual ADR format used in the repo
