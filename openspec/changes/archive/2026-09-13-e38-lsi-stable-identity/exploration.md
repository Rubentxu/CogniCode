# Exploration — e38-lsi-stable-identity (LSI M3: Stable Identity & Temporal Snapshots)

## Current State (identity map, file:line)

- `crates/cognicode-core/src/domain/evidence_kernel/ids.rs:23,50,80` — `EntityId(u64)`, `OccurrenceId(u64)` (defined but **never used** outside the `mod.rs:48` re-export — free slot), `SnapshotId` bijective with `RevisionId`.
- `crates/cognicode-core/src/application/fact_bridge/entity_table.rs:23-31` — `EntityIdTable::build`: snapshot-scoped, sorted unique id strings → `EntityId(1..N)`, no hashing (e37 D3 determinism).
- `crates/cognicode-core/src/application/fact_bridge/batch_builder.rs:119-160` — `finish()` sorts records by (subject, predicate, object, detail, producer) then assigns ids; fact sets are byte-identical per snapshot.
- Identity convention pinned: `crates/cognicode-core/tests/equivalence_harness/harness.rs:85-91` — `IDENTITY_CONVENTION` text + `PINNED_IDENTITY_DIGEST = "fnv1a64:efccc22e912913fe"`; changing convention text fails the harness until explicit re-pin.
- e37 D3 (openspec/changes/e37-lsi-projection-bridge/design.md:15): subject id strings are legacy FQNs `"{file}:{name}:{line}"`; files their path; kind rides `core:defines` `provenance.detail`; references stay `FactValue::Text`.
- Store keying: `crates/cognicode-core/src/infrastructure/evidence_kernel/in_memory.rs:82` — `HashMap<(WorkspaceId, SnapshotId), Vec<Fact>>`; read port `facts_in_snapshot` (`domain/evidence_kernel/ports.rs:89`).
- `Fact` (`domain/evidence_kernel/fact.rs:108-121`): `{id, subject: EntityId, predicate, object, snapshot, provenance: ProvenanceRecord}` — has snapshot pin + provenance, **no validity intervals** (grep over all umbrella specs for `valid_from/valid_to/interval`: zero hits; M3 spec does not require them).
- `SnapshotDescriptor` (`domain/evidence_kernel/snapshot.rs:26-40`): `source_state`/`config_digest` reserved, may be empty; `RevisionStore` (`domain/ports/revision_store.rs:32`) is a monotonic counter + head pointer, carrying **no parent/diff/git-SHA** info.
- Critical consequence: because the FQN embeds `line`, **any line shift changes the raw id string** — today's keying alone fails UAT-U20; identity must come from a continuity layer, not raw FQN equality.

## What M3 must satisfy (the contract)

Umbrella `specs/stable-entity-identity/spec.md`: (R1) logical Entity identity modeled separately from per-snapshot Occurrence location. S1 line-shift → new Occurrence references same EntityId. S2 ambiguous continuity (margin not exceeded) → `Ambiguous`, no forced merge. ADR-038: matcher with evidence; calibrated rename heuristic; NEVER force ambiguous match; rollback keeps legacy key as occurrence metadata. ADR-039: `SnapshotDescriptor` gains source_state/config_digest (one git commit may yield several snapshots). UAT-U20 (line shift keeps EntityId, 100%), UAT-U21 (file move keeps Entity, ≥99%), UAT-U22 (ambiguous rename returns ambiguous/unknown). Umbrella tasks 4.1–4.5. e38 implements this spec — do not duplicate; delta spec only pins terminology + e38-specific behavior.

## Approaches for reconciling logical EntityId vs e37's snapshot-scoped EntityId

1. **A — EntityId becomes persistent-logical** (assigned once, carried across snapshots by the matcher; fact identity pins move to (EntityId, OccurrenceId) pairs)
   - Pros: literally matches spec wording; one id namespace.
   - Cons: snapshot N+1 fact bytes become history-dependent (replaying an isolated snapshot changes ids — breaks the byte-stability discipline of e36/e37); `PINNED_IDENTITY_DIGEST` + all e37 goldens + `from_facts` harness must re-pin; `FactBatchBuilder` needs a prior-snapshot lookup, coupling the bridge to the store; `InMemoryFactStore`/future PG schema need an entity table. Effort: **High**, risk: high.
2. **B — Separate stable-identity layer on top** (recommended): keep e37's snapshot-scoped `EntityId` as the per-snapshot occurrence key; ADD `StableEntityId` (new newtype in `ids.rs`) + a continuity module (domain) mapping `(ws, snap, EntityId)` → `(StableEntityId, status, evidence, confidence)`; `OccurrenceId` = the snapshot-scoped `EntityId` value (1:1 with subjects per snapshot, trivially deterministic).
   - Pros: zero change to facts, batch builder, goldens, `from_facts` harness, store schemas (facts stay keyed `(ws, snap)`); e37 `IDENTITY_CONVENTION` text untouched → digest survives; continuity output is a derived artifact, cleanly rollbackable (ADR-038 rollback = "legacy key as occurrence metadata" — exactly B); matches hexagonal purity (new port + in-memory adapter).
   - Cons: terminology split (code `StableEntityId` vs spec word "EntityId") must be pinned explicitly in the e38 spec delta and mapped to ADR-038; consumers must query the continuity layer for stable identity. Effort: **Medium**.
3. **C — Rename snapshot-scoped `EntityId` → `OccurrenceId` everywhere and introduce logical `EntityId`** — maximally faithful naming, but churns every e37 file, forces digest re-pin + golden re-baseline for zero behavioral gain over B. Effort: High. Rejected.

