## Exploration: sandbox-precision

### Current State
The sandbox runner already has a mature execution loop, but the failing
`rust.yaml`, `python.yaml`, `js.yaml`, and `ts.yaml` manifests are exposing a
mix of three different problems: manifest/workspace mismatches, fixture/setup
infrastructure bugs, and scoring/classification gaps.

- **`rust.yaml` is not failing because “Rust support is broken” end-to-end.**
  It is failing mostly because the orchestrator picks the generic Rust fixture
  (`sandbox/fixtures/rust`) when `repo` is omitted, even though the manifest is
  clearly written for the real `serde` repo. That makes paths like
  `src/ser.rs` and validation commands like `rustfmt --check src/` run in the
  wrong workspace.
  - `read_file_raw` / `read_file_outline`: manifest tests raw/outline reads of
    `src/ser.rs`; actual MCP response is file-not-found; result is misclassified
    as `protocol_violation` because `mcp_error` maps to `ProtocolViolation`.
  - `search_content`: tests workspace text search, but the manifest sends
    `query` while MCP `SearchContentInput` requires `pattern`; this is a
    manifest schema bug, again misclassified as `protocol_violation`.
  - `extract_symbols` / `find_references`: these are analysis scenarios, but
    the orchestrator still runs post-tool validation for non-`read_only`
    scenarios. Because file workspaces are converted to the parent directory,
    validation runs from `.../src` and commands like `rustfmt --check src/`
    resolve to `src/src`, producing `syntax_failure`. This is an orchestrator
    cwd/validation bug.
  - `safe_refactor_rename_preview`: marked `expected_fail`, and it does fail,
    but only because the target file is missing. The pass is therefore not
    informative.
  - `safe_refactor_rename_concrete`: reported as `preexisting_fail`, but the
    repo is not preexisting-broken; baseline validation fails only because the
    orchestrator runs the scenario in the wrong workspace.
  - `edit_file_syntax_rejected`: reported as `expected_fail`, but the result is
    also not meaningful; the validation command uses `cargo check || true`, so
    syntax rejection is not being measured directly.
  - `path_safety_rejection`: the underlying MCP response correctly says
    `Path outside workspace`, but the final result is collapsed to generic
    `expected_fail`, so the taxonomy loses the fact that path safety actually
    worked.

- **`python.yaml` failures are primarily sandbox infrastructure / workspace
  selection failures.**
  The manifest targets the real `click` repo (`src/click/__init__.py`), but the
  orchestrator again prefers the generic Python fixture when `repo` is omitted.
  That creates nonexistent workspaces like `/tmp/.../src/click`, so the MCP
  server never initializes and read-only scenarios become `no_result` /
  `sandbox_infra_failure`.
  - `read_file_raw`, `read_file_outline`, `search_content`: no response at all;
    `server_startup_ms` stays `0`; this is consistent with MCP startup failing
    because `--cwd` points to a nonexistent directory.
  - `extract_symbols`: validation fails immediately with `os error 2`; again,
    this is bad workspace setup, not Python parsing quality.
  - `edit_file_concrete`: reported as `preexisting_fail`, but the “preexisting”
    failure is just baseline validation running in a nonexistent workspace.
  - `edit_file_syntax_rejected` and `path_safety_rejection`: both get credit as
    `expected_fail`, but for the wrong reason — validation fails before the real
    scenario is exercised.
  - This does **not** look like “MCP cannot handle Python files”; an older run
    (`20260409T191538`) shows `python_read_file_raw_default` passing, which
    strongly suggests a regression in orchestrator workspace resolution rather
    than parser capability.

