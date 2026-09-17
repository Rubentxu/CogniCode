# e85 — Verification Report

## Executed evidence (real, reproducible)

| Evidence | Command | Result |
|---|---|---|
| `cogh`-binary suite | `cargo test -p cognicode-cli --bin cogh` | **147 passed / 0 failed** |
| Release-tool suite | `cargo test -p cognicode-cli --bin cognicode-release` | **35 passed / 0 failed** |
| End-to-end install | `lifecycle::tests::test_clean_home_install` | installs from a generated, locally served release |
| Workspace build | `cargo build --workspace` | GREEN |
| Release build | `cargo build --release` (3 binaries) | GREEN |
| Known-failure gate | `python3 scripts/check_known_failures.py` | **exit 0 — 41 entries, matches baseline** |
| Architecture self-host | `cargo test -p cognicode-core --test architecture_self_host_e2e` | **3 passed**, incl. zero-drift-on-clean-source |
| Contract guard | `bash scripts/check-release-matrix.sh` | **RESULT: OK** |
| Formatting (touched package) | `cargo fmt -p cognicode-cli --check` | clean |

### The end-to-end test is the one that matters most

`test_clean_home_install` does not mock the contract. It:

1. builds real `tar.gz` payloads with canonical names,
2. runs the **real** `release_factory::generate_release` to produce the manifest,
3. serves them over loopback (the manifest keeps canonical `github.com` URLs;
   only the fetch origin is redirected),
4. runs the real installer, and
5. asserts the tracker, the extracted `bin/cognicode`, and the shim.

So "the producer and the consumer speak the same artifact language" is
**demonstrated**, not asserted. Before e85 the equivalent test was green only
because the default `core` profile resolved to zero components and therefore
downloaded nothing.

### Local release rehearsal (full producer pipeline, offline)

```text
smoke on clean HOME      cogh 0.95.0 / cognicode 0.95.0 / cognicode-mcp 0.95.0
packaged                 cogh-0.95.0-x86_64-unknown-linux-gnu.tar.gz      (2.8 MB)
                         cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz  (8.6 MB)
                         cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz (12 MB)
generate                 OK  payloads=3 manifests=1 sha256sums_entries=5
verify                   OK  9 checks, all green
```

The generated `bundle-0.95.0-x86_64-unknown-linux-gnu.yaml` contains **only**
`cognicode` and `cognicode-mcp` — `cogh` is absent by construction, which is the
Layer 0 / Layer 1 separation holding in the artifact itself.

## Adversarial suite (WU17) — all rejected

| Attack | Where it is caught |
|---|---|
| placeholder digest (`0000…0001`, all-zero, all-`f`) | `ArtifactDigest::parse`, at deserialisation |
| malformed digest | `ArtifactDigest::parse` |
| v1 `apiVersion`, and arbitrary `v3`/`v999`/`v2beta` | `validate` (exact-schema rule) |
| wrong artifact filename | `validate` (derived-name rule) |
| wrong platform token in the filename | `validate` |
| non-canonical URL | `validate` |
| component/bundle version mismatch | `validate` |
| duplicate component; duplicate profile name | `validate` |
| unknown profile reference; empty component profiles | `validate` |
| profile resolving to zero components | `validate` (the M7 defect, now impossible) |
| Layer 0 or meta kinds inside a bundle | `validate` (Layer rule) |
| tag != `v{version}` | `generate_release` and `verify_release` |
| missing platform lane | `build_inventory` (orphan rule) |
| non-canonical filename in staging | `build_inventory` |
| wrong version in a filename | `build_inventory` |
| payload modified after hashing | `verify_release` (digest mismatch vs SHA256SUMS) |
| manifest modified after SHA256SUMS | `verify_release` |
| phantom manifest component | `verify_release` |
| orphan payload absent from the manifest | `verify_release` |
| x86 artifact listed in an ARM manifest | `validate` + the generated-ARM test |

## Deliberately NOT executed

| Not executed | Reason |
|---|---|
| aarch64 build locally | Needs a native ARM runner; the lane exists and is exercised in CI on `ubuntu-24.04-arm`. The local rehearsal is x86_64 only. |
| `cogh update` / `latest` / `rollback` | e86. Unchanged stubs, by instruction. |
| `mise` / `install.sh` install | e87. |
| Fresh-Linux lifecycle UAT | e88. |
| macOS / Windows builds | Tier 2. Lanes and `Platform` variants preserved; not advertised. |

## Pre-existing conditions observed (not caused by e85)

- `cargo fmt --check` over the whole workspace is dirty in `cognicode-core`
  (`application/ai/boundary_tests.rs`, `application/ai/critic.rs`, and others).
  The package this cycle touched is clean. Not reformatted, because touching
  unrelated files would inflate the diff.
- `cargo test --workspace --lib` shows a non-empty failure set in
  `cognicode-core`; the known-failure checker confirms it is **exactly** the
  recorded baseline (41 entries), so it is not drift from this cycle.
