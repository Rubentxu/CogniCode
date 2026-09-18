# DEBT-VERIFY REPORT — E86.2.1 — Bearer Token Preservation Through Redirects

> Owner: E86.2.1 (post-verify debt gate)
> Cycle: debt-verify (E86.2.1)
> Date: 2026-09-18
> HEAD: `a506fc2d` (parent: `cfc079d2`)
> Branch: `main` (local, no push)
> Program: cognicode-distribution (umbrella e84 + e85 + e86)
> Authority (this report): proposal + design + spec + apply-receipt at
> `openspec/changes/e86-2-1-bearer-on-redirect-fix/`
> Format reference: `openspec/changes/archive/2026-09-17-e86-1-cogh-pinned-bug-remediation/verification-report.md`
>
> This file covers executive summary + risks 1–4.
> See `debt-verify-report-part2.md` for risks 5–8, carry-forward,
> environmental note, and commit info.

---

## Executive summary

| Metric | Value |
|---|---|
| Total risks evaluated | 8 |
| PASS (no debt) | 2 (risks 3, 6) |
| ISSUE (debt introduced or exposed) | 3 (risks 1, 4, 8) |
| NEEDS-FOLLOW-UP (cycle candidate) | 3 (risks 2, 5, 7) |
| CRITICAL issues | 0 |
| HIGH issues | 0 |
| MED issues | 2 (R1, R5) |
| LOW issues | 3 (R2, R4, R8) |
| INFO items | 3 (R3, R6, R7) |
| **Blockers for cycle close** | **none** |

E86.2.1 is **sound and shippable** for the bounded intent
(bearer preserved across `github.com → *.githubusercontent.com`).
The debt surface is dominated by three MED/LOW items that belong
to **E86.2.2** or follow-up cycles, plus one MED cross-cutting item
worth elevating if no follow-up cycle claims it soon.

**Cycle verdict: PASS-WITH-FOLLOW-UPS.** E86.2.1 can close to archive
once the three `NEEDS-FOLLOW-UP` items are registered as debt for the
next cycle. None of the ISSUE / NEEDS-FOLLOW-UP items invalidate the
fix.

---

## Evidence legend

All evidence was extracted by reading the on-disk sources at HEAD
`a506fc2d` (as recorded by `apply-receipt.md`). `cargo` and
`cargo clippy` were NOT re-invoked in this debt-verify pass: bash
execution on this worker host returned `ENOENT` on every invocation
(see §"Environmental note" in part 2). The "Runs green" claims below
are sourced from `apply-receipt.md §5–§7`, which the apply worker ran
on the actual repo. Recommendations that depend on running
`cargo clippy` should be re-verified by the orchestrator before being
closed.

---

## Risk 1 — Loopback ghost leaks in production `gh_trust_set`

- **Status**: ISSUE
- **Severity**: MED

**Evidence.**

`gh_trust_set()` in
`crates/cognicode-cli/src/cmd/lifecycle_resolver.rs:339-354` carries
two entries for the test fixture loopback servers:

```rust
// Loopback aliases for the unit tests' mini TCP servers.
// ... suffixes are exact (not wildcards) ...
"127.0.0.1",
"localhost",
```

`is_in_trust_set()` in `installer_transaction.rs:65-74` matches
these **exactly** (the wildcard branch only fires for entries
beginning with `*.`):

```rust
trust.iter().any(|&trusted| {
    if trusted.starts_with("*.") {
        host.ends_with(&trusted[1..])
    } else {
        host == trusted
    }
})
```

`grep -rn "gh_trust_set" crates/` shows six matches across exactly
two files (`installer_transaction.rs`, `lifecycle_resolver.rs`);
no other production call site exists. The "use only in
installer_transaction" premise from the prompt is **structurally
satisfied today**.

**Debt.** The loopback entries are unconditionally present in the
**production binary** (no `#[cfg(test)]` gate on the static list).
The trust set is consulted on every redirect hop. In a future bug
class where a hostile HTTP endpoint redirects the installer to
`http://127.0.0.1:<ephemeral>` (a CSRF/SRF vector where the attacker
controls the `Location` header and the user's machine runs a dev
server), the bearer would be silently forwarded to the loopback
target and the redirect would NOT be classified as
`DisallowedRedirectHost`.

