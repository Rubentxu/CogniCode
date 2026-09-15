# Spec — e41 — DFG conformance coverage

> Change: `e41-lsi-dfg-conformance-coverage` | Phase: specify | Date: 2026-09-15

## Scope

Close the gap that the M5 conformance corpus had no fixture of
`algorithm = "dfg"`. The DFG dispatch path already exists
(`application/program_analysis.rs:330`) and is wired into the
`dispatch` map (line 223) under the `program-analysis-server`
feature, but the conformance corpus never exercised it.

## Requirements

### Requirement: Conformance corpus includes DFG fixtures

**Given** the M5 conformance corpus registered in
`application::program_analysis::conformance::canonical_corpus()`

**When** the corpus is run with `--features program-analysis-server`

**Then** the corpus MUST include at least one fixture of
`algorithm = "dfg"` that exercises the basic emission contract
(function_id, cfg_digest, statements with id/defs/uses).

### Requirement: DFG dispatch arm is registered in the conformance dispatch map

**Given** the algorithm → AlgorithmId dispatch map in
`application::program_analysis::conformance::run_corpus()`

**When** the conformance corpus contains a `dfg` fixture

**Then** the map MUST include a `"dfg" => DFG.clone()` arm under
the `#[cfg(feature = "program-analysis-server")]` gate, mirroring
the `interproc_summary` arm.

### Requirement: DFG fixtures declare required params

**Given** the `DfgParams::validate` contract (requires `function_id`
and `cfg_digest` keys in the params object)

**When** a DFG fixture is added to the conformance corpus

**Then** the fixture's `params` JSON object MUST include both
`function_id` and `cfg_digest` keys with non-empty string values.

### Requirement: DFG dispatch is exercised by replay guard

**Given** the conformance corpus now includes DFG fixtures

**When** `replay_guard_is_byte_identical_for_entire_corpus` runs

**Then** the DFG fixtures MUST be included in the corpus and their
outputs MUST be byte-identical across two runs (D5 determinism).

## Non-goals

- Per-fixture digest pinning (covered by the existing `replay_guard`
  contract at the corpus level).
- Cross-producer fixtures (LSP + tree-sitter; deferred to M5+).
- Performance budget per-algorithm (DFG now contributes to the existing
  median/p95/max envelope automatically).
