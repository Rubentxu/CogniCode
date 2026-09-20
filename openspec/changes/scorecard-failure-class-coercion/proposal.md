# Proposal: scorecard-failure-class-coercion

## Intent

Resolve the silent `TypeError` abort in `sandbox/scripts/release_scorecard.py:gate_g5()`
that prevents the scorecard from completing whenever any `result.json` in the
runs directories carries a structured (non-string) `failure_class` value. This
bug is currently blocking the E31-G scorecard streak counter (Gate 2 of the
v1.0.0 pre-cut checklist): `current_streak = 1/3` with 10 history runs, of
which the most recent ones can no longer complete a fresh streak evaluation.

## Root cause

`sandbox/results/full_run/*` contains 43 `result.json` files with
`failure_class` serialized as a dict (specifically
`{"mcp_tool_error": {"tool_name": <name>, "error_message": <msg>}}`),
produced by `cognicode-sandbox::determine_failure_class` when the outcome is
`"mcp_error"`. The Rust enum `FailureClass::McpToolError { tool_name, error_message }`
serializes to a JSON object, not a flat string. The scorecard's
`_aggregate_results` (line 231-232) then attempts to use the dict as a dict
key in `failure_distribution`, raising `TypeError: cannot use 'dict' as a dict
key (unhashable type: 'dict')`. The aborts take down `gate_g5` and propagate
through the full scorecard run, so the streak counter never increments.

## Scope

**In scope:**
- Add a defensive coercion helper `_normalize_failure_class(fc)` to
  `release_scorecard.py` that flattens dict-shaped `failure_class` values to a
  canonical string (`"<outer_key>:<inner_tool>"` for the known
  `mcp_tool_error` shape; `<outer_key>` for other dict shapes).
- Apply the helper at the two read sites in `release_scorecard.py`:
  - line 231 (`_aggregate_results`)
  - line 1372 (the `oom/timeout` tool list in `gate_g5` family rollup)
- A regression test under `sandbox/scripts/tests/` covering both dict-shaped
  and string-shaped `failure_class` inputs.
- Document the assumption ("failure_class MUST be hashable string-equivalent;
  producer should also flatten in a follow-up cycle") in the module docstring.

**Out of scope:**
- Changing `cognicode-sandbox` Rust types (custom Serialize on `FailureClass`).
  This is a producer-side fix that belongs to a separate cycle
  (`sandbox-failure-class-flatten-bd`). The producer-side fix is not blocking
  the scorecard because this defensive read-site fix is sufficient.
- Re-running the streak counter (post-merge: orchestrator runs
  `just scorecard-streak` 2-3 times).
- Updating the streak history JSON; old runs keep their verdicts.

## Approach

Single file change (`sandbox/scripts/release_scorecard.py`) + one regression
test file (`sandbox/scripts/tests/test_failure_class_coercion.py`).

The coercion contract:

```python
def _normalize_failure_class(fc: object) -> str:
    """Coerce a failure_class field to a flat string key.

    Accepts: str, dict, None.
    Returns: a non-empty string suitable for use as a dict key.

    Dict shapes observed in the sandbox:
      {"mcp_tool_error": {"tool_name": <str>, "error_message": <str>}}
      → "mcp_tool_error:<tool_name>"

    Generic dict → "<outer_key>" (first key found).
    None or missing → "pass" (the historical default).
    """
    if fc is None:
        return "pass"
    if isinstance(fc, str):
        return fc
    if isinstance(fc, dict):
        if not fc:
            return "pass"
        outer_key = next(iter(fc.keys()))
        inner = fc[outer_key]
        if isinstance(inner, dict):
            tool = inner.get("tool_name")
            if isinstance(tool, str) and tool:
                return f"{outer_key}:{tool}"
        return str(outer_key)
    return str(fc)
```

## Risks

- **Behavior change risk**: any downstream consumer that already special-cases
  `"mcp_tool_error"` as a bare string would now see
  `"mcp_tool_error:trace_path"`, `"mcp_tool_error:find_references"`, etc.
  Mitigation: greps for `"mcp_tool_error"` literal in the repo show only
  producer-side Rust code; the only Python read sites are the two we are
  fixing. The new bucket names are strictly more informative, not breaking.
- **Hidden schema**: if the producer starts emitting a different dict shape
  in the future, the helper falls back to the outer key. This is documented
  in the function's docstring.

## Verification

- `python3 -m pytest sandbox/scripts/tests/test_failure_class_coercion.py -v`
  passes.
- `python3 sandbox/scripts/release_scorecard.py --runs sandbox/results/ci_smoke,sandbox/results/quality,sandbox/results/full_run --stability sandbox/results/stability.json --coverage-matrix sandbox/reports/coverage_matrix.yaml --output sandbox/results/scorecard_run`
  completes without `TypeError`. The output's `failure_distribution` contains
  the keys `"pass"` (e.g., 17 occurrences), `"expected_fail"` (5), and
  `"mcp_tool_error:<tool>"` for each of the 43 `mcp_tool_error` results
  (one entry per tool).
- `just scorecard-streak` completes end-to-end and increments
  `sandbox/results/scorecard_streak.json:current_streak`.

## Closure criteria

- 0 `TypeError` from `gate_g5()` with the current sandbox fixture.
- New regression test passes.
- `failure_distribution` populated correctly (count check: 17 pass + 5
  expected_fail + 43 mcp_tool_error:* = 65 entries).
- `cargo fmt --check` and `python3 -m py_compile sandbox/scripts/release_scorecard.py`
  clean.

## Note on prior cycle

This is the **first cycle** of the E33-follow-up scorecard-stabilization
track. It is a blocker for the v1.0.0 pre-cut checklist Gate 2 (E31-G
scorecard streak). The producer-side follow-up
(`sandbox-failure-class-flatten-bd`) is queued but is not on the critical
path; the read-site fix is sufficient to unblock the streak.

Roadmap-completion initiative: shape C blocker (v1.0.0 cut operational).