The exact-match logic and the port-stripping in
`installer_transaction.rs:145-154` make this very narrow — an
attacker would have to land on `127.0.0.1` with no port suffix that
the stripper doesn't normalize — but the attack surface is non-zero
in production.

**Recommendation.**

- Move the two loopback entries into a `#[cfg(test)]` slice or a
  `cfg(test)!`-gated `pub(crate) fn gh_trust_set_with_test_hosts()`
  so the **production binary** ships only with the 6 GitHub-origin
  hosts. 5-line change in `lifecycle_resolver.rs` and a single test
  signature update in `installer_transaction.rs::tests`.
- Alternative: switch `is_in_trust_set()` to `cfg(test)!`-gated
  loopback matching — but that breaks the helper signature and
  complicates the unit tests. Less attractive.
- **Cycle ownership**: E86.2.2 (security hardening of the redirect
  helper) is the natural home. Until then, register as MED debt.

---

## Risk 2 — `MAX_DOWNLOAD_REDIRECTS=5` is hard-coded (no env override)

- **Status**: NEEDS-FOLLOW-UP
- **Severity**: LOW

**Evidence.**

`grep -rn "max_redirect\|MAX_REDIRECT\|COGH_MAX_REDIRECTS" crates/`
returns **zero matches** outside `MAX_DOWNLOAD_REDIRECTS` itself.
The constant lives at `installer_transaction.rs:26`:

```rust
/// GitHub's release-asset redirect chain is at most 2 hops
/// (api.github.com → objects.githubusercontent.com). 5 provides
/// defensive slack while still protecting against redirect loops.
const MAX_DOWNLOAD_REDIRECTS: u8 = 5;
```

It is read once at `installer_transaction.rs:97` and again in the
user-facing error string at line 100:

```rust
if hop > MAX_DOWNLOAD_REDIRECTS {
    return Err(InstallerError::Network(
        url.to_string(),
        format!("TooManyRedirects (>{})", MAX_DOWNLOAD_REDIRECTS),
    ));
}
```

The proposal §"Risks" (line 199) explicitly acknowledged this:
*"env var `COGH_MAX_REDIRECTS` deferred to a follow-up if any user
reports legitimate chains longer than 5."* The design §7 risk table
(line 199) repeats the deferral. Neither `tasks.md` nor
`apply-receipt.md` picked the env-var follow-up.

**Debt.** Five hops is **defensible** today: the apply worker's
real-PC UAT (`apply-receipt.md §3`) confirmed the actual chain is
`github.com → release-assets.githubusercontent.com` (1 hop). The
defensive cap is well-chosen for the current GH posture.

However: a future move to a CDN that fronts releases with a custom
3+ hop chain (Cloudflare + Akamai + region pinning is a common
pattern) would lock users out without recourse.

**Recommendation.**

- Register as LOW debt; close in the cycle that first sees a
  user-reported `TooManyRedirects` error. The constant stays put
  until then — adding the env-var plumbing prematurely would expand
  the public surface for a problem nobody has hit.
- When the cycle lands, the env var should be **optional**: unset
  → keep `MAX_DOWNLOAD_REDIRECTS=5` as the default.

---

## Risk 3 — `Policy::none()` on the production client (other call sites)

- **Status**: PASS
- **Severity**: INFO

**Evidence.**

`grep -rn "reqwest::blocking::Client::builder" crates/` shows four
production sites (one is test-only):

| File | Function | Policy | Redirect risk? |
|---|---|---|---|
| `installer_transaction.rs:265` | `advance_stage::Downloading` | `Policy::none()` | **owned by `download_with_bearer()`** |
| `lifecycle_resolver.rs:264` | `http_get` | default | Uses `api.github.com` (no cross-host redirect today). Documented in proposal §"Non-goals" as deferred to E86.2.2 |
| `registry.rs:64` | `download_to` | default | URL is direct (plugin manifest `version.url`), no cross-host redirect |
| `layout.rs:641` | `download_to_string` | default | Same: manifest-driven URL, no redirect chain |

