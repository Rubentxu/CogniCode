# Reactive Behaviors Specification

## Purpose

Define bounded reactive execution classes and patterns.

## Requirements

### Requirement: Behavior authority classes

The runtime MUST distinguish PureDerivation, ReactiveAnalysis and AgentBehavior and enforce their allowed outputs.

#### Scenario: Agent cannot emit extracted fact

- GIVEN AgentBehavior executes
- WHEN it attempts to write an extracted Fact
- THEN the runtime rejects the write and records a policy violation

### Requirement: Bounded execution

Every non-trivial behavior MUST run under declared resource/event budget or system default.

#### Scenario: Budget exhaustion is recorded

- GIVEN a behavior exceeds its budget
- WHEN execution stops
- THEN a budget exhaustion event is recorded and canonical snapshot remains consistent
