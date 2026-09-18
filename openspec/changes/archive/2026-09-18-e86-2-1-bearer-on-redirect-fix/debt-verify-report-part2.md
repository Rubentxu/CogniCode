# DEBT-VERIFY REPORT — E86.2.1 — PART 2 (Risks 5–8 + Carry-forward)

> Continuation of `debt-verify-report.md` (executive summary + risks 1–4).
> This file covers risks 5–8, the carry-forward recommendations,
> the environmental note, and the commit/file index.
> Same date, same HEAD (`a506fc2d`), same authority (proposal +
> design + spec + apply-receipt at
> `openspec/changes/e86-2-1-bearer-on-redirect-fix/`).

---

## Risk 5 — `response.bytes()` loads the whole asset into memory

- **Status**: NEEDS-FOLLOW-UP
- **Severity**: MED

**Evidence.**

`installer_transaction.rs:285`:

```rust
std::io::copy(&mut response.bytes().unwrap().as_ref(), &mut file)
    .map_err(|e| InstallerError::Io(dest.clone(), e))?;
```

`response.bytes()` materializes the full body into a `Bytes` value
(via reqwest's `Response::bytes()`). For a typical mcp-server
tarball at ~9 MB this is harmless; for a future bundle that bundles
multiple platforms or a much larger asset (the `cogh update` story
calls for "self-update of the installer itself"), this allocates
proportional memory and holds it for the lifetime of the response
object before `std::io::copy` actually flushes to disk.

Compare to `registry.rs:86` which uses the streaming path:

```rust
resp.copy_to(&mut out)
    .with_context(|| format!("write {}", dest.display()))?;
```

This streams the body directly from the network socket to the file
without buffering.

The design §8 explicitly defers this: *"SSE / streaming bodies. The
current installer reads the full body into memory
(`response.bytes().unwrap()`). The manual loop preserves this."*

`apply-receipt.md §8` also flags it as part of E86.2.2's anticipated
scope (along with the `resolve_download_url` /
`COGNICODE_RELEASE_BASE_URL` interaction).

**Debt.** Real but bounded: the largest single asset shipped today
is ~9 MB and the installer's resident set can absorb that without
issue. The risk grows with bundle size, not with user count.

**Recommendation.**

- Track as MED debt for E86.2.2.
- Fix is local: replace `response.bytes().unwrap()` with
  `response.bytes_stream()` (or just use the `resp.copy_to`
  streaming pattern from `registry.rs:86`). Two-line change in
  `installer_transaction.rs:283-286`. Add a regression test that
  asserts the response body is consumed without buffering (e.g. by
  checking the streaming-read interface exists and is used).
- Watch for: with `download_with_bearer()` returning
  `reqwest::blocking::Response`, the caller must consume the body
  before falling through to the next iteration of the loop — the
  current code already does so (one terminal response, one
  `bytes()` call, then `Ok(())`). When moving to streaming, ensure
  the streaming consumer is `Drop`-ed before re-entering the loop.
  This is the main subtlety for the next cycle's author.

---

## Risk 6 — `is_in_trust_set` wildcard logic is duplicated

- **Status**: PASS
- **Severity**: INFO

**Evidence.**

`grep -rn "is_in_trust_set" crates/` returns two matches, both inside
`installer_transaction.rs` (the definition at line 65 and the call
site at line 156). `lifecycle_resolver.rs::gh_trust_set()` (line 339)
returns the **raw static slice** — no wildcard interpretation. The
suffix-match logic (`trusted.starts_with("*.")` → `host.ends_with(...)`)
lives in **exactly one place**: `installer_transaction.rs:65-74`.

There is no `_resolver_internal` / `_shared` copy of the same
predicate, so the consistency claim holds: if `gh_trust_set()` is
extended tomorrow with another `*.foo` entry, the helper picks it up
automatically.

**Verdict.** Single source of truth. Pass.

**Recommendation.**

- None. Worth noting in the next-helper consolidation pass: if the
  helper ever needs to be reused (e.g. E86.2.2 brings
  `lifecycle_resolver::http_get` under the same loop), promote
  `is_in_trust_set` to `pub(crate) fn` in `installer_transaction`
  and have it take `&'static [&'static str]` explicitly to keep
  the seam testable.

---

## Risk 7 — `cargo build` warnings on `installer_transaction.rs`

- **Status**: NEEDS-FOLLOW-UP
- **Severity**: INFO

**Evidence.**

`grep` for `#[allow(...)]`, `#[warn(...)]`, `#[deny(...)]` attributes
on `installer_transaction.rs` returns zero matches. The file is
governed by the workspace-wide lint configuration (no file-local
relaxation).

`apply-receipt.md §7` reports the worker's own clippy run:

> `cargo clippy -p cognicode-cli --bin cogh --tests 2>&1 | grep -A4 "installer_transaction.rs:"`
> → 4 pre-existing warnings on lines 13/209/225/622 (unrelated), 0 new

The four pre-existing lines are flagged as unrelated to E86.2.1; the
apply worker confirmed none of them are introduced by the helper or
the three new tests.

**Cannot independently re-verify** because bash execution is broken
on this worker host (see §"Environmental note" below). The
apply-receipt claim is the best available evidence.

**Recommendation.**

- Trust the apply receipt for E86.2.1 close.
- Re-run `cargo clippy -p cognicode-cli --bin cogh --tests 2>&1 |
  grep -A4 installer_transaction.rs:` post-close on the orchestrator
  host and confirm "0 new warnings" claim survives this report's own
  addition.
- If a new warning appears post-debt-verify, file it as MED debt
  (this report's edits to a sibling file shouldn't introduce any,
  but the clippy database can surprise).

---

## Risk 8 — Cross-check: proposal + design vs. code (deviations)

- **Status**: ISSUE
- **Severity**: LOW

**Evidence.**

The apply worker documented one substantive deviation from the
proposal/design in `apply-receipt.md §5`:

> **Design said**: client's default redirect policy is the issue;
> helper controls hops. ✓
> **Code does**: production client now `.redirect(Policy::none())` so
> the manual loop owns every hop end-to-end. The .md noted
> "auto-redirect is left at default for the *first* hop" — that was
> wrong; the apply worker discovered empirically that the helper
> cannot control the first hop unless the policy is `none()` from the
> start. The design was updated to clarify: see §"Implemented
> behaviour" in `design.md`.

Re-reading `design.md` line by line (lines 91-109):

```text
1. Builds the first request with the bearer.
2. Calls `client.get(url).send()` (the client still has default
   redirect policy — leaving it gives us a single-hop fall-back if a
   future reqwest upstream relaxes the strip, with no manual-loop
   changes needed). For reqwest 0.12.28 the bearer will be stripped on
   the cross-origin hop, so the helper detects "Location" header on
   status 301/302/303/307/308 and re-issues `client.get(new_url)
   .bearer_auth(token).send()` until status is final or max hops hit.
```

This text **still describes the original design** (auto-redirect for
the first hop). The apply receipt asserts the design was updated,
but the on-disk design.md does **not** reflect the corrected
narrative. There is **no `§Implemented behaviour` section** in the
current design.md (verified by reading the full file). The
deviation is faithfully recorded in the apply-receipt but the design
itself remains stale.

Other deviations cross-checked:

| Item | Proposal / Design | Code | Status |
|---|---|---|---|
| `MAX_DOWNLOAD_REDIRECTS: u8 = 5` | proposal §"Approach", design §3.1 | installer_transaction.rs:26 | ✓ matches |
| `gh_trust_set()` reused | proposal §"Approach", design §3.2 | installer_transaction.rs:12 (import) | ✓ matches |
| `bearer_from_env()` reused | proposal §"Approach", design §3.1 | installer_transaction.rs:270 | ✓ matches |
| `Network("download", "DisallowedRedirectHost <host>")` | design §4 | installer_transaction.rs:159 | ✓ matches |
| Private vs public helper | proposal Q1 | apply-receipt.md §4 says "PRIVATE" | ✓ matches |
| First-hop auto-redirect default | design §3.1 line 94-97 | installer_transaction.rs:267 (`Policy::none()`) | **de facto** deviation; apply-receipt notes it; **design.md not updated** |
| `response.bytes()` body-into-memory | design §8 (out of scope) | installer_transaction.rs:285 | ✓ matches (deferred) |

**Debt.** One stale sentence in `design.md §3.1` describes behaviour
the code does not implement. A future maintainer who reads the
design first and the code second will be confused. The apply-receipt
papers over it for the cycle, but the debt-verify expectation is
that the design is the **canonical** reference, not the receipt.

**Recommendation.**

- Update `design.md §3.1` to read:

  > Calls `client.get(url).send()`. The production client is built
  > with `.redirect(reqwest::redirect::Policy::none())` so the helper
  > owns every hop. **No auto-redirect**, no fall-back: the
  > `download_with_bearer()` manual loop is the single source of
  > truth for which `Location` headers get followed.

- This is a 1-paragraph doc edit; safe for the next maintainer.

- Alternative (less disruptive): delete the misleading sentence
  from `design.md §3.1` and add a forward pointer
  *"see `installer_transaction.rs:265-269` for the production client
  configuration"*.

---

## Final risk table (consolidated)

| ID | Risk | Status | Severity | Action |
|---|---|---|---|---|
| R1 | Loopback ghost leaks via `gh_trust_set` in production binary | ISSUE | MED | E86.2.2 — gate `127.0.0.1` / `localhost` behind `#[cfg(test)]` |
| R2 | `MAX_DOWNLOAD_REDIRECTS=5` is hard-coded, no env override | NEEDS-FOLLOW-UP | LOW | Track; env-var follow-up only when first user hits the cap |
| R3 | `Policy::none()` on the production install client | PASS | INFO | None — needed for the manual loop to own hops |
| R4 | `Network` envelope over-loads redirect-class errors | ISSUE | LOW | E86.2.x — promote `DisallowedRedirectHost` / `TooManyRedirects` to first-class variants |
| R5 | `response.bytes()` materializes the whole body in RAM | NEEDS-FOLLOW-UP | MED | E86.2.2 — switch to `resp.copy_to` / streaming pattern |
| R6 | `is_in_trust_set` wildcard logic duplication | PASS | INFO | None — single source of truth confirmed |
| R7 | New clippy warnings on installer_transaction.rs | NEEDS-FOLLOW-UP | INFO | Re-verify on orchestrator host post-close |
| R8 | Design.md §3.1 stale on the first-hop auto-redirect | ISSUE | LOW | Edit design.md §3.1 (one paragraph) |

**Counts:** ISSUE = 3, NEEDS-FOLLOW-UP = 3, PASS = 2. By severity:
MED = 2 (R1, R5), LOW = 3 (R2, R4, R8), INFO = 3 (R3, R6, R7).
CRITICAL = 0. HIGH = 0.

**Blockers for cycle close:** none. E86.2.1 can close to archive
once the orchestrator registers R1 / R5 as E86.2.2 carry-forward and
schedules R4 / R8 as one-line docs / refactor items.

---

## Carry-forward recommendations (E86.2.2 scope)

1. **R1** — gate the loopback entries in `gh_trust_set` behind
   `#[cfg(test)]`. Single-file change in `lifecycle_resolver.rs`;
   update `installer_transaction.rs::tests` to use the test-gated
   variant. Add a unit test that asserts `gh_trust_set()` in a
   release build contains **only** the 6 GitHub-origin hosts (use
   a `cfg`-aware build script or a doc test).
2. **R5** — swap `response.bytes()` for streaming write in
   `installer_transaction.rs:283-286`. Mirror the
   `registry.rs:84-87` pattern. Add a test that exercises a
   multi-MB body and asserts resident-set growth is bounded.
3. **R2 / R4 / R7** — opportunistic. Close only when triggered.
4. **R8** — apply-cycle cleanup of design.md. Single-paragraph edit.

---

## Environmental note (honesty)

Bash execution on this debt-verify worker (`session_maple`) returned
`No such file or directory` on every invocation, including
`/bin/true` and `echo`. The initial exploration therefore went via
the file-reading tools only. All evidence above was extracted from
on-disk files at HEAD `a506fc2d`; no shell command was run in this
session. The `cargo test` and `cargo clippy` claims in
`apply-receipt.md` are accepted as the worker's own observations and
are flagged as **not independently re-verified** in the relevant
risks above (R7 in particular).

This is a debt-verify artefact, not a debt on the production code,
but the orchestrator should be aware: if the next cycle requires
fresh clippy / test runs as part of its gate, that work must happen
on a worker with a working shell.

---

## Commits

The commit `chore(debt-verify): E86.2.1 post-verify debt report`
will be authored by `Ruben <rubentxu@cognicode.dev>` on the local
`main` branch (no push). SHA reported in the swarm DM and the
follow-up completion message.

If shell execution is restored by the time of commit, the commit
hash will be added here and to the part-1 file's commit footer.

---

## Files added by this report

| Path | Purpose |
|---|---|
| `openspec/changes/e86-2-1-bearer-on-redirect-fix/debt-verify-report.md` | Part 1 — executive summary + evidence legend + risks 1–4 |
| `openspec/changes/e86-2-1-bearer-on-redirect-fix/debt-verify-report-part2.md` | Part 2 — risks 5–8 + carry-forward + environmental note |

No code changed. No existing file modified.
