# ADR-053 — CP1 License Publication and Record

- Status: **ACCEPTED**
- Date: 2026-09-27
- WorkItem: A-007 (CP1.1)
- Cycle: `p-c1fac1fea05615c6/cp1-oss-foundation`
- Origin: **the license decision itself was taken in M0.9, not here.** This ADR records and publishes it; it does not re-decide it.
- Supersedes: none
- Superseded by: none

## Context

### What already existed

`deny.toml` encodes the project's **inbound** license policy: a conservative allow-list of permissive licenses (MIT, Apache-2.0, BSD-2/3, 0BSD, ISC, Zlib, BSL-1.0, CC0-1.0, MIT-0, MPL-2.0, Unicode-3.0, Unlicense, CDLA-Permissive-2.0, Apache-2.0 WITH LLVM-exception, LGPL-2.1-or-later), explicitly refusing GPL-2.0/3.0, AGPL-3.0, SSPL, Commons Clause, and the JSON License. Its comment block still reads:

> Recommended operator action: pick a license (MIT, Apache-2.0, MIT OR Apache-2.0, etc.) and add `license = "..."` to cognicode-runtime/Cargo.toml and cognicode-sandbox/Cargo.toml.

That recommendation was **already actioned**. Maintenance unit **M0.9** (commit `f0708d4b`, CLOSED 2026-09-26) added `license = "MIT OR Apache-2.0"` to the 10 workspace crates that lacked it, bringing all 12 members into line. `cargo deny check licenses` reports exit 0 with "licenses ok".

M0.9 also disclosed that the earlier `deny.toml` claim of "2 crates flagged" was a **miscount**: the real output listed 10 unlicensed crates. That correction is recorded in `docs/roadmap/MAINTENANCE.md` and `docs/roadmap/JOURNAL.md` §4114. This ADR does not rewrite that history; it cites it.

M0.9 deliberately did **not** touch `[workspace.package]`, recording: *"`[workspace.package]` no se tocó — el operador puede formalizar license-of-record ahí si lo desea."*

### What was actually missing

As of base `f24609f0927fad51b3d2cc33376e5ee90d154080` the repository had **no `LICENSE` file, no `LICENSE-MIT`, and no `LICENSE-APACHE`**, and `package.json` declared no `license` field.

This is a real legal defect, and it is easy to miss. Every crate declares `MIT OR Apache-2.0`, but that expression is a **choice between two named licenses**, and the repository shipped **neither text**. A third party reading this repository could see the identifier and had no way to learn the terms they would be accepting. A license expression without its texts is a dangling reference, not a grant.

### A premise that was wrong and got corrected

The CP1 exploration initially reported that `cognicode-runtime` and `cognicode-sandbox` "lack a license field and are flagged as unlicensed". That was **false** — inherited from the stale `deny.toml` comment rather than from an observation. All 12 crates declare the expression. The correction was made at a Build-phase checkpoint on 2026-09-27 and propagated to the exploration report, specification and design **before any artifact was written**. Had it not been caught, CP1 would have shipped an ADR that re-decided a closed unit's outcome under a false premise.

## Decision

**CogniCode is licensed under `MIT OR Apache-2.0` (SPDX), as decided in M0.9. CP1 publishes it and records it as the license of record.**

Specifically:

1. **`LICENSE-MIT` and `LICENSE-APACHE`** are committed at the repository root, both texts in full. This is the actual fix: the dual expression becomes a usable grant.
2. **`LICENSE`** at the root states the dual grant in prose, names both text files, carries the SPDX identifier, and cites this ADR. It also states the copyright line.
3. **`Cargo.toml [workspace.package]`** declares `license = "MIT OR Apache-2.0"`, formalising the deferral M0.9 left open. The 12 members keep the explicit field they already carry; they are deliberately **not** converted to `license.workspace = true`, which would be a 12-file rewrite of a closed unit's shape for no behavioural gain.
4. **`package.json`** declares the same expression, matching `apps/explorer-ui/src/wasm/cognicode_diagram_wasm/package.json`, which already did.
5. A test asserts the **exact** expression resolves for every workspace member through `cargo metadata`, so the grant cannot be silently narrowed or widened later.

## Rationale for keeping the dual expression rather than narrowing it

1. **M0.9 closed it deliberately**, applied consistently to all 12 crates, with `cargo deny check licenses` green. Narrowing it in a documentation cycle would rewrite a closed unit's outcome for no adopter benefit.
2. **It is maximally permissive for consumers.** Each adopter picks whichever of the two fits their policy. This is the Rust ecosystem's default dual expression, and it removes a common procurement blocker.
3. **Both operands are already allowed inbound.** `MIT` and `Apache-2.0` are both in `deny.toml`'s allow-list, so the outbound grant is inside the project's own policy by construction. A test asserts this per operand.
4. **Apache-2.0 contributes the patent grant** for adopters who choose it, which matters for a tool that generates and applies source refactors. MIT contributes maximum simplicity for those who prefer it. Shipping both is strictly better for consumers than choosing one.
5. **Publishing both texts removes the whole defect.** Narrowing the expression would not improve any adopter outcome; publishing the texts completely resolves the legal gap.

