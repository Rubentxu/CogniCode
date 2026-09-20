# Cycle p-c1fac1fea05615c6/clippy-style-lints-bd — Bounded clippy hygiene (style+trivial lints)

## Goal

Eliminate the small cluster of trivial/cosmetic clippy lints visible
in the workspace-wide run, bounded to mechanical edits across 5 files
without introducing any `#[allow(clippy::*)]` attribute. The cycle
keeps the operator's "no scope-creep / no waivers / no force-push /
conventional commits" guardrails from H4.7 / H5 phase 1.

## Scope (what this cycle changed)

Seven (7) trivial clippy warnings fixed across five (5) files, all
behaviour-preserving:

| File | Warning | Fix |
|---|---|---|
| `crates/cognicode-cli/src/cmd/doctor.rs` | `doc_list_item_overindented` × 4 | Re-indent the doc-list continuation lines |
| `crates/cognicode-cli/src/cmd/ide.rs` | `useless_vec` | Drop `let path = vec![...]` that was never iterated in the test |
| `crates/cognicode-cli/src/cmd/ide.rs` | `assert_eq with literal bool` | Replace `assert_eq!(x, true)` with `assert!(x, "{v:?}")` |
| `crates/cognicode-cli/src/cmd/ide.rs` | `match for single pattern` | Collapse `match &step { Step::X {..} => .., _ => {} }` to `if let` |
| `crates/cognicode-cli/tests/cogh_cli.rs` | `map_or can be simplified` | `map_or(false, \|c\| …)` → `is_some_and(\|c\| …)` |
| `crates/cognicode-core/src/domain/behaviors/class.rs` | `for_loop over single element` | Desugar to `let class = BehaviorClass::PureDerivation; assert!(…);` |
| `crates/cognicode-core/src/domain/findings/ports.rs` | `redundant_closure` | `.map(\|d\| EvidenceResolution::Known(d))` → `.map(EvidenceResolution::Known)` |

Five files, +16 / −18 lines total.

## Out of scope (recorded as candidates)

These trivial lints were inspected but NOT applied this cycle, with
the reasoning recorded in `release-manifest.md`:

- `if_statement_can_be_collapsed` at
  `crates/cognicode-core/src/application/self_hosting/acceptance.rs:51`
  was applied and reverted: the collapse re-uses `path` after a move
  through the first branch, which clippy's lint doesn't catch because
  both branches are at statement level.
- `variant_name_starts_with_enum_name` at
  `crates/cognicode-cli/src/cmd/release_contract.rs:106,108`
  (variants `Layer0Boot`, `Layer1Runtime`). The enum is
  `serde(rename_all = "kebab-case")` and these variants serialise as
  `layer0-boot` / `layer1-runtime`; renaming the variants breaks
  deserialisation in every consumer. Tracked for a future
  `clippy-serde-shaped-rename-bd` cycle that includes the consumer
  sweep.
- `derefed_type_is_same_as_origin` at
  `crates/cognicode-cli/src/cmd/installer_transaction.rs:128`
  requires a signature redesign (the function takes `mut warn_sink:
  Option<&mut Vec<u8>>` to allow test injection). Tracked for a
  `clippy-installer-signature-cleanup-bd` cycle.

## Verification (at HEAD `be61724b`)

- `cargo fmt --check`                                  : clean
- `cargo check --workspace --tests`                    : clean
- `cargo clippy -p cognicode-cli --bin cognicode-release --tests`
  : 71 → 52 warnings
- `cargo clippy --workspace --tests`
  : 75 → 63 warnings
- `cargo test -p cognicode-cli --bin cognicode-release` : 44/44 PASS
- `cargo test -p cognicode-cli --bin cogh` (with `TMPDIR=/tmp/cognicode-h4.7-verify`)
  : 285/286 PASS + 1 ignored + 1 pre-existing test failure
  (`layout::tests::f3_t4_broken_same_version_install_is_repaired_not_hidden`,
   panic at `crates/cognicode-cli/src/bin/../cmd/layout.rs:2024`,
   verified pre-existing on `ba8897b5` and `d98f4a08` with the
   cycle's diff stashed)
- `cargo test -p cognicode-core --lib`                : 2074 PASS, 27 ignored
  (TMPDIR workaround required)

## Release status

Cycle stalled in `RELEASE_PENDING` at `phase=release` / `sequence=4`.
The release-manifest in the cycle-artifacts dir records:

- Workspace version stays at `0.97.2` (no PATCH bump committed).
- `sddk release plan --tag v0.97.3` would be the next step, but the
  decision to cut a tag is reserved to the operator per the H4.7
  "release engineering is irreversible" rule.
- Operator-visible release artifact (`v0.97.2`) already includes the
  H5 phase-1 + H5 phase-2 work as PATCH content.

## Honest accounting

- The cycle commits work in **exactly one** commit (`be61724b`).
  No force-push, no rebase, no squash, no AI-attribution trailer.
- Three trivial-lint candidates were deferred with reason, not silently
  dropped.
- Pre-existing test failure was *not* attributed to this cycle and
  *not* fixed under "no scope-creep".
- `permissions.yaml` (untracked, in repo root) is left in place for
  possible future `sddk release apply` invocations; not committed
  because adding a new file to the project root was not authorized.
