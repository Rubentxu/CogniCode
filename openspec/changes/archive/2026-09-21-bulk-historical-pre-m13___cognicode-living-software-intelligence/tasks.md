# Tasks: Living Software Intelligence Foundation

> **Completion state is authoritative in `state.yaml`.** The checkboxes below
> describe the original decomposition and are intentionally left immutable;
> consult `state.yaml` for operational status. (Contract chosen in cycle e55.)

## 1. Baseline and contracts

- [ ] 1.1 Inventory current MCP/CLI/Explorer consumers of CallGraph/GenericGraph.
- [ ] 1.2 Create golden multi-language repositories.
- [ ] 1.3 Capture current graph/symbol/impact outputs as fixtures.
- [ ] 1.4 Add benchmark harness and machine-readable baseline artifacts.
- [ ] 1.5 Review and number proposed ADRs.

## 2. Domain foundation

- [ ] 2.1 Add `EntityId`, `OccurrenceId`, `SnapshotId`, `FactId`, `EvidenceId`.
- [ ] 2.2 Add typed `FactValue` and namespaced `RelationKind`.
- [ ] 2.3 Expand provenance to `ProvenanceRecord` while preserving legacy classification.
- [ ] 2.4 Define Evidence and EvidenceGrade.
- [ ] 2.5 Define SnapshotDescriptor.
- [ ] 2.6 Define domain ports FactStore/EvidenceStore/SnapshotStore/SchemaRegistry.
- [ ] 2.7 Add round-trip/property tests.

## 3. Projection bridge

- [ ] 3.1 Adapt Tree-sitter extraction to FactBatch.
- [ ] 3.2 Adapt LSP observations to FactBatch.
- [ ] 3.3 Implement CallGraphProjection.
- [ ] 3.4 Implement GenericGraphProjection.
- [ ] 3.5 Add structural equivalence harness.
- [ ] 3.6 Run UAT-U10/U11.

## 4. Identity and temporal knowledge

- [ ] 4.1 Implement occurrence identity.
- [ ] 4.2 Implement stable semantic fingerprint.
- [ ] 4.3 Implement continuity matcher with ambiguous state.
- [ ] 4.4 Integrate Git rename/move evidence.
- [ ] 4.5 Publish identity precision/recall benchmark.

## 5. Semantic provider pipeline

- [ ] 5.1 Define provider precision tiers and diagnostics.
- [ ] 5.2 Refactor CompositeProvider behind common pipeline.
- [ ] 5.3 Add fallback event/evidence semantics.
- [ ] 5.4 Add SCIP/compiler adapters only where spikes justify them.
- [ ] 5.5 Add semantic conformance fixtures for Rust/TS/Java.

## 6. Analysis foundation

- [ ] 6.1 Define CFG/DFG projection contracts.
- [ ] 6.2 Implement function summaries.
- [ ] 6.3 Add slicing/dominator integration using graph-algos.
- [ ] 6.4 Add taint v1.
- [ ] 6.5 Add optional Z3 feasibility adapter.
- [ ] 6.6 Publish performance/resource envelopes.

## 7. Findings and detectors

- [ ] 7.1 Define Finding/Risk/EvidenceClass lifecycle.
- [ ] 7.2 Define Detector IR schema/parser/validator.
- [ ] 7.3 Implement AST detector backend.
- [ ] 7.4 Implement graph-pattern backend.
- [ ] 7.5 Implement dataflow backend.
- [ ] 7.6 Implement QualityIssue compatibility projection.
- [ ] 7.7 Build Axiom rule classifier/import tooling.

## 8. Reactive runtime

- [ ] 8.1 Define IntelligenceEvent/EventStore.
- [ ] 8.2 Implement causal `caused_by` and correlation.
- [ ] 8.3 Generalize RunLineage to ExecutionLineage.
- [ ] 8.4 Add context/read-set recorder.
- [ ] 8.5 Implement PureDerivation registry.
- [ ] 8.6 Implement ReactiveAnalysis registry.
- [ ] 8.7 Implement AgentBehavior restricted output surface.
- [ ] 8.8 Add pattern subscriptions and budgets.
- [ ] 8.9 Add policy/approval primitives.

## 9. CI workflows

- [ ] 9.1 Implement semantic PR world/fork descriptor.
- [ ] 9.2 Implement semantic diff.
- [ ] 9.3 Implement affected-work planner.
- [ ] 9.4 Implement EvidenceBundle.
- [ ] 9.5 Implement conservative full-work fallback.
- [ ] 9.6 Expose `why_scheduled` through CLI/Explorer.

## 10. Fork/trial/promotion

- [ ] 10.1 Implement fork lineage.
- [ ] 10.2 Implement structural/semantic world diff.
- [ ] 10.3 Implement ChangeProposal lifecycle.
- [ ] 10.4 Implement sandbox TrialExecutor port.
- [ ] 10.5 Implement three-way promote dry-run.
- [ ] 10.6 Implement fail-closed apply and audit marker.

## 11. Packs and architecture

- [ ] 11.1 Define pack manifest and capability schemas.
- [ ] 11.2 Add authority ladder/shadow mode.
- [ ] 11.3 Add pack conformance suite.
- [ ] 11.4 Define ArchitectureConstraint.
- [ ] 11.5 Implement ADR->candidate constraint workflow.
- [ ] 11.6 Implement drift finding and Explorer view.

## 12. AI agents

- [ ] 12.1 Define InvestigationFrame.
- [ ] 12.2 Implement LlmPort/provider-neutral interface.
- [ ] 12.3 Implement Semantic Miner prototype.
- [ ] 12.4 Implement Finding Critic prototype.
- [ ] 12.5 Implement Fix Agent emitting ChangeProposal only.
- [ ] 12.6 Add prompt/tool/read lineage and security tests.

## 13. Continuous improvement

- [ ] 13.1 Implement historical replay dataset format.
- [ ] 13.2 Enforce OPTIMIZE/CONFIRM disjoint sets.
- [ ] 13.3 Define FailureRegime taxonomy contract.
- [ ] 13.4 Add analyzer shadow comparison.
- [ ] 13.5 Add held-out promotion gate.
- [ ] 13.6 Demonstrate one governed improvement end-to-end.
