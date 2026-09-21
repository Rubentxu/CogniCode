# Portable Execution Backends Specification

## Purpose

Separate execution/isolation capabilities from CogniCode's canonical analysis model so Linux, macOS and Windows hosts can use the same product semantics with different execution mechanisms.

## Requirements

### Requirement PEX-001: Execution backend boundary

Commands that execute project code MUST run through an execution-backend contract that exposes capabilities and structured outcomes. Canonical Facts/Evidence/Policy authority MUST NOT depend on a concrete container engine.

#### Scenario: Backend substitution

- GIVEN identical ExecutionSpec input
- WHEN it is run through two conforming backends capable of the requested operation
- THEN both produce the same logical ExecutionOutcome schema
- AND backend-specific transport/process details do not leak into Evidence authority.

### Requirement PEX-002: Initial backend set is minimal

The first portable implementation MUST support a native process backend and a Podman isolation backend. Additional Docker/remote backends MUST be justified by a concrete consumer or gap.

#### Scenario: No speculative backend proliferation

- GIVEN no workload requires Docker-specific or remote behavior
- WHEN e75 closes
- THEN Native + Podman conformance is sufficient
- AND no backend exists solely for feature-count parity.

### Requirement PEX-003: Isolation is capability-driven

A work item requiring isolation MUST NOT silently downgrade to native execution when an isolation backend is unavailable.

#### Scenario: Required isolation unavailable

- GIVEN a Trial requires isolated execution
- AND the host has no compatible isolation backend
- WHEN execution is requested
- THEN the result is explicit Unavailable/Insufficient capability
- AND the trial/policy path cannot interpret it as PASS.

### Requirement PEX-004: Host-specific Podman strategy

Podman-backed isolation MAY be direct on Linux and VM-backed on macOS/Windows, while preserving one logical backend contract.

#### Scenario: macOS Podman Machine

- GIVEN macOS with a healthy Podman Machine
- WHEN an isolated work item executes
- THEN CogniCode uses the Podman backend through the managed machine
- AND the WorkResult/EvidenceBundle does not encode VM implementation details as semantic project facts.

### Requirement PEX-005: Existing hardened sandbox remains an oracle

The existing Linux/systemd/Quadlet sandbox MUST remain valid as hardened release-validation infrastructure, but Quadlet/systemd specifics MUST NOT become required by the portable execution domain contract.

#### Scenario: Windows host

- GIVEN Windows cannot run Linux systemd Quadlets natively
- WHEN portable isolation is configured
- THEN the host may use Podman Machine/compatible isolation
- AND existing Linux sandbox scenarios remain usable as an independent CI/release oracle.

### Requirement PEX-006: Execution results are evidence inputs, not authority

Execution backends produce observations/results. They MUST NOT mint canonical Fact authority, promotion authority or policy approval by themselves.

#### Scenario: Successful command without required evidence

- GIVEN a command exits zero
- WHEN required EvidenceBundle slots remain missing
- THEN PolicyGate may return InsufficientEvidence
- AND backend success alone cannot promote/apply a proposal.

### Requirement PEX-007: Remote execution is a seam, not initial scope

The execution contract SHOULD preserve enough abstraction for future remote execution, but e75 MUST NOT introduce distributed workers, queueing or transport protocols without a separate requirement and measured need.
