# e86.2.1 — COGNICODE_GITHUB_TOKEN not propagated across GitHub asset redirects

> Cycle: e86-2-1-bearer-on-redirect-fix
> Program: cognicode-distribution (umbrella: e84 contract + e85 release + e86 lifecycle)
> Parent: e86.2 (real-PC UAT — closed PARTIAL with HTTP 403 finding, see
>   `openspec/changes/e86-2-cogh-real-user-uat/uat-receipt.md`)
> Phase: propose | Date: 2026-09-18
> Delivery: bounded cycle, single slice, RED-first; design revised 2026-09-18
>   after empirical RED tests showed `Policy::custom(|a| a.follow())`
>   preserves the same stripping as the default policy — see §"Empirical
>   evidence revision" below. Design now mandates a **manual redirect
>   loop** in `installer_transaction.rs::Downloading`.
> Severity: BLOCKER for end-user installs against the v0.95.0 release until fixed

## Intent

`cogh install --version 0.95.0` against the real GitHub Releases endpoint
currently fails with HTTP 403 Forbidden on the tarball download. Cross-
verification with `curl` (anonymously and authenticated) proves the asset
exists and is reachable; the `reqwest::blocking::Client` built at
`crates/cognicode-cli/src/cmd/installer_transaction.rs:135` is the cause,
not the network.

Concretely the client:

1. **Has no `Authorization` header attached** for the original request to
   `https://github.com/.../releases/download/v0.95.0/<asset>.tar.gz`. The
   reader at `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs:270`
   (and `layout.rs:647`) feeds the Bearer into the **resolver** (for the
   GitHub API), not into the **installer client** for asset downloads.
2. **Follows the 302 redirect** to `objects.githubusercontent.com` with
   reqwest's default `redirect::Policy::default()`, which strips sensitive
   headers on cross-origin redirects (a deliberate reqwest behaviour).
   So even if (1) were fixed and the original request carried the Bearer,
   it would still be dropped at the redirect boundary.

The 403 from `objects.githubusercontent.com` is correct because the
anonymous or stripped-Bearer signed URL we arrived with is treated as
unauthorized by AWS S3 (the asset signing scope requires an authenticated
redirect reference).

This cycle fixes the download path in `installer_transaction.rs` (and
parallel reads in `registry.rs:64`, `layout.rs:641`, the resolver's own
HTTP at `lifecycle_resolver.rs:264`) so that:

- The `Authorization: Bearer <COGNICODE_GITHUB_TOKEN>` header travels on
  the original asset request.
- Cross-origin redirects to `*.githubusercontent.com`,
  `*.github.com`, `*.github.io` (the GH asset CDN space) **preserve**
  the Bearer, because they are the same trust boundary.
- Cross-origin redirects to anything else drop the Bearer (reqwest default).

The fix is small (one helper function + four call sites), but it MUST be
proven RED-first against a real `302 → signed-S3` chain, not against a
loopback fixture that never produced a 302.

## Approach

The fix is **a manual redirect loop inside
`crates/cognicode-cli/src/cmd/installer_transaction.rs::Downloading`**.
Auto-redirect is left at the default for the *first* hop, but a helper
`download_with_bearer()` detects `Location` on `30x` responses and
re-issues `client.get(new_url).bearer_auth(token)` on each subsequent
hop, terminating on:

- final `2xx` (write body to disk and return);
- final `4xx / 5xx` (existing `Network(url, "HTTP <status>")`);
- hop redirected to a host outside `lifecycle_resolver::gh_trust_set()`
  (returns `Network("download", "DisallowedRedirectHost <new_host>")`);
- chain longer than 5 hops (returns
  `Network("download", "TooManyRedirects (>5)")`).

Read first:
  - `crates/cognicode-cli/src/cmd/installer_transaction.rs:127` —
    the `InstallStage::Downloading` arm that currently emits the bug.
  - `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs::gh_trust_set`
    — already `pub fn`; reused for the trust-set check on each hop.
  - `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs::bearer_from_env`
    — already exported; reused to decide whether the bearer is attached.

Implementation outline (concrete enough to act on):

