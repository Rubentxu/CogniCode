# Delta for provider-tier-provenance

> New capability; no existing main spec. Umbrella `semantic-provider-pipeline` owns explicit precision and no-fabrication; this delta adds tier order, diagnostics, and tier→provenance mapping. Tiers: S0 tree-sitter, S1 local resolver, S2 LSP (S3/S4 reserved); classes = kernel `Provenance`.

## ADDED Requirements

### Requirement: Ordered policy-gated tier pipeline

The composite provider MUST attempt tiers highest-first in fixed order, attributing each result to its producing tier. Policy MUST gate participating tiers; gated-off, unavailable, or failed tiers are skipped, falling through.

#### Scenario: Gated or failed tier falls through

- GIVEN the LSP tier is policy-disabled or errors
- WHEN a query resolves
- THEN the next eligible tier serves and declares that tier, with no LSP tier recorded as served

### Requirement: Structured diagnostics and counters travel with results

Fallback MUST surface as structured diagnostics attached to the result — naming provider, attempted tier, and outcome (unavailable/error/degraded) — not only log output. Each attempt MUST increment a per-tier counter, queryable from composite status.

#### Scenario: Fallback diagnostic is attached and counted

- GIVEN the LSP tier fails a hover
- WHEN the tree-sitter tier serves it
- THEN the result carries a diagnostic naming failed tier and outcome AND declares the reduced tier AND counters record the fallback

### Requirement: Pinned tier-to-provenance mapping

The fact bridge MUST map tiers to classes — LSP → Extracted, local resolver → Inferred, tree-sitter → Ambiguous — recording tier and provider in provenance detail as an appended `tier=<Tier>` entry. No new producer kind or predicate MAY be introduced. A class contradicting its declared tier MUST fail batch construction.

#### Scenario: Tier decides provenance class

- GIVEN a definition resolved by the local resolver
- WHEN the fact batch commits
- THEN the class is Inferred and detail carries `tier=<S1>` with provider identity

### Requirement: Heuristic results never claim Extracted and unresolved never fabricates

A tree-sitter heuristic binding MUST be classed Ambiguous, never Extracted. When no tier supports a binding, the unresolved outcome MUST propagate: no precise target fact is emitted, uncertainty names the site and exhausted tiers, and no subject or target identity is fabricated (including joinable-posed container fallbacks).

#### Scenario: Ambiguous heuristic match stays Ambiguous

- GIVEN a tree-sitter match on ambiguous line content
- WHEN the fact commits
- THEN its class is Ambiguous

#### Scenario: Unresolved site propagates without a fact

- GIVEN a dynamic call no tier resolves
- WHEN the batch commits
- THEN no target fact exists for that call AND uncertainty names the site and exhausted tiers
