---
name: cognicode-pr-review
description: >
  Review a pull request or diff using CogniCode to locate changed symbols,
  trace their consumers and usages, assess architectural impact, and verify
  against tests and CI. Trigger: when an agent receives a PR link, a diff,
  or a request to review changes. Applies to: any PR, branch, or commit
  range where CogniCode can index the codebase.
license: MIT
metadata:
  version: "1.0.0"
  maturity: stable
  author: CogniCode Team
  homepage: https://github.com/Rubentxu/CogniCode
---

# PR Review guided by CogniCode

This skill reviews a PR or diff using CogniCode's graph and symbol tools.
It follows a six-step workflow: diff → changed symbols → consumers/usages →
impact → architecture → tests/CI evidence → report.

Use this skill when you have access to a CogniCode MCP server or CLI
connected to the repository under review.

## Prerequisites

- A diff to review (commit range, branch diff, or PR number).
- CogniCode connected via MCP or CLI (`cognicode` binary available).
- Git access to resolve the diff.

If CogniCode is unavailable, fall back to direct `git diff` analysis and
manual symbol lookup. State the limitation explicitly.

## Step 1 — diff

Resolve and capture the diff.

```bash
git diff <base>...<head>
git log <base>..<head> --oneline
```

Record:

- the commit range and what it claims to do;
- the number of changed files and approximate scope;
- any linked issue or PR description.

If the diff is empty, stop here and report no changes to review.

**Done when:** the diff is captured, its scope is bounded, and the
commit intent is recorded.

## Step 2 — changed symbols

Locate the symbols that changed in the diff.

Use CogniCode to find symbols in modified files:

```bash
cognicode index outline <changed-file>
cognicode index query <symbol-name>
```

For each significant change, identify the symbol name, its kind (function,
type, module, constant), and its file and line range.

If the diff touches many files, prioritize:

1. public API changes (exports, public functions, type definitions);
2. changes in core or domain modules;
3. changes flagged by the PR author or reviewers.

**Done when:** you have a list of changed symbols with file:line locations.

## Step 3 — consumers and usages

For each changed symbol, find where it is used.

Use CogniCode's navigation tools:

```bash
# find all usages of a symbol (takes a name, and fails loudly if it cannot)
cognicode find-usages <symbol>

# find usages of whatever is at a position: navigate references takes
# file:line:column, not a symbol name
cognicode navigate references <file:line:column>

# trace call hierarchy
cognicode graph hierarchy <symbol>

# check for call chains between changed symbols
cognicode graph trace-path <caller> <callee>
```

This reveals the blast radius of each change. A symbol used in many places
has wider impact than an isolated one. Distinguish:

- **direct usages** — places that call or reference the symbol directly;
- **indirect usages** — callers of the direct users (second-degree impact);
- **test usages** — test files that cover the symbol.

Record the full usage graph for symbols that appear in:
- the public API surface;
- hot paths identified by the project;
- CI configuration or build scripts.

**Done when:** the usage graph for each significant symbol is captured.

## Step 4 — impact

Assess what could break when these symbols change.

From the usage graph in step 3, evaluate:

- **Breaking change risk** — does the symbol have a public contract? Is the
  change signature-compatible?
- **Transitive impact** — if a low-level symbol changes, which mid-level
  modules depend on it?
- **Data flow** — does the symbol carry data between modules? Does the
  change affect the data shape?

Use CogniCode's complexity and entry-point tools to find whether the
changed symbols are on critical paths:

```bash
# find entry points (no callers in the observed graph)
cognicode graph entry-points

# find hot paths (high fan-in, high fan-out)
cognicode graph hot-paths

# analyze impact of a change
cognicode graph impact <symbol>
```

A symbol on a hot path or entry point has higher impact than one in an
isolated utility.

State the impact level explicitly: `HIGH`, `MEDIUM`, or `LOW`.
For each `HIGH` or `MEDIUM` finding, record which files and tests are
likely affected.

**Done when:** impact is rated and the affected file list is bounded.

