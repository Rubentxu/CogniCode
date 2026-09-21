# Detector IR Specification

## Purpose

Define declarative detector composition across analysis backends.

## Requirements

### Requirement: Validated detector definition

Detector definitions MUST be parsed/validated before admission and MUST declare required analysis capabilities.

#### Scenario: Unsupported construct fails loud

- GIVEN a detector uses an unsupported IR construct
- WHEN it is registered
- THEN registration fails with a structured error before scanning

### Requirement: AI detector authority

An AI-authored detector SHALL start as a CandidateDetector without GATE authority.

#### Scenario: Candidate detector shadow mode

- GIVEN an agent proposes a detector
- WHEN it passes syntax validation only
- THEN it may run in experiment/shadow mode but cannot block CI