1. **`download_with_bearer()` helper** in `installer_transaction.rs`
   (private fn in the same module as `Downloading`):

   ```rust
   fn download_with_bearer(
       client: &reqwest::blocking::Client,
       url: &str,
       bearer: Option<&str>,
   ) -> Result<Response, InstallerError>;
   ```

   - Hop 0: `client.get(url).send()`. If `bearer` is Some, attach
     `.bearer_auth(token)` on the *first* request only (manual loop
     will re-attach on each subsequent hop).
   - On `30x` with `Location`:
     - parse the next URL;
     - check host against `gh_trust_set()` (use the existing
       `is_in_trust_set` helper already used by `lifecycle_resolver`);
       if not in set → `Network("download",
       "DisallowedRedirectHost <host>")`;
     - re-issue `client.get(next_url).bearer_auth(token).send()` if
       bearer Some, else `client.get(next_url).send()`;
     - increment hop counter; break at >5.
   - Otherwise (terminal status), return the response so the existing
     body-write code can consume it.

2. **`Downloading` arm refactor**: replace lines 139–142
   (`client.get(...).send()`) with a call to `download_with_bearer()`
   followed by the existing body-to-disk code. Bearer presence is
   decided by `bearer_from_env()` (already exported; returns
   `Option<&str>`). Lines 149–153 remain untouched.

3. **Tests (apply phase)** in
   `crates/cognicode-cli/src/cmd/installer_transaction.rs::tests`:
   - Test `e86_2_1_fix_01_bearer_propagates_across_redirect` —
     spawn A→B TCP listeners on `127.0.0.1`, set `gh_trust_set()` to
     accept `127.0.0.1` for the test, set `COGNICODE_GITHUB_TOKEN`,
     call `download_with_bearer`, assert B receives
     `Authorization: Bearer <token>` and 200 is returned.
   - Test `e86_2_1_fix_02_untrusted_redirect_host_returns_disallowed_error` —
     A→C where C is not in `gh_trust_set()` (e.g. `attacker.example.com`);
     assert error variant.
   - Test `e86_2_1_fix_03_too_many_hops_returns_error` — 6-hop chain,
     assert error variant.

   The two RED tests already in
   `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs::tests`
   (`sc_fix_01_…`, `sc_fix_02_…`) are retained as **defect-documentation
   tests** and given a `// DEFECT-DOC` comment block explaining what
   they prove.

4. **Re-run the e86.2 UAT** (`/tmp/cogh-uat-real-pc.sh`) against the
   rebuilt release binary with `COGNICODE_GITHUB_TOKEN="$(gh auth
   token)"` and confirm Phases B / C / D turn green. Update
   `openspec/changes/e86-2-cogh-real-user-uat/uat-receipt.md` with the
   GREEN result. Note the UAT-receipt already lists Phase A as PARTIAL
   (HTTP 403); Phase B was blocked behind this E86.2.1 close.

5. **helpers**: `bind_one_shot_302`, `bind_one_shot_ok` and `header_value`
   are already in `lifecycle_resolver.rs::tests` (added during the RED
   phase). The apply cycle either promotes them to a shared
   `cmd::test_support` module (preferred) or duplicates them (acceptable).
   Decision logged during apply.

## Out-of-band correctness note (history preserved)

The original proposal (2026-09-18, first draft) recommended
`reqwest::redirect::Policy::custom(|a| a.follow())` as the fix.
A third RED-first test `sc_fix_01_green_policy_with_extra_hosts_propagates_bearer`
was authored to assert that design. **That test failed on 2026-09-18
against the actual library** — reqwest 0.12.28 strips `Authorization`
on cross-origin redirects even when the policy closure returns
`Follow`. The discovery is documented in
`crates/cognicode-cli/src/cmd/lifecycle_resolver.rs::tests` as a comment
above `_build_gh_client_placeholder` ("the GREEN counterpart" was
removed during this revision; the placeholder is retained only as a
reference of the policy shape). The fix path is the manual loop
described above, not the policy.

## Acceptance contract

| REQ | Observable |
|---|---|
| REQ-FIX-01 | `e86_2_1_fix_01_bearer_propagates_across_redirect` passes; the existing `sc_fix_01_*` and `sc_fix_02_*` RED tests in `lifecycle_resolver.rs::tests` continue to pass as **defect-documentation** |
| REQ-FIX-02 | `installer_transaction.rs::Downloading` carries `Authorization: Bearer <token>` on every hop when `COGNICODE_GITHUB_TOKEN` is set, including the redirect hop |
| REQ-FIX-03 | `installer_transaction.rs::Downloading` returns `Network("download", "DisallowedRedirectHost <host>")` when a hop redirects to a host outside `gh_trust_set()`, and does not call the next hop |
| REQ-FIX-04 | `installer_transaction.rs::Downloading` returns `Network("download", "TooManyRedirects (>5)")` when the redirect chain exceeds 5 hops |
| REQ-FIX-05 | Re-run e86.2 UAT completes Phase B with HTTP 200 (asset downloaded), `cogh list` shows mcp-server@0.95.0, and Phase D uninstall ends with `~/.cognicode/install/mcp-server/0.95.0/` removed |
| REQ-FIX-06 | All 186 (current) + new tests stay green |
| REQ-FIX-07 | `cargo fmt -p cognicode-cli --check` exits 0 |
| REQ-FIX-08 | `cargo clippy -p cognicode-cli --bin cogh --tests` (without `-D warnings`) produces no new warnings on `installer_transaction.rs` (pre-existing workspace-wide warnings are out of scope) |