**Recommendation: B.** Satisfies every testable scenario (identity retention, ambiguous fail-closed) without invalidating pinned evidence; the e38 spec phase records the terminology mapping (`EntityId`-in-spec = `StableEntityId`; occurrence = snapshot-scoped `EntityId` + `OccurrenceId`).

## Semantic fingerprint feasibility (task 4.2)

Available at extraction time per symbol: name (GraphNode.label), `SymbolKind` (`core:defines` detail), file path, line/column (`extractor.rs:341-358 make_symbol_node`), file-level SHA256 (`ExtractionResult.content_hash`, types.rs:75). **No signature text, no body hash today.** LSP (`domain/traits/code_intelligence.rs`) gives symbols/kinds/ranges and hover signature text — RuntimeObserver, not deterministically available offline. Feasible deterministic fingerprint v1 computed FROM FACTS (no extractor change): `(SymbolKind, name)` as the equality core + sorted callee-name/type-ref multisets from the entity's `core:calls`/`core:references` facts as a similarity signal (Jaccard). Extending `make_symbol_node` with a body-range hash is a small additive extractor change but risks touching e36 goldens — defer to design; only if tiers below prove insufficient.

## Git rename/move evidence (task 4.4)

No `git2` dependency anywhere (workspace grep empty). Existing pattern: shell-out `infrastructure/git/git_history.rs:23-28` (`git log --follow` via `std::process::Command`). Hook point: new kernel-namespaced port (e.g. `RenameEvidencePort` in `domain/evidence_kernel/ports.rs`, respecting the A4 no-cross-re-export rule) returning `[(old_path, new_path, similarity)]`; infrastructure adapter shells out to `git diff -M --name-status --find-renames`. Domain purity preserved (trait in domain, adapter in infrastructure). Fixtures are NOT standalone git repos (lsi-baseline is tracked by the parent repo), so: unit tests use fixture-declared evidence; git-adapter tests build throwaway `git init` repos in temp dirs (ignored/integration tier).

## Continuity matcher sketch (task 4.3)

Inputs: `facts_in_snapshot(N)`, `facts_in_snapshot(N+1)`, optional rename evidence. Tiers: **T0** raw FQN present in both → same entity (unchanged only); **T1** same file path + same name + same kind → match (covers UAT-U20 line shifts); **T2** git rename/move evidence on the containing file + (kind, name) → match (covers UAT-U21 moves); **T3** (kind, name) across files with structural similarity (callee-multiset Jaccard ≥ declared threshold) → renamed match (covers rename/rename+edit). Ambiguity: >1 candidate within margin or scores within epsilon → status `Ambiguous` with candidates listed, never merged (ADR-038, UAT-U22). Statuses: `Matched{evidence, confidence}` | `New` | `Terminated` | `Ambiguous{candidates}`. All new ids assigned by the same sorted-sequential discipline.

## Workspace isolation suite (collisions = 0)

`WorkspaceId` (value_objects/workspace_id.rs) is a non-empty string providing hard isolation; `InMemoryFactStore` keys by `(WorkspaceId, SnapshotId)` — cross-workspace fact mixing is impossible by construction. Suite shape (reuse e37 harness skeleton, file-level `#![cfg(feature = "evidence-kernel")]`): run the full extract → commit → continuity pipeline over TWO workspaces with IDENTICAL symbol sets (same FQNs, names, kinds) and assert per-snapshot EntityId tables are workspace-independent and no `StableEntityId` mapping or ambiguous candidate ever crosses workspaces.

## Benchmark harness (task 4.5)

Mirror the e37 pattern (crates/cognicode-core/tests/…, declared constants, named report, digest pin, quarantine): new `tests/identity_benchmark.rs` with `PRECISION_THRESHOLD = 0.95`, `RECALL_THRESHOLD = 0.90` (state.yaml gates), a NEW convention text + pinned digest for the fingerprint/matcher (separate from e37's `PINNED_IDENTITY_DIGEST`), and a `just lsi-identity` recipe. Ground truth: before/after source-tree pairs + an expected-mapping JSON (`{before, after, outcome}`) covering pure rename, rename+edit, move, move+edit, colliding names (ambiguity), line shift (UAT-U20), unchanged control — e.g. `sandbox/fixtures/lsi-identity/<case>/`. Scoring: matched-vs-expected multiset → precision/recall; ambiguous counts as neither match nor miss (reported separately), matching UAT-U22.

## Risks

- Terminology drift between spec text ("same EntityId") and code (`StableEntityId`) — must be pinned in the e38 delta spec; else downstream phases misimplement.
- Adding a `ProducerKind` variant for matcher provenance would alter `ProvenanceRecord` serialization (bincode-variant indices) — reuse `DeterministicAnalyzer` + detail string instead.
- `SnapshotDescriptor.source_state` stays empty in M3 (no git-SHA capture exists); ADR-039 full experiment model is a later concern — note as deferred, do not half-wire.
- Perf-gate noise (e37 sub-us bench WARNING unresolved) — identity benchmark must reuse the bench-compare pattern and expect possible noise adjudication.
- e38 owns only NEW identity/continuity files + strictly additive kernel/bridge edits (state.yaml delivery note); Option B conforms, Option A would not.
- M2 equivalence harness unaffected by B (single-snapshot), but `facts_in_snapshot` ordering is commit-order — matcher must sort facts itself.

## Ready for Proposal

Yes. Recommend Option B for sdd-propose: additive `StableEntityId` + continuity port/adapter/matcher + fingerprint v1 from facts + git-evidence port (shell-out adapter) + two-fixture-tier benchmark harness; no re-pin of e37 evidence.
