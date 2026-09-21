# Pack Ecosystem Specification

## Purpose

Define versioned extension packs and authority.

## Requirements

### Requirement: Manifest and namespaces

Every pack MUST declare id/version/capabilities and owned namespaced schemas before activation.

#### Scenario: Schema conflict fails

- GIVEN two packs claim incompatible ownership of same schema id
- WHEN the second loads
- THEN load fails before behavior execution

### Requirement: Authority ladder

A new pack SHALL NOT gain GATE authority solely by being installed.

#### Scenario: Observe pack cannot block

- GIVEN pack authority OBSERVE
- WHEN it produces a blocker-severity finding
- THEN CI records it but does not block based on that pack alone
