# Spec: e11 — Truncation Field Naming Harmonisation

## Purpose

Harmonise the inconsistency between `truncated_reason` (used in `LandingPayload`
and `SubgraphResponse`) and `truncation_reason` (used in
`ContextualGraphResponse`). Both fields carry the same semantic meaning:
the cause string when `truncated: true`.

The canonical name going forward is `truncated_reason` (no 'i'), aligned with
`SubgraphResponse` and `LandingPayload`. The `ContextualGraphResponse` field
must be renamed with a wire-compatible migration strategy to avoid breaking
existing API consumers.

---

## ADDED Requirements

### Requirement: 1. Rename `truncation_reason` → `truncated_reason` in `ContextualGraphResponse`

The `ContextualGraphResponse` struct in `crates/cognicode-explorer/src/dto.rs`
MUST rename the field `truncation_reason` to `truncated_reason`.

The JSON key in wire format MUST also change from `truncationReason`
(`camelCase` rename from `truncation_reason`) to `truncatedReason`
(`camelCase` rename from `truncated_reason`).

#### Scenario: Wire format changes

- GIVEN a `ContextualGraphResponse` with `truncated: true`
- WHEN serialised to JSON
- THEN the JSON field name is `truncatedReason` (no 'i')
- AND the value is the same string as before

### Requirement: 2. Wire-Compatible Migration (Dual-Field Deprecation)

To avoid breaking existing API consumers during the transition:

1. The old field `truncationReason` (camelCase) MUST be accepted as input during
   deserialisation (skip serialisation on write).
2. The new field `truncatedReason` (camelCase) MUST be written on serialisation.
3. A Rust deprecation comment MUST mark the old field as deprecated.

#### Scenario: Deserialisation accepts old field

- GIVEN JSON with `{"truncationReason": "max_nodes_exceeded", "truncated": true}`
- WHEN deserialised into `ContextualGraphResponse`
- THEN `resp.truncated_reason == Some("max_nodes_exceeded")`
- AND no error is returned

#### Scenario: Serialisation produces new field only

- GIVEN `ContextualGraphResponse` with `truncated_reason: Some("max_nodes_exceeded")`
- WHEN serialised to JSON
- THEN the JSON contains `{"truncatedReason": "max_nodes_exceeded", ...}`
- AND the JSON does NOT contain `truncationReason`

### Requirement: 3. Update Call Sites

All call sites that construct `ContextualGraphResponse` and set
`truncation_reason` MUST be updated to use `truncated_reason`:

- `crates/cognicode-explorer/src/facades/view.rs` (line ~269)
- Any test files that assert on the old field name

### Requirement: 4. ADR for Migration

This cycle MUST include an ADR (or ADR delta) documenting the wire-compatible
migration decision, including:
- Why dual-field (deprecation period) was chosen over a breaking change.
- Estimated timeline for removing the deprecated field (e.g., next MAJOR).
- Consumer guidance: update to `truncatedReason` before the next MAJOR.

---

## UNCHANGED Requirements

- The `truncated: bool` field behaviour is unchanged.
- The `ContextualGraphResponse` endpoint contract (`GET /api/graph/:id/contextual`)
  is unchanged except for the field rename.
- `LandingPayload.truncated_reason` and `SubgraphResponse.truncated_reason`
  are unchanged (they already use the canonical name).

---

## Acceptance Criteria

- [ ] `ContextualGraphResponse.truncation_reason` is removed from the struct.
- [ ] `ContextualGraphResponse.truncated_reason` serialises as `truncatedReason`.
- [ ] Old JSON with `truncationReason` deserialises correctly (backwards compat).
- [ ] All Rust tests pass: `cargo test -p cognicode-explorer`.
- [ ] Frontend Zod schema for `ContextualGraphResponse` is updated to use
  `truncatedReason` (no 'i').
- [ ] ADR migration note is added.