- **`js.yaml` currently hides real `edit_file` problems behind green outcomes.**
  Latest results show 5 normal passes and 4 expected-fails, but the artifacts
  reveal that `edit_file` often returns `{"applied":false,...}` because the
  fixture content no longer matches the manifest’s `old_text`.
  - `edit_file_concrete`: intended to rename `calculateArea` to
    `computeRectangleArea`; MCP says `No matches found for old_string`, yet the
    scenario still ends as `pass` because validation passed and the orchestrator
    never treats `applied:false` as a failure for `expected_outcome: pass`.
    This is a real scoring/orchestration bug.
  - `edit_file_regression`: intended to change `*` to `+`; MCP again says no
    match. The scenario still lands in `expected_fail`, but not because the edit
    ran — the checked-in fixture is already contaminated and exports
    `calculateArea` after renaming the function to `computeRectangleArea`, so
    tests fail on `ReferenceError`.
  - `edit_file_syntax_rejected`: also reports no match because `js-hello` is
    already checked in with the broken syntax. The expected failure is real, but
    it is no longer testing edit rejection.
  - `path_safety_rejection`: underlying MCP response is correct (`Path outside
    workspace`), but the global JS validation command uses `cd /workspace && ...`
    even though validation runs on the host temp dir, so the test stage fails
    for the wrong reason. This is a manifest validation bug.

- **`ts.yaml` has both manifest bugs and a real fixture-copy infrastructure bug.**
  - `list_files_module`: manifest sets `workspace: ts-hello` and also passes
    `path: ts-hello`, so `list_files` resolves `.../ts-hello/ts-hello`; MCP
    returns “Directory not found,” but the result is again mislabeled as
    `protocol_violation`.
  - `edit_file_concrete`, `edit_file_regression`, `edit_file_syntax_rejected`:
    all are dominated by a TypeScript setup bug, not by the edit itself. The
    copied temp workspace cannot run `node_modules/.bin/tsc` because
    `copy_dir_recursive()` converts npm symlinks into regular files. The copied
    `.bin/tsc` script then tries to resolve `../lib/tsc.js` relative to `.bin/`,
    which does not exist, causing `Cannot find module '../lib/tsc.js'`.
    These are sandbox infrastructure failures that currently surface as
    `preexisting_fail` or generic `expected_fail`.
  - `path_safety_rejection`: here the underlying tool rejection is correct and
    validation passes; the taxonomy still collapses it to generic
    `expected_fail` instead of preserving `path_safety_rejection`.

- **Failure classification is materially overstating “protocol violations.”**
  In `determine_failure_class()`, both `mcp_error` and real
  `protocol_violation` map to `FailureClass::ProtocolViolation`. That means
  ordinary tool-level errors — file not found, bad input schema, directory not
  found — are being reported as stdout/protocol contamination. The label is
  therefore unreliable in current summaries.

- **General manifest correctness coverage is weak.**
  `rust.yaml`, `python.yaml`, `js.yaml`, and `ts.yaml` use
  `expected_outcome: pass|expected_fail` plus validation stages, but they define
  no `ground_truth` and no `metrics`. In practice, these suites are mostly
  checking “did it run / did validation stay green / did it fail when expected,”
  not “was the tool output correct.”

- **The scoring system has real KPI scaffolding, but most dimensions are still
  placeholders in normal scenario runs.**
  - `correctitud` is real only when a scenario has `ground_truth` and the tool
    has a matcher route.
  - `latencia` is real per scenario.
  - `escalabilidad` is currently computed with `workspace_size_kb = 0` inside
    `score_scenario()`, so normal runs default to a neutral placeholder.
  - `consistencia` is a single-sample heuristic (`95` for sub-2ms, else `75`).
  - `robustez` is always `75` in normal scoring because
    `compute_robustness_score(0, 0)` is hardcoded.
  - The aggregate MCP Health Score exists, but for many runs it is dominated by
    placeholders and by whichever scenarios happened to include ground truth.

- **Regression tracking exists, but only at coarse aggregate level.**
  `history.rs` stores run-level dimension averages and computes trends, and the
  report path can print a health score trend. However, per-scenario baseline
  regression comparison is still TODO: `compute_regressions()` explicitly says it
  cannot compare scenario-by-scenario because it only loads a `Summary`.

