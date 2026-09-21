# Proposal — e74 LSI Portable Runtime & Distribution

> Planned cycle; do not start before M9/e71–e73 closes and the post-M9 entry gate is GREEN.

## Goal

Turn the existing multiplatform `cogh`/bundle intent into an actual native-first distribution contract for Linux, macOS and Windows.

## Why now

The repository already models multiple platforms and the `cogh` spec promises Linux/macOS/Windows, while the current release workflow produces Linux x86_64 musl artifacts only. Before self-hosting can compare platforms, CogniCode itself must be installable and runnable natively on them.

## In scope

- supported target matrix and explicit waivers;
- release/package/install verification for Linux x86_64/aarch64, macOS x86_64/arm64, Windows x86_64;
- deterministic `BundleManifest::Platform` selection;
- platform adapters for path/shim/process/filesystem mechanics;
- `cogh doctor` capability discovery;
- installer/signing/notarization spike, including evaluation of `dist`/equivalent as packaging machinery.

## Out of scope

- isolated project-code execution;
- Podman Machine orchestration;
- remote workers;
- replacing `cogh` lifecycle/profile/plugin/lock semantics;
- AI agents.

## Exit condition

A clean supported host can install the platform-appropriate core profile and run native CogniCode/MCP analysis without a container runtime. Release artifacts and the documented supported matrix agree.

## Governing umbrella spec

`portable-runtime-distribution` in the Living Software Intelligence umbrella.
