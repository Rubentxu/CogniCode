# Proposal — e75 LSI Portable Execution / Sandbox

> Planned cycle; depends on e74 portable runtime and M9 Trial semantics.

## Goal

Make isolated execution a discovered capability with interchangeable backends instead of treating Linux/systemd/Quadlet as the product-level execution contract.

## In scope

- execution-backend boundary with structured capabilities/outcomes;
- `NativeProcessBackend` for explicitly safe work;
- `PodmanBackend` for isolated work;
- Linux direct Podman and macOS/Windows VM-backed Podman conformance;
- no silent downgrade from required isolation to native execution;
- reuse EvidenceBundle/PolicyGate semantics for execution outcomes.

## Out of scope

- distributed scheduler/queue;
- remote worker protocol;
- custom CogniCode VM appliance;
- speculative Docker backend;
- automatic enabling/installing of WSL/Hyper-V/Podman.

## Exit condition

The same logical isolated Trial/Work execution contract runs with a conforming Podman backend across representative Linux/macOS/Windows hosts while preserving fail-closed evidence/policy behavior.

## Governing umbrella spec

`portable-execution-backends` in the Living Software Intelligence umbrella.
