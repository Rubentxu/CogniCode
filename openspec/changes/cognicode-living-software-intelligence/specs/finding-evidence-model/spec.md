# Finding Evidence Model Specification

## Purpose

Define findings as evidence-backed conclusions.

## Requirements

### Requirement: Finding evidence chain

Every blocking Finding MUST link the Evidence supporting the decision and the detector/version that produced it.

#### Scenario: Blocking finding is explainable

- GIVEN a blocking finding
- WHEN a user requests its explanation
- THEN source/sink/path or equivalent evidence and causal lineage are available

### Requirement: Evidence grades

The platform SHALL expose evidence grade A/B/C/D and policies MAY gate on grade plus risk.

#### Scenario: Hypothesis cannot satisfy strong gate

- GIVEN a grade D finding only
- WHEN a gate requires grade B or stronger
- THEN the finding alone cannot block the gate
