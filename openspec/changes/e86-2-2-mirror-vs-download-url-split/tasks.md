# TASKS — E86.2.2 — Mirror-vs-download URL split

> Source: openspec/changes/e86-2-2-mirror-vs-download-url-split/{proposal.md, specs/url-split/spec.md}

## Task 1 — Env-var rename in code

**Files**:
- `crates/cognicode-cli/src/cmd/installer_transaction.rs`
- `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs`
- `crates/cognicode-cli/src/cmd/layout.rs` (only if it also reads the old name)

**Steps**:

1. Rename `ENV_RELEASE_BASE_URL = "COGNICODE_RELEASE_BASE_URL"` to
   `ENV_API_BASE_URL = "COGNICODE_API_BASE_URL"` in
   `lifecycle_resolver.rs`. Default stays `https://api.github.com`.
2. Add a new `ENV_ASSET_BASE_URL = "COGNICODE_ASSET_BASE_URL"` in
   `installer_transaction.rs`. Default is *empty* (no rewrite).
3. Rewrite `resolve_download_url()`:
   - If `ENV_ASSET_BASE_URL` is set and non-empty: rewrite canonical
     `RELEASE_DOWNLOAD_BASE/<rest>` → `<asset_base>/<rest>`.
   - Else if `ENV_RELEASE_BASE_URL` is set, non-empty, AND its host
     is `api.github.com`: emit deprecation warning, return canonical.
   - Else return canonical.
   - URLs that don't begin with `RELEASE_DOWNLOAD_BASE` pass through
     unchanged regardless of env vars (covers REQ-86-2-2-01b).
4. Update `lifecycle_resolver::http_get` to read `ENV_API_BASE_URL`
   instead of the old name. Update existing tests that depend on
   `COGNICODE_RELEASE_BASE_URL` to set the new var.
5. Search the workspace for any other `COGNICODE_RELEASE_BASE_URL`
   references (`grep -rn 'COGNICODE_RELEASE_BASE_URL' crates/`); update
   each or wrap in a compatibility alias.

**Acceptance**: REQ-86-2-2-01a/b/c/02/03 in spec.

## Task 2 — Test additions

**File**:
- `crates/cognicode-cli/src/cmd/installer_transaction.rs::tests`
- `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs::tests`

**Tests to add**:

- `t_e86_2_2_01a_resolve_download_url_honors_asset_base`
  → use env guard helper to set/unset; check rewrite behavior.
- `t_e86_2_2_01b_resolve_download_url_passes_through_non_canonical` →
  input `https://example.com/whatever`, regardless of env var.
- `t_e86_2_2_01c_resolve_download_url_canonical_when_unset` → default.
- `t_e86_2_2_03_deprecation_warning_on_old_var_with_api_host` →
  `COGNICODE_RELEASE_BASE_URL=https://api.github.com` set; capture
  stderr, assert contains "deprecated".
- `t_e86_2_2_02_resolver_reads_api_base_url` → set
  `COGNICODE_API_BASE_URL=http://127.0.0.1:PORT/api`, fire one 200 OK
  server, call `fetch_release_latest` with `COGNICODE_RESOLVE_*`
  fixtures; assert request reached `127.0.0.1:PORT`, not the
  hardcoded api.github.com.

**Acceptance**: REQ-86-2-2-04 (suite green).

## Task 3 — UAT script update

**File**: `/tmp/cogh-uat-real-pc.sh`

**Steps**:

1. Remove the `export COGNICODE_RELEASE_BASE_URL=...` line at top.
2. Add `phase_a_prefetch` step called from `phase_a()` (insert before
   `disposable_setup`). The function:
   - Reads `UAT_VERSION` env var (default `0.95.0`).
   - Computes bundle URL:
     `${COGNICODE_API_BASE_URL:-https://api.github.com}/repos/${GH_REPO}/releases/download/v${UAT_VERSION}/bundle-${UAT_VERSION}-x86_64-unknown-linux-gnu.yaml`
     *— but the bundle is on `github.com` not `api.github.com`, so we
     use the canonical `https://github.com/${GH_REPO}/releases/download/v${UAT_VERSION}/bundle-${UAT_VERSION}-<platform>.yaml`.*
   - `curl -sSfL -H "Authorization: Bearer $(gh auth token)" "$url" -o "$UAT_ROOT/receipt/bundle.yaml"`.
   - On failure, abort Phase A.
   - Compute `sha256sum "$UAT_ROOT/receipt/bundle.yaml"` and log it.
3. Export `COGNICODE_BUNDLE_MANIFEST=$UAT_ROOT/receipt/bundle.yaml`
   in the main script body so Phase B sees it.

**Acceptance**: REQ-86-2-2-05.

## Task 4 — Run full gates

```bash
cargo test -p cognicode-cli --bin cogh
cargo fmt -p cognicode-cli --check
cargo clippy -p cognicode-cli --bin cogh --tests
```

**Acceptance**: REQ-86-2-2-04, REQ-86-2-2-06 (no new clippy warnings
on touched files).

## Task 5 — UAT re-run, full pipeline

**Run**:

```bash
rm -rf /tmp/cogh-uat-real-pc-862
mkdir -p /tmp/cogh-uat-real-pc-862
UAT_ROOT=/tmp/cogh-uat-real-pc-862 \
COGH_BIN=/var/home/rubentxu/cargo-targets/release/cogh \
COGNICODE_GITHUB_TOKEN="$(gh auth token)" \
bash /tmp/cogh-uat-real-pc.sh
```

**Acceptance**: REQ-86-2-2-06 — Phases B/C/D run to completion,
`uat-receipt.md` flips `REQ-UAT-01..04` to PASS.

## Task 6 — Apply receipt + commit

**File**: `apply-receipt.md` mirroring the E86.2.1 template:
- SHA of the apply commit,
- diff stat,
- per-task evidence (loc + line + outcome),
- decision on the open question Q1 from the proposal (asset var name;
  either `COGNICODE_ASSET_BASE_URL` as proposed or another name the
  apply discovered was more consistent — record reasoning).
- new binary SHA-256.

Commit format:

```
fix(cogh install): split asset base URL from API base URL (E86.2.2)
```

(Local; no push.)

## Task 7 — Update uat-receipt.md to GREEN

Edit `openspec/changes/e86-2-cogh-real-user-uat/uat-receipt.md`:

- Replace the PARTIAL result line with `PASS (HTTP closed in E86.2.1,
  asset-vs-API split closed in E86.2.2, full UAT green in this re-run).
`
- Update REQ-UAT-01..04 status to PASS.
- Add the new binary SHA-256 (from Task 6).
- Cite E86.2.2 commit SHA in the references.

## Task 8 — Cycle closure (archive)

After Task 7 lands and the next-bound (E86.3) is staged, archive the
E86.2.2 docs by moving the proposal/spec/tasks/apply-receipt tree
under `openspec/changes/archive/2026-09-18-e86-2-2-mirror-vs-download-url-split/`.
Commit message: `archive(e86.2.2): promote cycle docs to openspec/changes/archive/`.
Follows the e86.1 / e86.2.1 precedent.
