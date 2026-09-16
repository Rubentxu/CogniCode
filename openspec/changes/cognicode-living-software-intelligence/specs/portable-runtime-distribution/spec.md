# Portable Runtime & Distribution Specification

## Purpose

Define CogniCode as a native-first, platform-aware product whose normal analysis capabilities do not depend on a container runtime, VM or Linux compatibility layer.

## Requirements

### Requirement PRT-001: Native-first core

`cogh` and the platform-neutral CogniCode analysis/MCP runtime MUST execute natively on supported host targets without requiring Podman, Docker, WSL or a VM.

#### Scenario: Windows user without container runtime

- GIVEN a supported Windows x86_64 host with no Podman/Docker/WSL execution backend
- WHEN the user installs the `core` profile
- THEN `cogh`, MCP startup, parsing, canonical Fact production and read-only analysis remain available
- AND isolated-execution features report unavailable capability rather than making core startup fail.

### Requirement PRT-002: Supported release targets

The release contract MUST publish coherent artifacts for Linux x86_64/aarch64, macOS x86_64/arm64 and Windows x86_64, or explicitly mark a target unavailable without silently substituting another platform artifact.

#### Scenario: Platform-specific bundle resolution

- GIVEN a bundle registry containing multiple platform manifests
- WHEN `cogh` resolves the current host
- THEN it selects exactly the matching `BundleManifest::Platform`
- AND artifact checksums are verified before activation.

### Requirement PRT-003: Platform semantics behind adapters

OS-specific path, shim/launcher, process and filesystem behavior MUST be isolated behind platform adapters/capabilities rather than becoming domain policy or scattered target checks in analysis code.

#### Scenario: Windows shim semantics

- GIVEN Windows cannot safely use the same symlink assumptions as Unix
- WHEN an executable shim is installed
- THEN a Windows-appropriate launcher/link strategy is selected by the platform adapter
- AND domain/application callers use the same logical shim contract.

### Requirement PRT-004: Capability-oriented doctor

`cogh doctor` MUST distinguish product health from optional execution capabilities and MUST provide actionable remediation without automatically enabling invasive OS features.

#### Scenario: Core healthy, isolation absent

- GIVEN native CogniCode is healthy and no isolation backend exists
- WHEN `cogh doctor` runs
- THEN core/MCP analysis is PASS
- AND isolated execution is reported unavailable with supported options
- AND the command does not silently enable WSL, Hyper-V, download a VM or install Podman.

### Requirement PRT-005: Release pipeline parity

The tracked release pipeline MUST test/package the supported target matrix rather than documenting targets that are never produced.

#### Scenario: Release contract and workflow agree

- GIVEN the set of supported platforms in the release contract
- WHEN release configuration is inspected
- THEN every required platform has a build/package/verification lane
- OR an explicit temporary waiver with owner, reason and expiry/trigger is recorded.

### Requirement PRT-006: Installer machinery does not own CogniCode lifecycle semantics

An external packaging tool MAY build/sign/package/bootstrap `cogh`, but version profiles, plugin/skill composition, project locks and CogniCode bundle semantics remain owned by `cogh` contracts.

#### Scenario: Packaging implementation replacement

- GIVEN the project replaces GitHub workflow packaging with `dist` or another tool
- WHEN the installer implementation changes
- THEN existing `BundleManifest`, profile, lock and IDE-adapter semantics remain unchanged unless separately evolved by contract.
