# Architecture Decision Records

This directory holds the architecture decision records (ADRs) for the CogniCode workspace.

| Number | Title | Status | Date |
|--------|-------|--------|------|
| [ADR-001](./ADR-001-parked-crates.md) | Parked Crates — Activation Criterion | ACCEPTED | 2026-06-25 |
| [ADR-002](./ADR-002-moldable-exploration-parity-program.md) | Moldable Exploration Parity Program | PROPOSED | 2026-06-25 |
| [ADR-003](./ADR-003-diagram-representations.md) | Diagram Representations — draw.io as Derived View | PROPOSED | 2026-06-28 |
| [ADR-004](./ADR-004-c4-investigation-model.md) | C4 Investigation Model | PROPOSED | 2026-06-28 |
| [ADR-005](./ADR-005-investigation-mode.md) | Investigation Mode — Knowledge Artifacts | PROPOSED | 2026-06-28 |
| [ADR-006](./ADR-006-functional-gtoolkit-parity.md) | Functional GToolkit Parity through MoldQL, ViewSpecs | PROPOSED | 2026-07-02 |
| [ADR-007](./ADR-007-node-properties-graph-query-port.md) | node_properties() on GraphQueryPort | PROPOSED | 2026-07-03 |
| [ADR-008](./ADR-008-node-properties-callgraph-repository-delegation.md) | node_properties() delegation to PostgreSQL | PROPOSED | 2026-07-03 |
| [ADR-009](./ADR-009-knowledge-layer-ports-and-universal-spotter.md) | Knowledge Layer Ports and Universal Spotter | PROPOSED | 2026-07-22 |
| [ADR-010](./ADR-010-diagram-artifacts-as-persistent-views.md) | Diagram Artifacts as Persistent Views | ACCEPTED | 2026-07-22 |
| [ADR-011](./ADR-011-architecture-decision-support-packs.md) | Architecture Decision Support Packs | ACCEPTED | 2026-07-22 |
| [ADR-012](./ADR-012-ui-visible-capability-contract.md) | UI-visible Capability Contract | PROPOSED | 2026-07-22 |
| [ADR-013](./ADR-013-progressive-moldable-workbench-shell.md) | Progressive Moldable Workbench Shell | PROPOSED | 2026-07-22 |
| [ADR-014](./ADR-014-moldql-pattern-graph-analytics-platform.md) | MoldQL Pattern Profile and Graph Analytics Platform | SUPERSEDED | 2026-07-27 |
| [ADR-015](./ADR-015-temporal-graph-history-and-atomic-ingest.md) | Temporal Graph History and Atomic Ingest Commits | SUPERSEDED | 2026-07-29 |
| [ADR-016](./ADR-016-renderer-neutral-semantic-projections.md) | Renderer-Neutral Semantic Projections | PROPOSED | 2026-06-15 |
| [ADR-017](./ADR-017-postgresql-native-ingest-pipeline.md) | PostgreSQL-Native Ingest Pipeline | SUPERSEDED | 2026-06-15 |
| [ADR-018](./ADR-018-evidence-gated-product-operability.md) | Evidence-Gated Product Operability | PROPOSED | 2026-07-29 |
| [ADR-026](./ADR-026-ladybugdb-canonical-migration.md) | LadybugDB as Sole Canonical Graph Store | EXECUTED | 2026-07-30 |
| [ADR-027](./ADR-027-ladybugdb-hybrid-schema-strategy.md) | LadybugDB Hybrid Schema Strategy | ACCEPTED | 2026-07-30 |
| [ADR-028](./ADR-028-ladybugdb-port-abstraction-architecture.md) | Port Abstraction Architecture for LadybugDB Migration | ACCEPTED | 2026-07-30 |
| [ADR-029](./ADR-029-callgraph-projection-port-seam.md) | CallGraphProjectionPort Seam | ACCEPTED | 2026-08-03 |
| [ADR-030](./ADR-030-quality-store-ladybug-schema.md) | QualityStore Schema: LadybugDB Backend | ACCEPTED | 2026-08-03 |
| [ADR-031](./ADR-031-release-1.0.0-definition.md) | Release 1.0.0: Definition of Production-Ready | PROPOSED | 2026-08-05 |
| [ADR-032](./ADR-032-sandbox-validation-system.md) | Sandbox Validation System: Podman Quadlets + Real Repos + Scoring | PROPOSED | 2026-08-05 |
| [ADR-033](./ADR-033-diagram-workbench-wasm-visual-computation.md) | DiagramWorkbench: WASM para Computation Visual | ACCEPTED | 2026-08-09 |
| [ADR-034](./ADR-034-cognicode-distribution-package.md) | Distribution package: cognicode-cli + skill-bundles + IDE adapters | ACCEPTED | 2026-08-10 |
| [ADR-035](./ADR-035-asdf-vm-version-management-pattern.md) | asdf-vm version-management pattern | ACCEPTED | 2026-08-10 |
| [ADR-036](./ADR-036-ide-abstraction-portable-skills-per-ide-adapters.md) | IDE abstraction: portable skills + per-IDE adapter plugins | ACCEPTED | 2026-08-10 |
| [ADR-037](./ADR-037-facts-over-graphs.md) | Facts over Graphs as canonical knowledge | ACCEPTED | 2026-09-12 |
| [ADR-038](./ADR-038-stable-entity-identity.md) | Stable Entity Identity separated from Occurrence | ACCEPTED | 2026-09-12 |
| [ADR-039](./ADR-039-snapshot-experiment-model.md) | Snapshots as reproducible analysis experiments | PROPOSED | 2026-09-12 |
| [ADR-040](./ADR-040-provenance-evidence-hypothesis.md) | Separate Fact, Evidence and Hypothesis | ACCEPTED | 2026-09-12 |
| [ADR-041](./ADR-041-storage-vs-compute-separation.md) | Storage backend separated from incremental compute | PROPOSED | 2026-09-12 |
| [ADR-042](./ADR-042-detector-ir-escalation.md) | Detector IR with cost-aware escalation | PROPOSED | 2026-09-12 |
| [ADR-043](./ADR-043-intelligence-event-log.md) | Intelligence Event Log for causal operational history | EXECUTED | 2026-09-12 |
| [ADR-044](./ADR-044-reactive-behavior-classes.md) | Three behavior classes with different authority | EXECUTED | 2026-09-12 |
| [ADR-045](./ADR-045-read-set-tracing.md) | Execution read sets as first-class lineage | EXECUTED | 2026-09-12 |
| [ADR-046](./ADR-046-software-world-fork-promote.md) | Software World Fork, Trial, Diff and Promote | EXECUTED | 2026-09-12 |
| [ADR-047](./ADR-047-evidence-based-delivery.md) | Evidence Bundle as CI promotion unit | EXECUTED | 2026-09-12 |
| [ADR-048](./ADR-048-pack-manifest-ontology.md) | Packs as extension and governance unit | PROPOSED | 2026-09-12 |
| [ADR-049](./ADR-049-executable-architecture-knowledge.md) | Architecture knowledge as versioned executable constraints | EXECUTED | 2026-09-12 |
| [ADR-050](./ADR-050-code-authorship-without-authority.md) | Code authorship without authority for AI and packs | EXECUTED | 2026-09-12 |
| [ADR-051](./ADR-051-historical-heldout-promotion.md) | Historical replay and held-out promotion | EXECUTED | 2026-09-12 (executed 2026-09-17, e83) |

## Format

Each ADR follows the standard structure:
- **Status**: PROPOSED / ACCEPTED / EXECUTED / DEPRECATED / SUPERSEDED (EXECUTED = decisión aplicada y verificada en código; ACCEPTED = decisión ratificada pero aún sin ejecutar)
- **Date**: ISO date of decision
- **Deciders**: who made the decision
- **Context**: why the decision is needed
- **Decision**: what was decided
- **Alternatives considered**: other options weighed
- **Consequences**: positive, negative, mitigations
- **References**: related ADRs, commits, engram obs

## Convention

- ADRs are numbered sequentially (`ADR-NNN-...md`)
- Filenames use kebab-case
- Status changes are recorded in-place (an accepted ADR is not re-numbered)
- Superseding ADRs reference the prior number in the References section
