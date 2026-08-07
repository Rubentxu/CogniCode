# Advanced Analytics Evidence Gate Specification

## Purpose

Define the evidence-gate ledger, admission criteria, and opt-in Neo4j CI parity
oracle for cohort-3+ graph analytics candidates per ADR-014 §6 admission
contract and §8 Neo4j oracle boundary.

## Requirements

### Requirement: Evidence gate ledger

The platform MUST maintain an evidence-gate ledger recording the admission
decision for every cohort-3+ algorithm candidate. Each ledger entry MUST
declare the algorithm name, the decision category (`ADMIT`,
`REJECT_AS_COMPOSE`, or `DEFER_WITH_MEASUREMENT_PLAN`), the evidence
supporting the decision, and the conditions under which the decision MUST be
re-evaluated. The ledger MUST at minimum contain entries for personalized
PageRank, conductance, modularity, k-shortest paths, multi-source
reachability, betweenness, Leiden, and node similarity.

#### Scenario: Ledger contains all eight cohort-3+ decisions

- GIVEN the evidence-gate ledger is read
- WHEN entries are listed
- THEN it contains at least personalized PageRank, conductance, modularity, k-shortest paths, multi-source reachability, betweenness, Leiden, and node similarity

#### Scenario: Each ledger entry declares decision, evidence, and conditions

- GIVEN any entry in the evidence-gate ledger
- WHEN the entry is read
- THEN it contains the algorithm name, a decision category, supporting evidence, and re-evaluation conditions

### Requirement: Cohort-3 admission rule

A cohort-3+ algorithm SHALL be admitted only when BOTH measured-as-low-cost
AND product-relevant evidence exists. Algorithms that fail either criterion
MUST be rejected as derivable or deferred with a measurement plan; they
SHALL NOT be silently dropped or admitted without evidence.

#### Scenario: Admission without evidence is rejected

- GIVEN a candidate algorithm with no recorded cost or relevance evidence
- WHEN admission is requested
- THEN the platform returns `EvidenceInsufficient` and registers no descriptor

#### Scenario: Low-cost but unproven relevance is deferred

- GIVEN a candidate algorithm with cost evidence but no product relevance signal
- WHEN admission is requested
- THEN the decision category is `DEFER_WITH_MEASUREMENT_PLAN` and the candidate stays out of the catalog

### Requirement: Opt-in Neo4j CI parity oracle

The platform MUST activate Neo4j GDS as an opt-in CI parity oracle for
overlapping algorithms and query semantics. The oracle MUST be gated on the
`NEO4J_URI` environment variable. Production MUST NOT depend on Neo4j and no
canonical graph write SHALL be sent to Neo4j.

#### Scenario: Build remains green without Neo4j configured

- GIVEN the test suite runs with no `NEO4J_URI` set
- WHEN the build completes
- THEN the suite passes with the oracle skipped and no test references Neo4j

#### Scenario: Oracle activates only when configured

- GIVEN `NEO4J_URI` is set and a parity fixture exists for an admitted algorithm
- WHEN the parity check runs
- THEN the oracle compares native and Neo4j outputs and records the result

### Requirement: Neo4j parity result recording

When Neo4j is configured and a parity fixture executes, the oracle MUST
record agreement or divergence against the declared numeric tolerance (1e-6
for seeded floating-point outputs, 1e-9 for cost). The result MUST be
CI-observable as a structured artifact.

#### Scenario: Agreement within tolerance is recorded

- GIVEN a parity fixture where native and Neo4j outputs differ by less than the declared tolerance
- WHEN the oracle runs
- THEN the recorded result is `ParityAgreement` with both values persisted

#### Scenario: Divergence outside tolerance is recorded for review

- GIVEN a parity fixture where outputs differ beyond the declared tolerance
- WHEN the oracle runs
- THEN the recorded result is `ParityDivergence` with both values and the delta persisted for review

### Requirement: Rejected algorithms exist only as composition helpers

Algorithms rejected as derivable (k-shortest paths, multi-source
reachability) MUST exist as thin composition helpers and SHALL NOT appear in
the admitted catalog. A composition helper MUST be expressible as a
composition of already-admitted algorithms.

#### Scenario: Rejected algorithms are not in the catalog

- GIVEN the production analytics registry
- WHEN k-shortest paths or multi-source reachability are listed
- THEN neither appears and `NotAdmitted` is returned on request

#### Scenario: k-shortest paths composed from admitted primitives

- GIVEN a directed projection and a `k=3` request
- WHEN a user composes k-shortest paths from `bounded_shortest_paths` plus a sort
- THEN a valid k-shortest path sequence is returned within `max_result_rows`

### Requirement: Deferred algorithms require measurement plans

Algorithms deferred (betweenness, Leiden, node similarity) MUST specify a
measurement plan describing the relevance signal, retrieval signal, or
workload that would justify re-evaluation. Re-evaluation SHALL require
fresh evidence, not the original deferral rationale.

#### Scenario: Deferred algorithms are not in the catalog

- GIVEN the production analytics registry
- WHEN betweenness, Leiden, or node similarity are listed
- THEN none appears and `NotAdmitted` is returned on request

#### Scenario: Deferral entry references a measurement plan

- GIVEN a deferred algorithm entry in the evidence-gate ledger
- WHEN the entry is read
- THEN a measurement plan identifier is present and references a CogniCode-internal doc