### Affected Areas
- `sandbox/manifests/rust.yaml` — real-repo Rust scenarios with bad path/schema assumptions and no correctness ground truth.
- `sandbox/manifests/python.yaml` — real-repo Python scenarios currently routed into the wrong fixture workspace.
- `sandbox/manifests/js.yaml` — JS validation commands and edit expectations drifted from fixture reality.
- `sandbox/manifests/ts.yaml` — duplicated relative paths plus TypeScript validation that depends on preserved npm symlinks.
- `src/bin/sandbox_orchestrator.rs` — fixture-vs-repo selection, workspace cwd derivation, validation execution, outcome classification, and failure mapping.
- `src/sandbox_core/manifest.rs` — actual manifest schema: `ground_truth`, `metrics`, `preview_only`, `variant`, validation overrides, and timeout inheritance.
- `src/sandbox_core/scoring.rs` — dimension scoring, health score math, benchmark stats, and the current placeholder dimensions.
- `src/sandbox_core/history.rs` — trend tracking, regression alerting, and improvement ranking at run level.
- `src/sandbox_core/failure.rs` — failure taxonomy semantics and CI-blocking rules.
- `src/application/services/file_operations.rs` — actual `edit_file`, `search_content`, and `list_files` contracts; explains why schema mismatches and `applied:false` happen.
- `sandbox/fixtures/javascript/js-mutation/index.js` and `sandbox/fixtures/javascript/js-hello/hello.js` — contaminated JS fixtures that no longer match the manifest `old_text` assumptions.
- `sandbox/fixtures/typescript/ts-mutation/` and `sandbox/fixtures/typescript/ts-hello/` — TS fixtures whose copied `.bin/tsc` breaks when symlinks are flattened.

### Approaches
1. **Fix workspace and manifest semantics first** — make orchestrator choose repo workspaces when manifests target real repos, normalize file-vs-directory cwd rules, and repair manifest argument names/paths.
   - Pros: Removes most false negatives immediately; turns Rust/Python failures into meaningful tool tests.
   - Cons: Requires touching both orchestrator logic and several manifests.
   - Effort: Medium.

2. **Separate real protocol failures from normal tool errors** — stop mapping generic `mcp_error` to `ProtocolViolation`, and preserve path-safety/path-traversal outcomes even for `expected_fail` scenarios.
   - Pros: Makes KPI reports trustworthy; aligns taxonomy with its documented meaning.
   - Cons: Will change historical failure distributions and dashboards.
   - Effort: Medium.

3. **Harden fixture infrastructure** — preserve symlinks when copying fixtures, reset or regenerate contaminated fixtures, and avoid host-specific validation commands like `cd /workspace`.
   - Pros: Fixes TypeScript validation, restores JS mutation signal quality, and makes reruns reproducible.
   - Cons: Requires careful fixture cleanup and copy semantics changes.
   - Effort: Medium.

4. **Upgrade scoring from “didn’t crash” to real KPIs** — add ground truth/metrics to general manifests, fail `pass` scenarios when `edit_file.applied=false`, and replace placeholder dimension values with measured data.
   - Pros: Produces precise correctness and health metrics instead of mostly binary status.
   - Cons: More design work; some tools still need explicit expected outputs.
   - Effort: High.

### Recommendation
Start with **Approach 1 + Approach 2**, then immediately do **Approach 3**, and
only after that tighten KPIs with **Approach 4**.

Concretely, the best sequence is:
1. Fix repo-vs-fixture selection and workspace cwd rules in the orchestrator.
2. Repair manifest schema/path bugs (`query`→`pattern`, duplicated `ts-hello`,
   validation commands that assume `/workspace`).
3. Split `mcp_error` from true protocol contamination and preserve specific
   expected failure classes like path safety.
4. Preserve symlinks while copying fixtures and clean the contaminated JS/TS
   fixtures.
5. Add `ground_truth` and `metrics` to the general manifests so the final KPI is
   based on correctness, not just execution/validation status.

### Risks
- Changing workspace resolution may affect older fixture-based manifests that were implicitly relying on today’s fallback order.
- Reclassifying `mcp_error` will make historical “protocol_violation” counts drop sharply, which may look like a metric discontinuity.
- Cleaning fixture contamination without pinning a reset mechanism could let future runs drift again.
- Preserving symlinks in copied fixtures must be done carefully across platforms.
- Adding strict correctness assertions to the general manifests will initially lower pass rates before they improve measurement quality.

### Ready for Proposal
Yes — the problem is now clear enough to propose as a focused change with four
workstreams: (1) workspace/manifest correctness, (2) failure taxonomy accuracy,
(3) fixture infrastructure integrity, and (4) KPI/ground-truth expansion.
