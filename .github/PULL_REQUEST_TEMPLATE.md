# Pull request

All four sections below are **enforced**, not advisory. A PR that cannot answer
them cannot be reviewed.

## SDDK alignment

<!--
Work in this repository is tracked through SDDK, not through memory or a
branch name. State the work item and the cycle.

- Work item:
- Cycle:
- Base commit:

If the change is not aligned with a tracked work item, stop and open one first.
See docs/roadmap/ROADMAP.md.
-->

## What this changes

<!--
One paragraph. If the diff is doing more than one logical thing, split the PR:
one commit is one logical change.
-->

## Test evidence

<!--
Paste the commands you ran and their real output. Not a summary of what you
believe happened.

During development, run only the affected tests. The full release gate is
`merge-gate` in .github/workflows/pr-ci.yml and is required before merge; a
green subset is not a green gate, and saying "tests pass" from a subset is not
an acceptable claim.

If this is a bug fix, link the test that failed BEFORE and passes AFTER. A fix
without a regression test is an unverified claim.
-->

```
$ <command>
<output>
```

- [ ] The affected tests were run and their output is above
- [ ] This is a bug fix and a failing-before/passing-after test is linked
- [ ] I did not run the full release gate locally; CI runs `merge-gate`

## Release impact

<!--
Does this change a published contract or a shipped artifact?

- [ ] No — internal only, no version bump needed
- [ ] Yes — one of the below, and the version must be derived from the commit
      history, not chosen by hand

If yes, which:
- [ ] An MCP tool is added, removed, or renamed (`product/tools.json`)
- [ ] What an existing MCP tool returns changes
- [ ] A CLI command or flag changes
- [ ] A language is added, or moves between `supported` / `experimental`
- [ ] A profile's stability or installability changes
- [ ] A shipped binary or the release contract changes

Version impact, derived from history:
- [ ] breaking -> MAJOR
- [ ] feat -> MINOR
- [ ] fix -> PATCH
-->

## Checklist

- [ ] Commit messages follow Conventional Commits (`type(scope): description`)
- [ ] Commits are atomic
- [ ] I read `AGENTS.md` and `CONTRIBUTING.md`, and this PR follows both
- [ ] No second source of truth was introduced for an existing contract
- [ ] `Partial` / `Unknown` / `Unsupported` / `Failed` results are still
      distinguishable, and no partial result is presented as complete
- [ ] `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass
- [ ] Generated artifacts under `product/` were regenerated if the change affects
      them, and the `--check` variants are clean
