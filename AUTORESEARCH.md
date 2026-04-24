# autoresearch — CogniCode Self-Improvement Loop

One day, code intelligence tools will improve themselves. This is that story.

## Setup

To start a new autoresearch session:

1. **Agree on a run tag**: based on today's date (e.g. `apr22`). The branch must be `feat/autoresearch-loop`.
2. **Build the release binary**: `cargo build --release -p cognicode-mcp`
3. **Run the baseline**:
   ```bash
   cargo test --workspace 2>&1 | grep -E "^test result"
   ./target/release/sandbox-orchestrator run sandbox/manifests/rust_fixture.yaml --results-dir autoresearch/baseline --jsonl
   ```
4. **Record the baseline**: note test count and sandbox pass rate in `autoresearch/results.tsv`
5. **Confirm and go**: you are now autonomous

## What you CAN do
- Modify any source file in `crates/cognicode-core/src/` or `crates/cognicode-mcp/src/`
- Use CogniCode MCP tools on the CogniCode codebase itself (eat your own dogfood)
- Add new tests
- Refactor existing code

## What you CANNOT do
- Modify `AUTORESEARCH.md` (this file) — it is read-only for you
- Add new crate dependencies without good justification
- Remove existing tests (you can modify them, but not delete)
- Change the sandbox orchestrator binary interface

## The Goal

**Improve CogniCode's code quality while keeping all 832 tests green.**

Improvement targets (pick one per experiment):
- **Simplicity**: Remove unnecessary complexity. Delete code while preserving behavior. Best kind of improvement.
- **Performance**: Make graph building faster, queries snappier. Measure before and after.
- **Reliability**: Fix edge cases, improve error messages, add defensive checks.
- **Coverage**: Add tests for untested paths. But only if they test real behavior, not just coverage theater.

## Using CogniCode on CogniCode

You have a secret weapon: CogniCode's own tools. Before making any change:

```
1. build_graph → understand the architecture you're about to change
2. analyze_impact → know the blast radius before touching anything
3. get_hot_paths → identify critical functions (high fan-in = dangerous to break)
4. check_architecture → detect cycles you might introduce
5. trace_path → verify the change won't break execution chains
6. get_complexity → find overly complex functions that need simplification
```

This is the key differentiator from generic autoresearch: you have structural intelligence about your own code.

## The Experiment Loop

LOOP FOREVER:

1. **Pick a target**: Look at the codebase using CogniCode tools. Find something to improve.
2. **Analyze impact**: Use `analyze_impact` on the symbol you're about to change.
3. **Make the change**: Implement your improvement.
4. **Run tests**: `cargo test --workspace 2>&1 | grep -E "^test result"`
5. **Run sandbox**: `./target/release/sandbox-orchestrator run sandbox/manifests/rust_fixture.yaml --results-dir autoresearch/exp-NNN --jsonl`
6. **Evaluate**:
   - If ALL tests pass AND sandbox passes → KEEP
   - If any test fails → DISCARD, `git checkout .` to revert
   - If sandbox crashes → fix and retry ONCE, then DISCARD if still broken
7. **Log the result** in `autoresearch/results.tsv`
8. **If KEEP**: `git add -A && git commit -m "autoresearch: <description>"`
9. **If DISCARD**: revert all changes, learn from the failure, try something else

NEVER STOP. The loop runs until the human interrupts you.

## Simplicity Criterion

All else being equal, simpler is better:
- A 0.001 improvement that adds 20 lines of hacky code? Not worth it.
- A 0.001 improvement from deleting code? Definitely keep.
- Equal performance but simpler code? Keep.
- Equal performance but more complex code? Discard.

## Logging Results

Log every experiment to `autoresearch/results.tsv` (tab-separated):

```
commit	tests_passed	sandbox_passed	health_score	status	description
```

Columns:
1. git commit hash (short, 7 chars) — use "0000000" for discards
2. total tests passed (e.g. 832)
3. sandbox scenarios passed (e.g. 12/12)
4. health score (0-100, from sandbox summary)
5. status: `keep`, `discard`, or `crash`
6. description of what was attempted

Example:
```
commit	tests_passed	sandbox_passed	health_score	status	description
a1b2c3d	832	12/12	95.0	keep	baseline (no changes)
0000000	831	11/12	88.0	discard	tried to inline ensure_graph_built (broke 1 test)
b2c3d4e	832	12/12	95.5	keep	removed duplicate rebuild in build_lightweight_index
```

## Safety Rules

1. **Never force push** to the branch
2. **Always run tests before committing** — no exceptions
3. **If stuck on 3 consecutive discards**, take a step back, use CogniCode tools to re-analyze, find a different target
4. **If the codebase feels stuck**, try a different category: if performance experiments keep failing, try simplicity or coverage
5. **Respect the test count**: it should stay the same or grow, never shrink

## What Makes This Different From Karpathy's Autoresearch

1. **Self-analysis**: We use CogniCode to analyze CogniCode. The agent has structural intelligence about its own code.
2. **Multi-metric**: Not just one number. Tests + sandbox + health score + architecture quality.
3. **Simplicity focus**: Code deletion is celebrated, not just feature addition.
4. **SDD integration**: The agent can use the full SDD workflow (explore → propose → spec → design → tasks → apply → verify) for complex changes.
