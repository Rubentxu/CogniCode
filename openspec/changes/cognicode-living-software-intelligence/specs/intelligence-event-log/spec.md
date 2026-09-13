# Intelligence Event Log Specification

## Purpose

Define append-only causal history for software intelligence operations.

## Requirements

### Requirement: Causal events

Every analysis/proposal/promotion event MUST identify actor and MAY identify caused_by/correlation. Committed events SHALL be immutable.

#### Scenario: Finding causal chain

- GIVEN a finding triggered by a source delta
- WHEN causal chain is requested
- THEN source delta, fact commit, analysis and finding events are traversable in order

### Requirement: Batch-friendly log

Large FactDelta payloads SHOULD be referenced by content-addressed artifact instead of one event per Fact.

#### Scenario: Large scan bounded events

- GIVEN a scan producing one million facts
- WHEN the commit is logged
- THEN the log may record bounded batch events referencing immutable digests rather than one million fact events
