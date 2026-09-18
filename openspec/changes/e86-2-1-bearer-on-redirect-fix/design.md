# DESIGN — E86.2.1 — Fix Bearer Token Strip on Cross-Origin Redirect

> Owner: E86.2.1 (bounded cycle under E86.2)
> Status: **DESIGN — pending review before apply**
> Spec authority: `openspec/changes/e86-2-1-bearer-on-redirect-fix/proposal.md`
> Evidence: `openspec/changes/e86-2-cogh-real-user-uat/uat-receipt.md` (PARTIAL — RED defect)

---

## 1. Defect recap (verified empirically)

`cogh install --version <tag>` against a real GitHub Release fails when
the release asset is hosted on `objects.githubusercontent.com` and the
caller supplied `COGNICODE_GITHUB_TOKEN`. The asset download returns
HTTP 403 instead of 200 with body.

Cross-verified by two independent artifacts:

| evidence | result |
|---|---|
| `cogh install --version 0.95.0` (release binary `e198c7175d48fea77000adc9a3e212cf7bcc1d55f1fc65d866efba7d564b34cc`) | HTTP 403 |
| `curl -H "Authorization: Bearer $(gh auth token)" -L <asset-url>` | HTTP 200 (asset exists, 9 MB) |
| `curl --no-bearer -L <asset-url>` (no token, follow redirect) | HTTP 200 (public) |
| `curl -H "Authorization: Bearer X" --max-redirs 0` (no follow) | HTTP 302 → `objects.githubusercontent.com` |

Root cause is library-side: **reqwest 0.12.28 with default redirect policy
strips the `Authorization` header on a cross-origin redirect**, including
`github.com` → `objects.githubusercontent.com` (same organization,
different host). The same stripping occurs through `Policy::custom(|a| a.follow())`,
confirmed empirically by `sc_fix_01_default_policy_strips_authorization_on_cross_host_redirect`
in `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs::tests`.

