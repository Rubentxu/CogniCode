# merge-candidate-detection Specification (NEW)

## Purpose

Heuristic detector that flags nodes in **different** spaces that look like they represent the same real-world entity. Detection runs lazily on demand via `FederatedGraphService::detect_merge_candidates()`. The system **suggests**, never auto-merges — humans (or downstream tools) confirm. Gated by `multimodal` feature.

## Domain Types

| Type | File | Definition |
|------|------|------------|
| `MergeCandidate` | `crates/cognicode-explorer/src/federation/merge_candidate.rs` | `pub struct MergeCandidate { pub left: FederatedNode, pub right: FederatedNode, pub confidence: f64, pub reasons: Vec<MergeReason> }` |
| `MergeReason` | `crates/cognicode-explorer/src/federation/merge_candidate.rs` | `enum MergeReason { LabelMatch, KindMatch, PropertyOverlap }` |
| `MergeDetector` | `crates/cognicode-explorer/src/federation/merge_detector.rs` | Pure function `detect(&[FederatedNode]) -> Vec<MergeCandidate>` |

## Requirements

### Requirement: Label Normalization

Label normalization MUST lowercase, trim, strip surrounding punctuation, and collapse internal whitespace to single spaces. The normalized form is used for equality comparison.

#### Scenario: Whitespace and case normalized
- GIVEN labels `"User Service"`, `"user service"`, `"  user  service  "`
- WHEN normalized
- THEN all three produce `"user service"`

#### Scenario: Punctuation stripped
- GIVEN label `"User-Service"`
- WHEN normalized
- THEN it produces `"user-service"` (the hyphen is preserved as a separator)

### Requirement: Heuristic Scoring

Confidence is computed as a sum of weighted components:

| Component | Weight | Condition |
|-----------|--------|-----------|
| Base | 0.5 | Pair lives in different spaces (the only condition that makes a merge *plausible*) |
| Label match | 0.3 | `normalize(left.label) == normalize(right.label)` |
| Kind match | 0.2 | `left.kind == right.kind` |
| Property overlap | +0.1 (cap at 1.0) | Any key in `left.properties` is present in `right.properties` with the same value |

Pairs in the **same** space are NEVER candidates (the function filters them out before scoring).

#### Scenario: Full match reaches 1.0
- GIVEN `left` in `repo-a` and `right` in `repo-b` with same label, same kind, and 1 overlapping property
- WHEN scored
- THEN `confidence == 1.0` (capped)

#### Scenario: Label-only match
- GIVEN same label, different kind
- WHEN scored
- THEN `confidence == 0.8` (base 0.5 + label 0.3)

#### Scenario: Kind-only match
- GIVEN different label, same kind
- WHEN scored
- THEN `confidence == 0.7` (base 0.5 + kind 0.2)

#### Scenario: Same-space pair filtered
- GIVEN `left` and `right` both in `repo-a` with identical labels and kinds
- WHEN scored
- THEN the pair does NOT appear in the result

### Requirement: Threshold Filter

`detect(nodes) -> Vec<MergeCandidate>` MUST return only pairs with `confidence >= 0.8`. The threshold is a `const DETECTION_THRESHOLD: f64 = 0.8;` exported from the module.

#### Scenario: Below threshold excluded
- GIVEN a same-label, different-kind pair (confidence 0.8) AND a same-kind, different-label pair (confidence 0.7)
- WHEN `detect` runs
- THEN the 0.7 pair is excluded; the 0.8 pair is included

#### Scenario: Empty input
- GIVEN `detect(&[])` runs
- THEN the result is `vec![]`

### Requirement: Reason Trace

Each `MergeCandidate` MUST include a non-empty `reasons: Vec<MergeReason>` listing the components that fired. The `MergeReason` enum is non-exhaustive (prefixed with `_NonExhaustive`) so future scoring components can be added without a breaking change.

#### Scenario: Reasons populated
- GIVEN a full-match pair
- THEN `reasons` MUST contain `LabelMatch`, `KindMatch`, AND `PropertyOverlap` (in any order)

#### Scenario: Label-only reasons
- GIVEN a label-only match
- THEN `reasons` MUST equal `vec![MergeReason::LabelMatch]`

### Requirement: O(N²) Brute-Force Detection

For v1, `detect` runs an O(N²) comparison across all `FederatedNode` inputs. The function MUST be allocation-light (one `Vec` per output pair). Complexity is acceptable up to N=5000 nodes per call; larger inputs SHOULD be split by the caller.

#### Scenario: Small N is fast
- GIVEN 100 nodes across 3 spaces
- WHEN `detect` runs
- THEN it completes in under 50ms (assertion via test runtime)

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Two nodes with the same label and kind in 3+ spaces | A 3-way cluster produces 3 candidate pairs (each space compared to each other) |
| One node has empty properties | The PropertyOverlap component cannot fire; the rest of the score still applies |
| Labels differ only by trailing punctuation | The strip step normalizes them, so LabelMatch fires |
| Node with very long label (>200 chars) | The full label is normalized; no length cap is imposed in the detector |
| `detect` is called with N=0 nodes | Returns `vec![]` (no allocation beyond the empty vector) |
| Same space + same label + same kind + same properties | Confidence would be 1.0 BUT the same-space filter drops it |

## Out of Scope

- Auto-merge execution
- Property-based deep similarity (edit distance, embeddings)
- Negative evidence ("these two are explicitly NOT the same")
- User feedback loop (accept/reject candidates)
- Cluster-of-N consolidation (only pairwise candidates in v1)

## TDD RED Gate

Before implementation: (1) label normalization unit tests (case, whitespace, punctuation); (2) scoring table tests for the 4 confidence levels (0.5 base-only, 0.7 kind-only, 0.8 label-only, 1.0 full); (3) same-space filter test; (4) threshold filter test (include 0.8, exclude 0.7); (5) reason-population tests; (6) empty-input test. RED gate fails if any test passes before `merge_detector.rs` exists or compiles.

## Dependencies

- `federated-graph-service` — `FederatedNode` input type
- `federated-spaces` — `SpaceId` for the same-space filter
- `generic-graph-model` — `GraphNode`, `NodeKind`

## Multimodal Feature Gate

`MergeCandidate`, `MergeReason`, and `MergeDetector` MUST be `#[cfg(feature = "multimodal")]`. The threshold constant is exposed only when the feature is on. `brain_spaces` (the consumer MCP tool) gracefully returns an empty list when the feature is off.
