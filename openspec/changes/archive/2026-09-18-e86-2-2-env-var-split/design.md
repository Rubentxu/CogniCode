# DESIGN — E86.2.2 — Mirror-vs-download URL split

> Owner: E86.2.2
> Status: DESIGN (apply in flight)
> Spec: openspec/changes/e86-2-2-mirror-vs-download-url-split/specs/url-split/spec.md

## 1. Defect recap

The single env var `COGNICODE_RELEASE_BASE_URL` was doing two jobs:

1. Setting the resolver's API origin for `/repos/.../releases/latest`
   (and the by-tag variant). Default: `https://api.github.com`.
2. Setting the installer's asset-download origin. Default was *implicit*
   (canonical `github.com` URLs passed through unchanged), but the
   variable was never consulted by the installer, which instead used
   `RELEASE_DOWNLOAD_BASE` directly. So the variable *only* drove the
   resolver today.

The UAT script exported `COGNICODE_RELEASE_BASE_URL=https://api.github.com`
to override the resolver base; that worked. But the E86.2.1 cycle
inadvertently exposed a latent bug in `resolve_download_url()`: that
function **does** read `COGNICODE_RELEASE_BASE_URL` and rewrites
canonical asset URLs onto whatever the variable points at. When the
UAT script set it to `https://api.github.com`, every
`https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/...`
became
`https://api.github.com/Rubentxu/CogniCode/releases/download/v0.95.0/...`,
which returns 403 because `api.github.com` only serves JSON.

The interaction closes once we rename. After the rename, the UAT
script no longer exports the old var; the installer reads the new
asset var (which defaults to no rewrite → canonical URLs); the resolver
reads the new API var (default api.github.com). The two contracts are
now independent.

## 2. Design choice

**One-time split + deprecation shim.** Two new env vars
(`COGNICODE_API_BASE_URL`, `COGNICODE_ASSET_BASE_URL`), and a
deprecation shim that accepts the old `COGNICODE_RELEASE_BASE_URL`
*only* when set to the historical `api.github.com` value, emits a
warning, and returns canonical. Any other use of the old name
(undefined behaviour) is left as-is for one cycle and then the shim
is removed.

Rejected alternatives:

- *Hard rename with no shim*: would break any user who already set
  the old var to point at a custom mirror (documented in early e84-era
  notes but never the canonical usage). One cycle of warning is
  cheap insurance.
- *Magic rewrite of api.github.com → github.com*: hides the
  misconfiguration; breaks any user who legitimately wants the
  installer to fetch from a custom host (e.g. enterprise GH).
- *Keep one var, ignore it in installer*: that's the status quo and
  is what caused the bug; not a fix.

## 3. Components

### `lifecycle_resolver.rs` — rename

```rust
pub const ENV_API_BASE_URL: &str = "COGNICODE_API_BASE_URL";
pub const DEFAULT_API_BASE: &str = "https://api.github.com";
```

`http_get(url: &str, base_url: Option<&str>)` reads
`ENV_API_BASE_URL` (via the same precedence: arg > env > default).
`fetch_release_latest` and `fetch_release_by_tag` continue to pass
the env-derived base to `http_get`.

### `installer_transaction.rs` — add asset var + fix resolve_download_url

```rust
pub const ENV_ASSET_BASE_URL: &str = "COGNICODE_ASSET_BASE_URL";
```

`resolve_download_url(canonical)`:

1. If `canonical` does not start with `RELEASE_DOWNLOAD_BASE`, return
   unchanged. (`REQ-86-2-2-01b`: non-canonical URLs pass through.)
2. Compute the rest (path after the base).
3. If `ENV_ASSET_BASE_URL` is set and non-empty, return
   `<asset_base><rest>`. (`REQ-86-2-2-01a`.)
4. If `ENV_RELEASE_BASE_URL` is set, non-empty, AND its host is
   `api.github.com`, emit a deprecation warning and return canonical.
   (`REQ-86-2-2-03`.)
5. Else return canonical. (`REQ-86-2-2-01c`.)

The deprecation shim is implemented as an `eprintln!` so it lands in
stderr (mirrors the dev-only-fixture warning at
`installer_transaction.rs:468`). It runs at most once per process; a
`std::sync::Once` guard prevents log spam.

### UAT script update

`/tmp/cogh-uat-real-pc.sh`:

- Remove the top-level `export COGNICODE_RELEASE_BASE_URL=...` line.
- Add a new `phase_a_prefetch` step that downloads
  `https://github.com/Rubentxu/CogniCode/releases/download/v${VERSION}/bundle-${VERSION}-x86_64-unknown-linux-gnu.yaml`
  into `$UAT_ROOT/receipt/bundle.yaml`.
- Export `COGNICODE_BUNDLE_MANIFEST=$UAT_ROOT/receipt/bundle.yaml`.

`PHASE_A` updated to call `phase_a_prefetch` after the network check.

## 4. Behaviour matrix

| `ENV_ASSET_BASE_URL` | `ENV_RELEASE_BASE_URL` (legacy) | canonical URL `github.com/.../releases/download/...` | non-canonical URL |
|---|---|---|---|
| unset | unset | returned unchanged | unchanged |
| `https://mirror/assets` | unset | rewritten to `https://mirror/assets/...` | unchanged |
| unset | `https://api.github.com` | unchanged + warning | unchanged + warning |
| unset | `https://something.else` | unchanged (shim does not match → no warning) | unchanged |
| `https://mirror/assets` | `https://api.github.com` | rewritten to mirror (asset var wins); no shim warning | unchanged |

## 5. Tests

Five new tests (`installer_transaction::tests` for the URL split;
`lifecycle_resolver::tests` for the API base rename):

- `t_e86_2_2_01a_resolve_download_url_honors_asset_base`
- `t_e86_2_2_01b_resolve_download_url_passes_through_non_canonical`
- `t_e86_2_2_01c_resolve_download_url_canonical_when_unset`
- `t_e86_2_2_03_deprecation_warning_emitted`
- `t_e86_2_2_02_resolver_reads_api_base_url`

All tests serialised via a per-module mutex (since they mutate
process-global env vars).

## 6. Risk + rollback

- **Risk**: existing tests that touched `COGNICODE_RELEASE_BASE_URL`
  silently now read the new var name; if any test forgot to update,
  it would fail at runtime, not compile-time. The grep pattern in
  Task 1 step 5 forces an exhaustive find. If any are missed the
  test suite fails fast and the apply is rolled back.
- **Rollback**: single `git revert` of the apply commit + scrub the
  deprecation shim.

## 7. Acceptance gates

- `cargo test -p cognicode-cli --bin cogh` ≥193 passed, 0 failed.
- `cargo fmt -p cognicode-cli --check` clean.
- `cargo clippy -p cognicode-cli --bin cogh --tests` no new warnings
  on `installer_transaction.rs` or `lifecycle_resolver.rs`.
- `/tmp/cogh-uat-real-pc.sh` end-to-end Phases A/B/C/D all green.

## 8. Out of scope

- Removing the deprecation shim: deferred to E86.2.3 or E87.
- Per-component asset mirrors.
- Switching from `include_str!("dev-bundle.yaml")` to a generated
  dev-only fixture.
- E86.3 (uninstall coverage): unrelated; parallel track.
