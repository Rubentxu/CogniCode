# e84 — Distribution Reality & Artifact Contract

## Intent

Turn the existing distribution prototype into **one coherent release contract**.

The pieces already exist: `cogh`, an atomic `InstallerTransaction` with a rollback
journal, `BundleManifest`, `COGNICODE_HOME`, platform adapters, a native GitHub
release matrix, IDE adapters. The problem is **consistency, not absence**:
the release producer and the `cogh` bundle consumer do not speak the same artifact
protocol, and nothing compares them.

## Bounded scope

This is a characterization and contract cycle. It reconciles what exists and fixes
the vocabulary. It does **not** publish a release (e85), implement the lifecycle
(e86), add channels (e87), or run Linux UAT (e88).

## Non-goals (WU10)

No Control Plane. No Backstage. No new MCP tools. No new analysis features. No
pack ecosystem. No AI agents. No architecture UX. No IDE adapter expansion. No
redesign of CogniCode core. No generic version manager.

## Deliverable

An agreed canonical artifact contract (WU2), a proven producer/consumer mismatch
(WU1), a full characterization (WU0), ownership and layering decisions
(WU3/WU7), a manifest-generation design (WU8), support tiers (WU9), and named
outcomes for the release-factory and channel decisions (WU5/WU6).
