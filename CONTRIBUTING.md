# Contributing to CogniCode

Thanks for considering a contribution. This document describes the workflow
this project **actually enforces**, not a generic one. If a rule here conflicts
with your habit, the CI gate wins; if you think a rule here is wrong, open an
issue and say so — the rules are not sacred, but they are enforced.

## Before you start

Read `AGENTS.md` at the repository root. It is the contract this project works
under: it defines the engineering discipline, the security boundaries, and the
evidence standard. `CONTRIBUTING.md` tells you the workflow; `AGENTS.md` tells
you why the workflow exists.

Understand the architecture before changing it: the project is a
[hexagonal](https://en.wikipedia.org/wiki/Hexagonal_architecture_(software))
Rust workspace. `crates/cognicode-core` holds the domain, `crates/cognicode-mcp`
is the MCP adapter, `crates/cognicode-cli` is the CLI adapter. Application ports
must not depend on MCP adapters. If you are unsure where a change belongs, ask
in an issue first — a change in the wrong layer costs more than a question.

## License

By contributing you agree that your contribution is licensed under the same
terms as the project: `MIT OR Apache-2.0`, at the recipient's option. See
`LICENSE`, `LICENSE-MIT`, `LICENSE-APACHE`, and the decision record in
`docs/adr/ADR-053-cp1-license-publication.md`.

## Commit messages

Strict [Conventional Commits](https://www.conventionalcommits.org/):

```
type(scope): description
```

Allowed types: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `perf`.
Breaking changes are marked with `!` after the scope, or with a
`BREAKING CHANGE:` footer.

Commits must be **atomic**: one commit is one logical change. Do not mix a bug
fix with a refactor. Do not mix a documentation fix with a behaviour change. If
the review says "split this", split it.

## The test rules

**Run only the affected tests during development.** The full suite is
expensive; a change to one module should be verified by the tests that cover
that module. The full gate runs in CI; your local loop should not.

The most common commands, from the `justfile`:

```bash
just test-unit   # unit tests
just check       # cargo check across the workspace
just lint        # clippy
just fmt         # format
```

If your change touches the release contract, the MCP tool surface, or the
product artifacts, there are narrower checks that are the real gate:

```bash
python3 -m pytest -q scripts/product/    # product artifact projections
cargo deny check licenses                 # dependency and licence policy
```

**Every bug fix needs a test that failed before and passes after.** A fix
without a regression test is an unverified claim. The CI gate will not catch
this for you; review will.

**A passing selected suite is not a passing release gate.** Do not report "tests
pass" from a subset. The full gate is `merge-gate` in
`.github/workflows/pr-ci.yml`, and it is required for every change to `main`.

**When you run the full suite on a shared machine, give it a private
`TMPDIR`.** Cargo deletes "orphaned" `rustdoctest*` directories under `TMPDIR`
when it starts, so two concurrent cargo runs delete each other's doctest
argument files and the loser reports:

```
failed to load argument file: /tmp/rustdoctestXXXXXX/rustdoc-cfgs: No such file or directory
```

That is contention, not a broken doctest. Do not chase it in the code:

```bash
TMPDIR=/var/tmp/your-own-dir cargo test --workspace --no-fail-fast
```

**Count results after the test process exits, never from a log that is still
being written.** A count taken mid-run silently under-reports, and two people
counting the same run get two different totals. Aggregate once `cargo test`
has returned.

## Pull requests

Use the repository's PR template. It asks for four things, and all four are
enforced:

1. **SDDK alignment.** Work is tracked through SDDK, not through a mental note.
   A PR that is not aligned with a tracked work item cannot be reviewed
   meaningfully. See `docs/roadmap/ROADMAP.md`.
2. **Conventional Commit** subject and atomic diff.
3. **Test evidence** — the commands you ran and their real output, not a
   summary of what you believe happened.
4. **Release impact** — does this change a published contract, a CLI surface, an
   MCP tool name, or a shipped binary? If yes, it is a breaking change and the
   version must be derived from the commit history, not chosen by hand.

Every change to `main` goes through a pull request with a green
`merge-gate` check. Direct pushes to `main` are not accepted.

## Reporting bugs

Use the bug report template. The most useful bugs include:

- the exact version (`cognicode --version`, and the commit if you built from
  source),
- the platform,
- how you installed (`cogh`, `install.sh`, `mise`, or from source),
- a minimal reproduction: a repository, a command, and the wrong output.

If you are not sure whether something is a bug or a design decision, open a
discussion or an issue anyway. "I expected X" is useful information even when
the answer is "that is intended, here is why".

## What we will not merge

- Changes that introduce a second source of truth for an existing contract. If
  a value is already generated from code, the new place must not restate it.
- Changes that make a partial result look like a complete one. `Partial`,
  `Unknown`, `Unsupported` and `Failed` must stay distinguishable.
- Changes that quietly widen a published contract without a version bump.
- Generic documentation that describes a workflow this project does not have.
  If you write documentation, write what is true here.
