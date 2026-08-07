# Knowledge Layer Ports Specification

**Version**: 0.1.0
**Date**: 2026-08-05
**Status**: draft
**Change**: `e13-wave2-knowledge-layer-ports`
**Derived from**: `e13-wave2-knowledge-layer-ports/proposal.md`

## ADDED Requirements

### Requirement: AdrRepository Port Trait

The system MUST provide a `Send + Sync` trait for ADR lifecycle discovery. Implementations SHALL support listing ADRs filtered by status and full-text search across titles and topics. The port MUST use its own `AdrRepositoryError` type, not the explorer error enum.

#### Scenario: List all ADRs for a workspace

- GIVEN three ADRs exist for workspace "ws-1" with statuses Accepted, Superseded, and Rejected
- WHEN `list_adrs("ws-1", None)` is called
- THEN all three summaries are returned

#### Scenario: Filter ADRs by status

- GIVEN ADRs exist with mixed statuses
- WHEN `list_adrs("ws-1", Some(AdrStatus::Superseded))` is called
- THEN only Superseded ADRs are returned

#### Scenario: Search ADRs by title

- GIVEN an ADR with title "Knowledge layer ports"
- WHEN `search_adrs("ws-1", "knowledge", 10)` is called
- THEN matching ADRs are returned case-insensitively

#### Scenario: Search ADRs by topic

- GIVEN an ADR tagged with topic "diagrams"
- WHEN `search_adrs("ws-1", "diagrams", 10)` is called
- THEN ADRs matching the topic are returned

### Requirement: DocRepository Port Trait

The system MUST provide a `Send + Sync` trait for documentation discovery. Implementations SHALL support listing docs filtered by section and full-text search across titles, sections, and excerpts.

#### Scenario: List all docs for a workspace

- GIVEN three docs exist for workspace "ws-1"
- WHEN `list_docs("ws-1", None)` is called
- THEN all three summaries are returned

#### Scenario: Filter docs by section

- GIVEN docs with sections "Introduction" and "Architecture"
- WHEN `list_docs("ws-1", Some("Architecture"))` is called
- THEN only docs in that section are returned

#### Scenario: Search docs by excerpt

- GIVEN a doc with excerpt "A gentle introduction to CogniCode Explorer"
- WHEN `search_docs("ws-1", "introduction", 10)` is called
- THEN the matching doc is returned

### Requirement: EvidenceStore Port Trait

The system MUST provide a `Send + Sync` trait for evidence discovery. Implementations SHALL support listing evidence filtered by kind and full-text search across titles and excerpts.

#### Scenario: List all evidence for a workspace

- GIVEN three evidence items exist for workspace "ws-1"
- WHEN `list_evidence("ws-1", None)` is called
- THEN all three summaries are returned

#### Scenario: Filter evidence by kind

- GIVEN evidence of kinds Trace, Measurement, and External
- WHEN `list_evidence("ws-1", Some(EvidenceKind::Trace))` is called
- THEN only Trace evidence is returned

#### Scenario: Search evidence by excerpt

- GIVEN evidence with excerpt containing "moldable"
- WHEN `search_evidence("ws-1", "moldable", 10)` is called
- THEN matching evidence is returned

### Requirement: AdrInspector Default View

The system SHALL provide an inspector view for `adr:{id}` objects that renders their markdown source. The view MUST be registered in the ViewRegistry under `InspectableObjectType::Adr`.

#### Scenario: Resolve ADR identity through repository

- GIVEN an `adr:ADR-001` object identity
- WHEN the ADR inspector resolves the identity
- THEN the AdrRepository port is queried for ADR "ADR-001"
- AND the source markdown is rendered as the default view

#### Scenario: ADR view listed in available views

- GIVEN a ViewRegistry with the AdrInspector registered
- WHEN `list_for(InspectableObjectType::Adr)` is called
- THEN an ADR source view descriptor is returned

## MODIFIED Requirements

### Requirement: ObjectIdentity ADR Variant

The `ObjectIdentity` enum SHALL include a new `Adr { id: String }` variant. The MVP id parser MUST accept `adr:{id}` (non-empty id). The variant SHALL be mapped to `InspectableObjectType::Adr`. It MUST be distinct from the `Decision` variant — `adr:001` and `decision:001` are different identities.
(Previously: no ADR variant existed; ADRs were routed through `Doc` or graph-backed `Decision`)

#### Scenario: Parse adr MVP id

- GIVEN the string "adr:ADR-028"
- WHEN `ObjectIdentity::parse_mvp_id` is called
- THEN `ObjectIdentity::Adr { id: "ADR-028" }` is returned

#### Scenario: Reject empty ADR id

- GIVEN the string "adr:"
- WHEN `ObjectIdentity::parse_mvp_id` is called
- THEN `ExplorerError::ResolutionFailed` is returned

#### Scenario: ADR round-trips through mvp id

- GIVEN `ObjectIdentity::Adr { id: "ADR-001" }`
- WHEN `to_mvp_id()` is called
- THEN the string "adr:ADR-001" is produced

#### Scenario: ADR maps to InspectableObjectType::Adr

- GIVEN `ObjectIdentity::Adr { id: "ADR-001" }`
- WHEN mapped through `identity_to_inspectable_type`
- THEN `InspectableObjectType::Adr` is returned (not `DecisionArtifact`)
