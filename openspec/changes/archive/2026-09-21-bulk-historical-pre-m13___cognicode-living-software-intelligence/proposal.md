# Proposal: CogniCode Living Software Intelligence Foundation

## Intent

Establish the architectural foundation that lets CogniCode evolve from a graph-centric Super-LSP into an evidence-backed, incremental and reactive software intelligence platform without losing current MCP/CLI/Explorer functionality.

## Problem

Current graph, quality, knowledge and analytics capabilities are valuable but further expansion into SAST, CPG, runtime evidence, supply-chain, AI agents and reactive CI risks multiplying sources of truth and coupling new features to specific graph/storage representations.

## Proposed direction

Introduce canonical `Entity`, `Fact`, `Evidence`, `Snapshot` and enriched `Provenance`; make CallGraph/GenericGraph/quality views projections; add stable identity, semantic provider tiers, and later an append-only Intelligence Event Log plus Reactive Intelligence Runtime supporting read sets, patterns, policies and software-world forks.

## In scope

- canonical evidence model;
- stable entity identity;
- snapshot experiment semantics;
- projection architecture;
- semantic provider pipeline;
- finding/detector model;
- reactive/event/fork contracts;
- packs/authority model;
- evidence-driven CI contracts;
- migration/benchmark/UAT plan.

## Out of scope for the first implementation slice

- production-grade distributed dataflow;
- full symbolic execution;
- deep analysis for every Tree-sitter language;
- automatic AI code promotion;
- replacement of current MCP/CLI/Explorer APIs.

## Affected areas

- `cognicode-core` domain/value objects/ports;
- ingestion/revision flow;
- CallGraph/GenericGraph creation;
- Ladybug adapter;
- runtime composition;
- Explorer/MCP projections;
- future quality/analyzer crates.

## Risks

- identity migration churn;
- duplicated state during transition;
- event/provenance storage growth;
- over-generalized ontology;
- incremental invalidation bugs;
- accidental authority escalation for AI/plugins.

## Rollback

Use projection/strangler migration. Legacy CallGraph/GenericGraph paths remain available until equivalence + UAT gates pass. New stores/fields are additive first; destructive removal is deferred to dedicated changes.