## Rejected alternatives

| Alternative | Why rejected |
|---|---|
| **Apache-2.0 only** | Would require rewriting 12 closed `Cargo.toml` files. The only advantage is a single identifier, and the tests already enforce exact-expression consistency, so the stated benefit is already obtained without a legal change. |
| **MIT only** | Permissive and already allowed, but drops the patent grant that Apache-2.0 provides to adopters of a code-rewriting tool. Narrowing a superset of permissions with no consumer demand is not a product improvement. |
| **MPL-2.0** | Allowed inbound, and file-level weak copyleft would be defensible. Rejected: it adds per-file obligations with no operational benefit to this crate-per-binary layout, and it is less universally accepted in enterprise policy allow-lists. |
| **AGPL-3.0** | Explicitly refused by the project's own `deny.toml`. Adopting it outbound would contradict the stated stance. |
| **No license (keep as-is)** | Not an option. Shipping no license means no one may lawfully copy, modify or distribute the code. |

## Consequences

- The project is legally consumable. Both texts ship, the expression is stated once, and every published crate and npm package declares it.
- `cargo deny check licenses` is expected to remain exit 0. This is a consequence to be **observed**, not assumed; the suite resolves the real value through `cargo metadata`.
- The license is now a published contract guarded by tests. Changing it requires superseding this ADR, not a silent edit.
- **Copyright holder:** `Copyright 2025-2026 The CogniCode Authors`, chosen to match the existing `authors = ["CogniCode Team"]` declaration in `Cargo.toml` rather than invent a holder. A legal reviewer with a different preference supersedes this ADR; the test asserts the line is present and non-placeholder, not that this exact string is the only valid one.

## CP1.7 — Discussions/community categories: partially applied, categories `operator-gated`

`13-ROADMAP-COMMUNITY-PRODUCTIZATION.md` §CP1 item 7 asks for "Discussions/community categories". GitHub Discussions are **repository settings**, not files in the repository. They cannot be committed and cannot be asserted by a test, so a code cycle must not claim them complete.

**Status: Discussions enabled, category creation `operator-gated`.**

### What was applied, and how it was verified

Enabling Discussions turned out to be automatable after all, so it was not left as operator work. `UpdateRepositoryInput.hasDiscussionsEnabled` exists in the GraphQL schema, and the mutation succeeded:

```
mutation($id:ID!,$d:Boolean!){
  updateRepository(input:{repositoryId:$id, hasDiscussionsEnabled:$d}){
    repository{ hasDiscussionsEnabled }
  }
}
```

The result was then read back through an independent endpoint rather than trusting the mutation's own return value: `GET /repos/{owner}/{repo}` reports `has_discussions: true`. Before the mutation it reported `false`.

### What remains operator-gated, and why

Creating the **categories** has no API surface. The GraphQL `Mutation` type exposes `createDiscussion`, `updateDiscussion`, `deleteDiscussion`, `addDiscussionComment` and the comment/poll/answer mutations, and **no category mutation at all** — `createDiscussionCategory` does not exist and returns `undefinedField`. The REST route `GET /repos/{owner}/{repo}/discussions/categories` returns 404. Categories are created only from the repository settings UI.

The intended initial set, so the operator action is unambiguous and does not require a design decision at that point:

| Category | Purpose | Emoji |
|---|---|---|
| **Announcements** | Release notes, roadmap moves, changes affecting how you run CogniCode. | 📣 |
| **Q&A** | Usage questions. Docs and existing discussions first. | 🙋 |
| **Show and tell** | What people built with CogniCode and workflows worth sharing. | 🛠️ |
| **Ideas** | Feature proposals and rough directions; worked use cases move fastest. | 💡 |

Until those four exist, CP1.7 is **not** complete and the CP1 OSS gate stays open. `test_license_adr_records_decision_and_operator_gate` keeps requiring this section to say `operator-gated`, which is still true of the remaining work.

## Verification

- `scripts/product/test_oss_foundation.py::test_license_texts_are_published` — canonical MIT and Apache-2.0 markers, minimum sizes, root `LICENSE` names both texts and the expression, copyright line present.
- `scripts/product/test_oss_foundation.py::test_workspace_package_states_the_license_of_record` — the deferred M0.9 line is present and exact.
- `scripts/product/test_oss_foundation.py::test_every_workspace_member_resolves_to_the_license_of_record` — resolves all members through `cargo metadata`; fails on `null` or on any expression that is not the license of record.
- `scripts/product/test_oss_foundation.py::test_package_json_declares_the_same_license` — both `package.json` files agree.
- `scripts/product/test_oss_foundation.py::test_license_identifiers_are_inside_deny_allow_list` — every operand is inside the inbound policy.
- `scripts/product/test_oss_foundation.py::test_license_adr_records_decision_and_operator_gate` — this ADR records the expression, the M0.9 origin, the commit, the rejected alternatives, and the CP1.7 operator gate.
