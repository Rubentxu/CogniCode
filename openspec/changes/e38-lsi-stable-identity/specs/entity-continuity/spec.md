# Delta for entity-continuity

> New capability; no existing main spec. Terminology pin: umbrella "same EntityId" = cross-snapshot `StableEntityId`; occurrence = snapshot-scoped `EntityId` + `OccurrenceId`. Facts, goldens, and e37's pinned digest stay unchanged.

## ADDED Requirements

### Requirement: Tiered deterministic continuity matching

Continuity MUST map each occurrence to a `StableEntityId` plus status via ordered deterministic tiers: unchanged identity string; same path, name, and kind; rename/move evidence with matching kind and name; fingerprint similarity above threshold. The fingerprint (facts-only, no extractor change) combines a kind-and-name equality core with sorted callee/type-reference multisets as similarity signal. The matcher MUST sort facts itself and tie-break deterministically; identical fact sets MUST yield identical mappings regardless of commit order.

#### Scenario: Line shift keeps stable identity

- GIVEN a function re-extracted after unrelated lines are inserted above it
- WHEN continuity is resolved
- THEN the new occurrence maps to the same `StableEntityId`

#### Scenario: File move keeps stable identity

- GIVEN a symbol whose containing file moves between snapshots unchanged
- WHEN move evidence is declared
- THEN the occurrence maps to the same `StableEntityId`

#### Scenario: Fingerprint is fact-deterministic

- GIVEN the same committed fact set
- WHEN fingerprints are computed twice
- THEN both runs produce identical fingerprints

### Requirement: Ambiguous continuity fails closed

Statuses MUST be `Matched`, `New`, `Terminated`, or `Ambiguous`. Multiple candidates within the configured margin, or scores tied within epsilon, MUST yield `Ambiguous` listing candidates; identities MUST NOT be merged, and `Ambiguous` MUST be terminal per snapshot (never retroactively force-matched).

#### Scenario: Colliding names fail closed

- GIVEN two equally plausible rename targets for one occurrence
- WHEN continuity cannot exceed the configured margin
- THEN the status is `Ambiguous`, candidates listed, no merge created

### Requirement: Fail-closed rename evidence port

The rename-evidence port MUST live in the domain layer; its adapter obtains evidence from the version-control tool. On tool absence, failure, or indeterminate output, it MUST return no evidence, never invented evidence, so matching falls through tiers.

#### Scenario: Version control unavailable degrades safely

- GIVEN a workspace without version-control metadata
- WHEN the adapter is queried
- THEN it returns no evidence; matching proceeds without rename support

### Requirement: Workspace isolation of continuity

Continuity MUST be workspace-isolated: with two workspaces committing identical symbol sets, occurrence tables, stable-identity mappings, and ambiguous candidates MUST NOT intersect; collisions MUST be zero.

#### Scenario: Identical workspaces do not collide

- GIVEN two workspaces committing identical symbol sets
- WHEN the continuity pipeline runs for both
- THEN no mapping or candidate crosses workspaces; collisions are zero
