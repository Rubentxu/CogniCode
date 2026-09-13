# Delta for projection-equivalence-harness

> New capability introduced by this change; no existing main spec. Scope: the equivalence-measurement and cutover-gating contract for fact-derived projections. CallGraph projection equivalence and rebuildability live in the umbrella change `cognicode-living-software-intelligence` (`projection-architecture`); fact/provenance/snapshot semantics live in its `evidence-kernel` spec. e37 implements both; this capability makes their declared tolerance, quarantine, and cutover guarantees testable.

## ADDED Requirements

### Requirement: Declared equivalence contract

The harness MUST declare, before any comparison, a minimum structural equivalence threshold (MUST be at least 99%) and a deterministic comparison method over normalized (stably sorted) node/edge multisets per golden fixture. A harness run MUST fail when any non-quarantined fixture scores below the declared threshold, and MUST report per-fixture equivalence scores.

#### Scenario: All fixtures meet the threshold

- GIVEN the golden fixture set with legacy and fact-derived projections built per fixture
- WHEN the harness compares normalized node/edge multisets
- THEN every non-quarantined fixture scores at or above the declared threshold AND the run passes with a per-fixture report

#### Scenario: Fixture below threshold fails the run

- GIVEN one non-quarantined fixture whose normalized multisets score below the declared threshold
- WHEN the harness runs
- THEN the run fails AND the report names the fixture and its score

### Requirement: Quarantine of known-unstable surfaces

Divergences on surfaces explicitly listed in the declared KNOWN_UNSTABLE_SURFACES quarantine MUST be excluded from the equivalence score and reported as quarantined. Any divergence on a surface not covered by the quarantine MUST fail the run; the harness MUST NOT silently tolerate it.

#### Scenario: Quarantined divergence is excluded and reported

- GIVEN a known-unstable surface that diverges between legacy and fact-derived projections
- WHEN the harness runs
- THEN that surface is excluded from the equivalence score AND is listed as quarantined in the report

#### Scenario: Unquarantined divergence fails

- GIVEN a divergence on a surface not listed in the quarantine
- WHEN the harness runs
- THEN the run fails AND the diverging surface is reported

### Requirement: Deterministic fact identity

Fact adapters feeding compared projections MUST derive entity identities through one declared, deterministic, golden-pinned convention and MUST emit only predicates from the canonical registered relation set, so that identical inputs yield identical fact sets across runs.

#### Scenario: Repeated extraction is identical

- GIVEN an unchanged golden fixture
- WHEN its extraction adapters run twice
- THEN both runs produce identical fact sets (subject, predicate, object)

#### Scenario: Convention change requires explicit re-baseline

- GIVEN a change to the entity-identity convention
- WHEN the harness runs
- THEN it fails until the affected goldens are explicitly re-pinned as a new baseline

### Requirement: No silent cutover

The fact-derived CallGraph MUST NOT become the default serving source unless a harness run passes on the full golden fixture set with only quarantined exclusions. The bridge path MUST be reachable only behind the off-by-default evidence-kernel feature gate.

#### Scenario: Gate off serves the legacy path

- GIVEN the evidence-kernel feature disabled
- WHEN any consumer requests graph outputs
- THEN results are served from the legacy path AND committed goldens remain byte-stable

#### Scenario: Failing harness keeps legacy default

- GIVEN a harness run failing any non-quarantined fixture
- WHEN default source selection is evaluated
- THEN the legacy CallGraph remains the serving source
