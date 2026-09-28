---
name: cognicode-agent-hardness
description: >
  Teaches the `inspect → corroborate → constrain → execute → verify`
  workflow so an agent never treats CogniCode as an oracle of truth
  without evidence from contracts, tests, or runtime. Trigger: when an
  agent is about to make a change, assert a property, or dismiss a
  finding — and has CogniCode tools available. Applies to: any
  analysis, refactoring, architecture, or decision that relies on
  CogniCode output.
license: MIT
metadata:
  version: "1.0.0"
  maturity: stable
  author: CogniCode Team
  homepage: https://github.com/Rubentxu/CogniCode
---

# Agent Hardness — inspect, corroborate, constrain, execute, verify

CogniCode finds relationships, measures complexity, and traces paths through
code. It is a **search and measurement tool, not a truth authority**.
A finding from a graph query is a candidate, not a verdict. This skill
teaches the discipline to turn candidates into verified conclusions.

## The five-step workflow

```
inspect   — What does CogniCode report?
corroborate — Does a second, independent source confirm it?
constrain  — What is the safe action space given the evidence?
execute   — Perform the constrained action.
verify    — Did the action produce the expected result?
```

Skipping a step produces a fragile result. Treating inspect output as
verify output is the root cause of most agent errors with CogniCode.

## Step 1 — inspect

Run the CogniCode query. Record:

- the exact tool name and arguments used;
- the scope of the analysis (files, modules, time range);
- what the output claims;
- what the output **does not** claim (the complement).

Never assert a global property from a partial graph. `build_graph` omits
unread files. `find_usages` misses dynamically constructed names. `trace_path`
shows static connectivity, not runtime execution. State these limits
explicitly.

**Done when:** you have the raw output, its scope, and its silence.

## Step 2 — corroborate

Find a second, **independent** source that confirms the candidate finding.
Two queries from the same CogniCode graph are not corroboration — they
share the same parser state, the same cache, and the same observation
window.

Independent sources include:

- **Source code** — read the actual file and lines. Verify that the
  reported relationship exists in the text, not just in the graph.
- **Compilation** — `cargo build` or `cargo check` proves that a type
  relationship is real. A missing import is a build error, not a graph
  artifact.
- **Tests** — an existing test that exercises the path proves runtime
  behavior. A test that does not exist is not evidence of absence.
- **CI runs** — a GitHub Actions job log for the exact SHA proves
  that a gate passed. A passing CI configuration file does not.

If no corroboration source exists, state `UNVERIFIED` explicitly and
proceed with appropriate caution. Do not promote an uncorroborated
candidate to a confirmed finding.

**Done when:** corroborating evidence is found, or `UNVERIFIED` is
stated with the reason.

## Step 3 — constrain

Based on corroborated evidence, define the safe action space. An action
is constrained when:

- it affects only files/modules confirmed to be in scope;
- it respects declared authority (a read-only tool cannot write;
  a mutating tool requires explicit opt-in);
- it is reversible or has a rollback path documented before execution;
- it does not rely on a tool whose preconditions are unmet
  (e.g., a graph query requiring an up-to-date index when the index
  is stale).

Explicitly name what you are **not** doing, not just what you are doing.

**Done when:** the action description has explicit inclusions, exclusions,
and boundaries.

## Step 4 — execute

Perform the action within the constrained scope. Log:

- the exact command or change applied;
- the files modified;
- the estimated blast radius before execution;
- the actual blast radius observed after execution.

If the observed blast radius exceeds the constraint, stop and reassess
before continuing.

**Done when:** the action is complete and the blast radius is recorded.

## Step 5 — verify

Confirm the action produced the expected result through a second,
independent check — not the same command used in step 1.

Effective verifications:

- **Build** — `cargo build --all-targets` proves no compilation breakage.
- **Tests** — `cargo test` proves behavior is preserved. Run the
  **specific tests** for the affected area, not the full suite unless
  the blast radius justifies it.
- **CI** — check the actual CI run for the SHA, not just the workflow
  file.
- **Binary behavior** — run the affected binary against a known fixture
  and compare output.

If the verification fails, revert to the last confirmed-good state and
re-enter the workflow at step 1.

**Done when:** verification confirms the expected outcome, or the action
is reverted and the finding is re-evaluated.

## Common mistakes

### Treating graph output as runtime proof

`get_hot_paths` ranks functions by fan-in. Fan-in does not equal load,
frequency, or latency. A function called from many places is not
necessarily hot. Measure in a running system or from actual profiling
data before optimizing.

### Claiming absence from an empty result

`find_usages` returned nothing → "the symbol is unused." But if the
graph is stale, the symbol may be used in files that were not parsed.
Always verify with a build or grep over the actual source tree.

### Treating a JSON schema as a runtime contract

output_schema in the MCP catalog describes the shape of a response.
It does not prove that every response matches the schema, or that the
handler always returns the declared fields. Test with a real call.

### Using a cached graph as fresh evidence

A `build_graph` call from a previous session or a previous build may
omit files that changed since. Always check the timestamp or rebuild
before drawing conclusions from cached data.

### Confusing two queries with corroboration

`get_call_hierarchy` and `trace_path` on the same function use the
same graph. Running both and noting they agree is not corroboration —
it is the same observation twice. Corroboration requires a different
information channel.

## Skill-specific rules

- Never invoke `cognicode doctor` or `cogh doctor` as a verification
  step — doctor is a diagnostic report, not an assertion.
- When using MCP tools, verify the tool exists in the current
  capability catalog (`cognicode capabilities --format json`) before
  assuming it is available.
- Profiles (`core`, `reviewer`, `developer`) limit which tools are
  available. Check the active profile before planning an action.
- A tool declared `read` by the authority audit cannot write files even
  if the implementation appears to support it. Trust the authority
  declaration, not the implementation appearance.

## References

- `cognicode` skill — install verification and surface selection.
- `cognicode-mcp` skill — MCP tool reference and graph prerequisites.
- `cognicode-quality-investigator` skill — twelve-dimensional quality
  audit methodology.
