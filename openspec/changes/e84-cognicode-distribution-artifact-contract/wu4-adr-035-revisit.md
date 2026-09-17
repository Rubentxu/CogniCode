# e84 WU4 — ADR-035 Revisit (LOCAL — never pushed)

> Governance: `docs/adr/**` is local-only. This note records the proposed ADR-035
> change; the ADR edit itself stays in the working tree.

## What ADR-035 said

The original decision was to mirror asdf 1:1. `crates/cognicode-cli/src/bin/cogh.rs`
still carries that lineage in its header: *"Mirrors the asdf-vm pattern (ADR-035)."*

## Why that is now too strong

asdf 0.16 was a full Go rewrite that changed its interface — `asdf global`,
`asdf local`, and the old `asdf update` are gone. Copying a moving 1:1 target
guarantees drift, and the things we actually benefit from were never the command
vocabulary.

## Proposed replacement decision

```text
REUSE (the mechanics that make installs safe):
  immutable per-version directories
  shims
  per-project pinning
  atomic install semantics with rollback

DO NOT COPY:
  the generic plugin ecosystem
  asdf's exact command vocabulary
  shell/version-manager responsibilities (completions, global shell integration)
```

`cogh` is a **CogniCode lifecycle manager** that borrows proven install mechanics,
not a generic version manager and not an asdf clone.

## Status

Proposed. The ADR text is edited locally only. ADR-034 (`cognicode-distribution-package`)
and ADR-036 are reviewed in the same pass for the same over-fitting risk.