## Step 5 — architecture and tests/CI evidence

Verify the findings against the codebase's actual state.

### Architecture

Check whether the change respects the project's architectural rules:

- Does the diff introduce a cross-module dependency that the ADRs or
  architecture constraints forbid?
- Is the layering respected (domain must not import infrastructure, etc.)?
- Are new public APIs in the right module?

Use CogniCode to navigate to the relevant modules and verify the actual
imports and call patterns, not just the stated intent.

### Tests

Locate and examine tests that cover the changed symbols:

```bash
# find test files that reference a changed symbol
cognicode find-usages <symbol>  # look for test/ or _test files
```

Read the actual test content to verify:
- The test exercises the changed behavior, not just the type signature.
- The test asserts the right properties.
- New behavior is covered by a new or updated test.

If the diff removes or changes behavior without updating tests, flag it.

### CI evidence

Check the actual CI runs for the PR, not the workflow configuration file.
A passing CI workflow file does not prove the change passed CI — the run
log does.

```bash
gh run list --branch <branch> --limit 5
gh run view <run-id> --log
```

If CI ran and passed, note the run ID, job, and conclusion.
If CI did not run, flag it.

**Done when:** architecture violations are noted, test coverage is
verified, and CI evidence is captured or flagged as absent.

## Step 6 — report

Deliver a structured PR review.

```markdown
## Summary

<PR title or diff description>
Reviewed <N> commits across <F> files.
<N> symbols with HIGH impact, <N> with MEDIUM, <N> with LOW.

## Changed Symbols

| Symbol | File | Kind | Impact |
|--------|------|------|--------|
| Foo::bar | src/foo.rs | fn | HIGH |
| Baz | src/baz.rs | type | MEDIUM |

## Findings

### HIGH

1. **Symbol: `Foo::bar`** (`src/foo.rs:42`)
   - What changed: signature changed from `fn(a: u32)` to `fn(a: u32, b: u32)`
   - Evidence: <output from cognicode navigate references>
   - Impact: <usage count>, transitive callers: <list>
   - Tests: <covered / not covered> — <test file and line>
   - CI: <passed / failed / not run> — <run ID>

2. ...

### MEDIUM

...

## Architecture

<Any ADRs or architectural constraints violated>

## Recommendations

1. <actionable fix>
2. <actionable fix>
```

For each finding:
- State the symptom clearly.
- Show the evidence from CogniCode.
- Distinguish confirmed findings from hypotheses.
- Do not report an issue that you cannot back with evidence.

**Done when:** the report is complete with evidence traces for every
confirmed finding.

## Common mistakes

### Treating CI config as CI evidence

The workflow YAML file shows what _should_ run. The actual run log shows
what _did_ run and its result. Always verify the run, not the config.

### Reporting a finding from a stale graph

If the graph was built before the PR was created, it may not include the
new files. Rebuild the graph before drawing conclusions about usages:

```bash
cognicode index build
```

Then re-run the symbol and usage queries.

### Mixing up usage count with impact

Many usages of a symbol does not automatically mean HIGH impact.
An internal utility used in 50 places has MEDIUM impact if changing it
does not break any contract. An entry point used in 3 places can have
HIGH impact if those 3 callers are on critical paths. Assess both quantity
and architectural position.

## Skill-specific rules

- Never treat a CogniCode graph query as a substitute for reading the
  actual source code. Use the graph to navigate; verify by reading.
- Profiles (`core`, `reviewer`) limit which CogniCode tools are
  available. Check the active profile before planning which queries to run.
- If the MCP server is not connected, state the limitation and continue
  with `git diff` + grep as the fallback surface.
- The `reviewer` profile includes the MCP daemon needed for interactive
  navigation. Use `cogh doctor` to verify the MCP surface before starting.

## References

- `cognicode` skill — install verification and surface selection.
- `cognicode-mcp` skill — MCP tool reference and graph prerequisites.
- `cognicode-agent-hardness` skill — inspect, corroborate, constrain,
  execute, verify discipline for using CogniCode safely.