## Risks

- **Token leakage to wrong host** — the trust-set check on each hop
  must be conservative. We reuse `lifecycle_resolver::gh_trust_set()`,
  which is already `pub`. Any host outside that set stops the redirect
  chain and returns `DisallowedRedirectHost`. CogniCode installers do
  not legitimately follow redirects to third-party mirrors via
  GH-releases (those use `--base-url` instead), so this is the
  intended behaviour.
- **reqwest API drift** — the manual loop uses only stable reqwest
  APIs (`get`, `bearer_auth`, `send`, response `.headers()`). No
  dependency on the redirect-policy machinery at all, so any drift
  in `Policy::custom` does not affect us.
- **Concurrent fixes to other modules** — E86.2.1 only modifies
  `installer_transaction.rs` (the production site of the bug); the
  resolver's `http_get` is intentionally left alone in this cycle
  because `api.github.com` does not currently redirect cross-host.
  If GH changes posture, a follow-up E86.2.2 will adopt the same
  manual loop in `lifecycle_resolver::http_get`. Keep E86.2.1 self-
  contained.
- **Tests with raw TCP listeners** — fragile in CI; use std
  `std::net::TcpListener` carefully with a per-test OS-allocated port
  (`TcpListener::bind("127.0.0.1:0")`). No global port constants.
- **Max-hop budget (5)** — defensive cap, GH reality is 1–2 hops.
  Captured by an env var `COGH_MAX_REDIRECTS` deferred to a follow-up
  if any user reports legitimate chains longer than 5.

## Deliverables (apply phase)

1. New helper `download_with_bearer()` and refactor of the
   `InstallStage::Downloading` arm in
   `crates/cognicode-cli/src/cmd/installer_transaction.rs`
   (single file edit, ~80 LOC net).
2. Reuse or promotion of the `bind_one_shot_302` / `bind_one_shot_ok` /
   `header_value` test helpers already added during the RED phase in
   `lifecycle_resolver.rs::tests`. If duplicated into
   `installer_transaction.rs::tests` for symmetry, leave a
   `// SHARED-WITH` comment pointing at the originals so a future
   `cmd::test_support` consolidation has a clear migration path.
3. Three new tests (`e86_2_1_fix_01..03`) under
   `installer_transaction.rs::tests`.
4. Annotation comment block `// DEFECT-DOC` above the two RED tests
   in `lifecycle_resolver.rs::tests` explaining what they prove.
5. `openspec/changes/e86-2-1-bearer-on-redirect-fix/specs/bearer-redirect/spec.md`
   (Given/When/Then for REQ-FIX-01..05) — written by the apply cycle
   alongside the implementation, since the spec is operationally the
   test descriptions.
6. Updated `openspec/changes/e86-2-cogh-real-user-uat/uat-receipt.md`
   flipping Phase B from PARTIAL to GREEN.
7. Apply receipt with RED-first proofs and the UAT re-run evidence.

## Non-goals

- Adopting `cargo-dist` (ADR-052, out of scope)
- Adding GPG signature verification (out of scope; future cycle)
- Mirroring the asset CDN trust policy to other Git-like services
  (GitLab, etc.) — none in the Tier-1 platform list today
- Migrating `lifecycle_resolver::http_get` to a manual loop. The
  endpoints it calls (`/repos/.../releases/latest`, `/tags/...`) do
  not currently cross-host; deferred to E86.2.2 if needed.

## Sequencer

E86.2.1 is **blocked-by nothing** except the e86.2 UAT finding (which is
committed in commit `10768064`). Itself unblocks E86.2's
`uat-receipt.md` to flip from PARTIAL to GREEN, which then permits E86.3
to begin with a clear "install actually works on a real user PC" baseline.

## Open question

**Q1.** Should `download_with_bearer()` be public (reusable from
`registry.rs` and `layout.rs` later) or stay a private fn inside
`installer_transaction.rs`? The current call sites in those other
modules do not show the bug (their URLs are direct, not redirected),
so we keep the helper private to minimise blast radius. **Lean:
private**. Final decision in apply phase.
