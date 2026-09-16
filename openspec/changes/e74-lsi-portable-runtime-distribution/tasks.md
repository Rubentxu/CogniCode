# Tasks — e74 LSI Portable Runtime & Distribution

> Planned decomposition. Refine during e74 design; the umbrella spec owns requirements.

## WU1 — Characterize and close portability gaps

- inventory Linux-only assumptions in cogh/runtime/MCP/install paths;
- characterize path, shim/launcher, process and filesystem differences;
- establish supported target build checks without changing authority semantics.

## WU2 — Release and bundle parity

- produce/verify artifacts for the supported target matrix or explicit temporary waivers;
- align platform bundle resolution and checksum verification;
- spike packaging/signing/notarization tooling without moving lifecycle authority out of cogh.

## WU3 — Native install/doctor acceptance

- extend doctor capability model;
- test clean-host native core install/startup on representative Linux/macOS/Windows runners;
- adversarial checks: wrong-platform artifact, checksum mismatch, unavailable optional capability must fail clearly.

## Closure gate

No container runtime is needed for normal platform-neutral analysis, and the release matrix matches the contractual platform matrix.
