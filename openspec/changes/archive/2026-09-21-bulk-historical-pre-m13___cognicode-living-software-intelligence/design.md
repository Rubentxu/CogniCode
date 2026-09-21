# Design: Living Software Intelligence Foundation

## Architecture

```mermaid
flowchart LR
    S[Sources] --> SP[Semantic Providers]
    SP --> K[Evidence Kernel]
    K --> P[Projections]
    P --> A[Analyses]
    K --> E[Intelligence Event Log]
    A --> E
    E --> R[Reactive Runtime]
    R --> H[Hypotheses / Proposals]
    H --> F[Fork / Trial / Promote]
    A --> X[Explorer / MCP / CI]
    F --> X
```

## Core decisions

1. Facts are canonical; graphs are projections.
2. Entity identity is separate from source occurrence.
3. Snapshot pins source+analysis configuration.
4. LLM outputs are hypotheses/proposals.
5. Read dependencies are lineage.
6. Reactive work is bounded and policy-controlled.
7. Software worlds provide isolated experimentation.

## Projection migration sequence

```mermaid
sequenceDiagram
    participant Src as Source
    participant Legacy as Legacy Extractor
    participant Sem as Semantic Provider
    participant Facts as FactStore
    participant Proj as CallGraphProjection
    participant Eq as Equivalence Harness

    Src->>Legacy: build current CallGraph
    Src->>Sem: extract observations
    Sem->>Facts: commit FactBatch
    Facts->>Proj: build projected CallGraph
    Legacy->>Eq: expected graph
    Proj->>Eq: candidate graph
    Eq-->>Proj: structural/semantic diff
```

## Reactive CI sequence

```mermaid
sequenceDiagram
    participant Git as PR/Commit
    participant Ing as Ingest
    participant FK as Fact Kernel
    participant RX as Reactive Runtime
    participant CI as Work Planner
    participant Ev as Evidence Bundle
    participant Gate as Policy Gate

    Git->>Ing: source delta
    Ing->>FK: fact delta
    FK->>RX: facts.committed
    RX->>CI: affected closure/read-set invalidation
    CI->>CI: select required analyses/tests
    CI->>Ev: collect evidence
    Ev->>Gate: evaluate sufficiency/risk
    Gate-->>Git: pass/warn/block + explanation
```

## Data authority

| Producer | Can write Fact | Can write Evidence | Can write Hypothesis | Can write ChangeProposal |
|---|---:|---:|---:|---:|
| Tree-sitter adapter | yes | yes | no | no |
| LSP/compiler adapter | yes | yes | no | no |
| deterministic analyzer | derived facts | yes | no | no |
| runtime/test adapter | observed facts | yes | no | no |
| LLM agent | no | agent evidence | yes | yes |
| human | manual fact through explicit workflow | yes | yes | yes |

## Storage

LadybugDB remains the default graph-oriented persistence adapter. The domain contract SHALL remain backend-neutral. Content-addressed batch storage MAY use a separate physical representation if benchmarks show material benefit.

## Incremental computation

Phase 1 uses explicit dependency/read sets and cached summaries. Differential Dataflow is a later optimization gated by SPIKE-011; it SHALL NOT drive Fact semantics.
