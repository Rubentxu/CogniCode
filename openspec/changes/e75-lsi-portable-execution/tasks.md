# Tasks — e75 LSI Portable Execution / Sandbox

> Planned decomposition. Refine during e75 design.

## WU1 — Execution capability boundary

- characterize current WorkExecutor/TrialExecutor/sandbox seams;
- define the minimal execution backend/capability contract;
- implement NativeProcessBackend only for work explicitly allowed to run natively.

## WU2 — Podman isolation backend

- adapt existing hardened sandbox mechanics without exposing Quadlet/systemd in the domain contract;
- support direct Linux Podman and VM-backed host connectivity;
- keep backend outcomes structured and evidence-neutral.

## WU3 — Cross-platform conformance + adversarial isolation

- representative Linux/macOS/Windows acceptance;
- required-isolation-without-backend => unavailable/insufficient, never native fallback;
- command success with incomplete required evidence => InsufficientEvidence, never PASS;
- existing Linux external sandbox remains green.

## Closure gate

Isolation is portable by capability and backend, while core CogniCode remains native-first and backend success cannot mint authority.
