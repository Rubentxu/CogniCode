# e84 WU0 — Distribution Characterization

Inventory of what exists today, classified honestly. **The problem is
consistency, not absence** — most of the mechanics are real.

Classification: `IMPLEMENTED` · `PARTIAL` · `STUB` · `BROKEN_CONTRACT` · `FUTURE`

## `cogh` command surface

Source: `crates/cognicode-cli/src/bin/cogh.rs` (25 subcommands), `src/cmd/layout.rs`.

| Command | Class | Evidence |
|---|---|---|
| `cogh init` | IMPLEMENTED | `layout::cmd_init` → `home.init()` + `bundled::install_bundled_plugins` |
| `cogh install <plugin>` | PARTIAL | runs the real atomic pipeline, but the manifest it consumes is unsatisfiable (see WU1) |
| `cogh uninstall` | **STUB** | `cmd_uninstall` prints `uninstall: plugin=… version=…` and wires IDE removal; it does **not** remove `~/.cognicode/install/<version>/` |
| `cogh current` | IMPLEMENTED | reads the tracker file |
| `cogh list` | IMPLEMENTED | lists `~/.cognicode/plugins/` |
| `cogh latest` | **STUB** | prints `(latest --all: not yet implemented)` |
| `cogh update` | **STUB** | prints `(update all: not yet implemented)` |
| `cogh reshim` | **STUB** | prints `reshim: would regenerate … (not yet implemented)` |
| `cogh rollback` | **ABSENT** | the subcommand does not exist — `Rollback` appears **0** times in `cogh.rs`. Rollback exists only as *internal transaction machinery* (`rollback_journal` + the `Failed` state of `InstallerTransaction`), never as a user-facing command. |
| transaction rollback | IMPLEMENTED | on a failed stage the `InstallerTransaction` advances to `Failed` and the journal drives compensation. Safe-install is real; *user-initiated* rollback is not exposed. |
| `cogh where` | IMPLEMENTED | resolves the shim path |
| `cogh doctor` | IMPLEMENTED | `doctor::run_doctor`, 4 orthogonal dimensions |
| `cogh version` | IMPLEMENTED | `version::cmd_version` |
| `cogh plugin add/list` | PARTIAL | add + list work |
| `cogh plugin remove/update` | **STUB** | "not yet implemented … plugins are read-only in this version" |
| `cogh skill validate` | IMPLEMENTED | `skill::cmd_skill_validate` |
| `cogh ide detect/install/uninstall` | IMPLEMENTED | `ide::*` + `tests/cognicode_ide_adapter.rs` |

## Install engine

| Piece | Class | Evidence |
|---|---|---|
| `InstallerTransaction` | IMPLEMENTED | `installer_transaction.rs` (559 LOC): explicit stages Resolve → Download → SHA256 → Extract → Shims → Manifest → Commit/Fail, journal-driven |
| `rollback_journal` | IMPLEMENTED | dedicated module |
| `install_lock` | IMPLEMENTED | dedicated module |
| `verify_sha256` / `compute_sha256` | IMPLEMENTED | real hashing |
| `platform_adapter` | IMPLEMENTED | 533 LOC; Linux x86_64/aarch64, macOS x86_64/arm64, Windows x86_64 |
| `doctor` | IMPLEMENTED | dedicated module |

**The transactional core is genuine.** The observation in the cycle brief is confirmed:
`Resolve → Download → SHA256 → Extract → Shims → Manifest → Commit/Rollback` is real code, not a diagram.

## Manifest and bundle

| Piece | Class | Evidence |
|---|---|---|
| `BundleManifest` parse + schema validate | IMPLEMENTED | `bundle_manifest.rs`, `apiVersion: cognicode.bundle/v1` checked, 4 validation rules |
| `Platform` enum | IMPLEMENTED | 5 kebab-case variants, matches the release matrix table exactly |
| `ComponentKind` | IMPLEMENTED | Cogh / Cognicode / Daemon / Skill / Sandbox / ExplorerAsset / Plugin |
| `ProfileDef.include_kinds` | **INERT** | parsed into the struct, **never read for selection**. Only occurrences are the struct field and test fixtures. |
| Profile selection | **BROKEN_CONTRACT** | `components_for_profile` filters on `components[].profiles`, ignoring `include_kinds`. For `bundles/v0.95.0` the default profile `core` matches **zero** components. |
| `bundles/v0.94.1` … `v0.95.0` (5 files) | **BROKEN_CONTRACT** | all 15 `sha256` values are `000…0001/2/3` placeholders; see WU1 |
| Embedded manifest wiring | IMPLEMENTED | `include_str!(…/bundles/v{CARGO_PKG_VERSION}/bundle.yaml)` — version is a single source of truth |

The placeholder digests are **schema-valid**: `validate()` only requires
`len == 64 && all ascii hex`, so `0000…0001` passes. This is precisely the
mechanism by which a broken contract looks green.

## Producer and publication

| Piece | Class | Evidence |
|---|---|---|
| `.github/workflows/release.yml` | IMPLEMENTED but **BROKEN_CONTRACT** | 5-lane native matrix, build + install-smoke + publish; emits names the consumer does not expect (WU1) |
| install-smoke lane | IMPLEMENTED | extracts both tarballs, runs `cogh --version`, `cognicode-mcp --version`, `explorer-api --version` on a clean `HOME` |
| GitHub Releases | **BROKEN_CONTRACT** | `gh release list` returns **5 releases, every one `draft=true`**. Newest is `v0.93.0` with a single asset `cognicode-mcp-0.93.0-x86_64-unknown-linux-gnu.tar.gz`, i.e. the **old** artifact model. Nothing is published. |
| `v0.95.0` release | **ABSENT** | `bundles/v0.95.0/bundle.yaml` points at `releases/download/v0.95.0/…`; no such release exists |
| `SHA256SUMS` | **FUTURE** | not produced |
| release manifest as a published asset | **FUTURE** | the workflow uploads `dist-all/*.tar.gz` only |
| artifact attestations | **FUTURE** | absent |

## Channels and factory

| Piece | Class |
|---|---|
| `install.sh` bootstrap | **FUTURE** (does not exist) |
| `dist` / cargo-dist config | **FUTURE** (no `[workspace.metadata.dist]`, no config) |
| mise / aqua / Homebrew / asdf / Nix / cargo-binstall definitions | **FUTURE** |
| macOS / Windows release lanes | **FUTURE** — lanes are *defined* in the matrix but have never produced a published artifact |

## Non-goals honoured

No Control Plane, no Backstage, no MCP tools, no packs, no architecture UX, no IDE
adapter expansion. WU10: confirmed, nothing product-facing was touched.