There is no public reqwest API to re-attach stripped headers through a
policy callback; the upstream issue
[`seanmonstar/reqwest#1040`](https://github.com/seanmonstar/reqwest/issues/1040)
documents the limitation and remains open.

## 2. Design choice

**Manual redirect loop, executed in `installer_transaction.rs::Downloading`.**

Rationale:

- Reqwest's auto-redirect cannot be patched through `Policy::custom` to
  re-inject stripped headers. Empirically confirmed.
- A manual loop lets us **attach the bearer on every hop** explicitly and
  also enforce a host allow-list at each step (defense-in-depth against
  any GH-redirect chain that ends outside the trust set).
- The change is **local** to a single function in a single file.
- The change is **observable**: a new `Network::DisallowedRedirectHost`
  error variant makes "we stopped the redirect" visible in error
  envelopes; no silent failure.
- The change reuses the existing `HEAD`-then-`GET` switch that GH
  already provides (302 response: header `Location` + method preserved).

Rejected alternatives (recorded for cycle book-keeping):

1. **`Policy::custom` with `.follow()` (attempted in earlier RED test).**
   Fails empirically — same stripping behaviour as the default policy.
2. **Feature-flag through reqwest env var.** No such env var exists in
   reqwest 0.12.28.
3. **Vendor a forked reqwest.** Disproportionate; conflict resolution
   and version drift; rejected.
4. **Disable auth, rely on public release assets.** Breaks REQ-FIX-02
   (private / early-access releases need token auth). Rejected.
5. **Always re-issue with a fresh `reqwest::Request` for every hop.** We
   adopt this design — it is option 5 refined into option 1 of the
   single-function manual loop.

## 3. Components and shape

### 3.1 `installer_transaction.rs::Downloading`

The current `Downloading` arm (line 127) calls:

```rust
let mut response = client.get(resolve_download_url(&comp.url)).send()?;
```

with `client` built by `Client::builder().timeout(60s).build()` (no
redirect policy). This change replaces that branch with a helper:

```rust
fn download_with_bearer(
    client: &reqwest::blocking::Client,
    url: &str,
    bearer: Option<&str>,
) -> Result<(PathBuf, reqwest::blocking::Response), InstallerError>;
```

The helper:

1. Builds the first request with the bearer.
2. Calls `client.get(url).send()` (the client still has default
   redirect policy — leaving it gives us a single-hop fall-back if a
   future reqwest upstream relaxes the strip, with no manual-loop
   changes needed). For reqwest 0.12.28 the bearer will be stripped on
   the cross-origin hop, so the helper detects "Location" header on
   status 301/302/303/307/308 and re-issues `client.get(new_url)
   .bearer_auth(token).send()` until status is final or max hops hit.
3. **Trust set**: each hop's host must be in `gh_trust_set()`. If a hop
   redirects to a host outside the trust set, return
   `InstallerError::Network("download", "DisallowedRedirectHost <new_host>")`
   without following. The trust set is `lifecycle_resolver::gh_trust_set()`
   (already exported as `pub fn`, see §3.2).
4. **Max hops**: 5. GH's actual release-asset chain is at most 2 hops
   (api → codeload / objects). 5 leaves generous slack.
5. **Final hop**: the response (status, body) is consumed by the caller
   which writes to disk.

### 3.2 `lifecycle_resolver.rs::gh_trust_set`

Already exported (public) as `pub fn gh_trust_set() -> &'static [&'static str]`,
containing the static list:

```
github.com
api.github.com
objects.githubusercontent.com
raw.githubusercontent.com
*.githubusercontent.com
*.github.io
```

`installer_transaction.rs` imports it via

```rust
use super::lifecycle_resolver::gh_trust_set;
```

The `Downloading` helper normalizes each hop's host with
`is_in_trust_set(host)`. The existing RED-only test
`sc_fix_02_untrusted_redirect_target_is_not_followed` already proves
*reqwest alone* will stop on untrusted host (via default policy strip +
not-following for cross-host); the helper adds a defense layer
*inside* `Downloading`.

### 3.3 `InstallerError::Network` error envelope

Already supports the variant shape used in production logs:

```rust
Network(/* url */ String, /* reason */ String)
```

`DisallowedRedirectHost <new_host>` becomes the *reason*. Existing
handling at the surface surfaces the URL + reason to the user. No new
variant needed.

## 4. Behaviour matrix

| initial URL | hops | auth | expected outcome |
|---|---|---|---|
| `https://api.github.com/.../releases/latest` → redirect → `https://objects.githubusercontent.com/...` | 1 | bearer | asset body, 200 |
| public asset (no token) | 0–1 | none | asset body, 200 |
| `https://api.github.com/...` → 302 → `https://attacker.example.com/asset` | n/a | bearer | `Network("download", "DisallowedRedirectHost attacker.example.com")` |
| 302 → `https://example.net/asset` (not in trust set) | n/a | bearer | same as above |
| chain longer than 5 hops (defensive) | 6+ | bearer | `Network("download", "TooManyRedirects (>5)")` |
| 4xx / 5xx on any hop | n/a | any | existing `Network(url, "HTTP <status>")` |

## 5. Tests (apply-phase additions)

Three new tests in `crates/cognicode-cli/src/cmd/installer_transaction.rs::tests`,
re-using the `bind_one_shot_*` helpers already in `lifecycle_resolver.rs`
(promote them to a shared `test_http` module under `cmd::test_support` if
the GREEN cycle hits duplication, or duplicate the small helpers —
either is acceptable).

| test id | given | when | then |
|---|---|---|---|
| `e86_2_1_fix_01_bearer_propagates_across_redirect` | A→B chain on the trust set, `COGNICODE_GITHUB_TOKEN` set | `download_with_bearer(url, Some(token))` | B receives `Authorization: Bearer <token>`, body returns 200 |
| `e86_2_1_fix_02_untrusted_redirect_host_returns_disallowed_error` | A→C where `C ∉ trust_set` | call | returns `Err(Network("download", "DisallowedRedirectHost C"))` |
| `e86_2_1_fix_03_too_many_hops_returns_error` | synthetic 6-hop chain | call | returns `Err(Network("download", "TooManyRedirects (>5)"))` |

The RED tests already present in `lifecycle_resolver.rs::tests` are
retained as **defect-documentation tests** even after the apply cycle
completes: they show empirically that reqwest 0.12.28's default policy
strips the header, which is the entire reason we ship this design.

## 6. Feature flag, rollback, observability

- **Feature flag**: none. The change activates for every install.
  Defensible because: (a) the previous behaviour was broken for any
  token-authed install; (b) the new behaviour is at-least-as-strict as
  the old (extra trust-set enforcement); (c) rollback is a single git
  revert.
- **Rollback**: `git revert <merge-sha>` of E86.2.1.
- **Logging**: `Downloading` prints a single info line per asset
  `[cogh install] asset=<name> hops=<N> host=<final>` at the end. Stays
  out of the JSON envelope (CLI is plain-text stdout today).
- **UAT re-run**: a fresh UAT on a disposable clone (the same harness
  the E86.2 UAT used) is mandatory before E86.2.1 can be CLOSED. The
  UAT will succeed only after the apply phase lands.

## 7. Risk surface (top three)

| risk | likelihood | impact | mitigation |
|---|---|---|---|
| 5-hop limit too low for some future GH mirror | low | low (returned `TooManyRedirects` with usable error) | env var `COGH_MAX_REDIRECTS` later if any user reports it |
| Some release-asset URL is a redirect *off* the trust set legitimately | very low | low (returns `DisallowedRedirectHost` error) | user files an issue; we extend `gh_trust_set()` |
| reqwest upgrade later silently re-attaches the header | low | medium (manual loop would double-attach) | the manual loop checks "Authorization already set?" before attaching; idempotent |

## 8. Out of scope

- Same fix for `http_get` (`lifecycle_resolver.rs::fetch_release_latest`,
  `fetch_release_by_tag`). Those endpoints do not currently redirect
  cross-host (`api.github.com` serves them directly). If GH changes
  posture, a follow-up proposal E86.2.2 will adopt the same manual loop.
- `update` and `uninstall` flows. These do not download assets
  (update reuses `install`, uninstall reads tracker), so they inherit
  the fix transparently.
- SSE / streaming bodies. The current installer reads the full body
  into memory (`response.bytes().unwrap()`). The manual loop preserves
  this.

## 9. Acceptance gates

The apply cycle is GREEN only when:

1. `cargo test -p cognicode-cli --bin cogh` passes 0 / 0 failed
   (currently 186 / 0; expect 189 / 0 after the three new tests).
2. `cargo fmt -p cognicode-cli --check` is clean.
3. `cargo clippy -p cognicode-cli --bin cogh --tests` (without
   `-D warnings`) produces no new warnings on
   `installer_transaction.rs`. (Pre-existing warnings on the wider
   workspace are out of scope.)
4. A fresh UAT on the same disposable harness used in E86.2 produces
   a GREEN receipt: `cogh install --version 0.95.0` exits 0, asset
   written to disk, version tracked.
5. `cogh install --version 0.95.0` runs in <90 s end-to-end on the
   disposable harness (current baseline: TBD; baseline captured in
   the GREEN UAT receipt).

## 10. Decision summary

| question | answer |
|---|---|
| where the fix lives | `installer_transaction.rs::Downloading` (single function) |
| how the bearer is preserved | manual redirect loop with `Authorization` re-attached per hop |
| what counts as "trustworthy redirect" | `lifecycle_resolver::gh_trust_set()` (existing, public) |
| how many hops allowed | 5 (defensive; GH reality: 1–2) |
| what new error variant | none — reuses `Network(url, reason)` |
| rollback plan | single `git revert` |
