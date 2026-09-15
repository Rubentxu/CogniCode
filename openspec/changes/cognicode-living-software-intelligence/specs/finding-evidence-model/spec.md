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

### Requirement: Evidence classes

The platform SHALL expose evidence classes A/B/C/D — the strength of the finding's supporting evidence. This is distinct from the kernel `EvidenceGrade` (Supports/Refutes/Corroborates), which records how one piece of evidence relates to a fact. Policies MAY gate on class plus risk.

#### Scenario: Hypothesis cannot satisfy strong gate

- GIVEN a grade D finding only
- WHEN a gate requires grade B or stronger
- THEN the finding alone cannot block the gate