The helper at `lifecycle_resolver.rs:_build_gh_client_placeholder`
(line 799) is `#[cfg(test)]`-gated and used only by
`sc_fix_01_default_policy_strips_authorization_on_cross_host_redirect`
and `sc_fix_02_untrusted_redirect_target_is_not_followed` — those are
the defect-documentation tests.

**Verdict.** The `Policy::none()` change at
`installer_transaction.rs:267` is **necessary** for
`download_with_bearer()` to own every hop end-to-end (verified
empirically: with default policy, reqwest auto-follows the first hop
and strips the bearer — same defect, different layer; see
`apply-receipt.md §6`). The other call sites do **not** exhibit the
defect because their URLs are direct or their endpoints
(`api.github.com`) do not currently redirect cross-host. The proposal
correctly scoped the change to a single file (`installer_transaction.rs`).
The carry-forward item for E86.2.2 is already noted in
`apply-receipt.md §8` and in `design.md §8`.

**Recommendation.**

- No action for E86.2.1 close.
- E86.2.2 should mirror the manual-loop pattern into
  `lifecycle_resolver::http_get` ONLY if a real-world redirect chain
  ever appears on `api.github.com` (currently zero evidence).

---

## Risk 4 — `DisallowedRedirectHost` error envelope is over-loaded

- **Status**: ISSUE
- **Severity**: LOW

**Evidence.**

The error variant used is `InstallerError::Network(String, String)`
(error.rs:25-26):

```rust
#[error("network error fetching {0}: {1}")]
Network(String, String),
```

The redirect-specific failure modes are encoded as **reason strings**:

| Helper outcome | Encoded as |
|---|---|
| Hop to host outside trust set | `Network("download", "DisallowedRedirectHost <host>")` |
| Hop count exceeds cap | `Network("download", "TooManyRedirects (>5)")` |
| 30x without `Location` | `Network(<current_url>, "redirect status with no Location header")` |

A user-facing caller (`main.rs` / `cli.rs`) can only distinguish
these by **substring matching** on the reason string, which couples
the CLI surface to the helper's internal error vocabulary.

`error.rs:18-75` shows 12 `InstallerError` variants with structured
fields (e.g. `Sha256Mismatch`, `EmptyInstall(profile, version)`,
`ResolveFailed(String)`). Adding two new variants would be in line
with the existing style:

```rust
#[error("disallowed redirect host: {0}")]
DisallowedRedirectHost(String),

#[error("too many redirects (> {0})")]
TooManyRedirects(u8),
```

`design.md §3.3` and `design.md §10` both explicitly call this out
as a deliberate choice (*"No new variant needed"*). The author
weighed it against the cost of a new variant per redirect-class
error; the design is internally consistent. But the cost is paid
at every downstream consumer that wants to differentiate these
failures.

**Debt.** A future CLI diagnostic command (e.g.
`cogh doctor --explain`) will need to match on `DisallowedRedirectHost`
vs `TooManyRedirects` vs generic `Network` to give actionable advice.
At that point the string-substring approach breaks.

**Recommendation.**

- Out of scope for E86.2.1. Register as LOW debt.
- When the first cycle needs to differentiate these errors in user
  output, promote both to first-class variants. The migration is
  mechanical: a `match` arm in `installer_transaction.rs` plus the
  variant definitions in `error.rs`. Two new tests
  (`assert_disallowed_redirect_host_variant`,
  `assert_too_many_redirects_variant`) lock the new shape.

---

## Final risk table (preliminary; full table in part 2)

| ID | Risk | Status | Severity | Action |
|---|---|---|---|---|
| R1 | Loopback ghost leaks via `gh_trust_set` in production binary | ISSUE | MED | E86.2.2 — gate `127.0.0.1` / `localhost` behind `#[cfg(test)]` |
| R2 | `MAX_DOWNLOAD_REDIRECTS=5` is hard-coded, no env override | NEEDS-FOLLOW-UP | LOW | Track; env-var follow-up only when first user hits the cap |
| R3 | `Policy::none()` on the production install client | PASS | INFO | None — needed for the manual loop to own hops |
| R4 | `Network` envelope over-loads redirect-class errors | ISSUE | LOW | E86.2.x — promote `DisallowedRedirectHost` / `TooManyRedirects` to first-class variants |

Continue reading at `debt-verify-report-part2.md` (risks 5–8,
carry-forward, environmental note).
