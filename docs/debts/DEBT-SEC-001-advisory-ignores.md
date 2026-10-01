# DEBT-SEC-001 — Advisory ignore register for `deny.toml`

**Status:** current · **Opened:** 2026-10-01 · **Scope:** `[advisories] ignore` in [`deny.toml`](../../deny.toml)
**Enforced by:** `crates/cognicode-cli/tests/advisory_ignore_backing_contract.rs`

## What this document is

`deny.toml` is the blocking supply-chain gate for releases: it runs at
`release.yml:128` and `release-validate.yml:123`, and `merge-gate` is the only
required check on `main`. Its own header used to say that every ignore below it
had "a reason and a recorded fix path in
`docs/prf/specs/RECONCILIATION-MATRIX.md` and JOURNAL §55".

That was false, and it was false in the way this repository has been bitten
several times before: a written guarantee that no mechanism enforces.

- `RECONCILIATION-MATRIX.md` carries one summary sentence at line 44 claiming
  "5 deudas documentadas". It has no per-advisory row. None of the five ids
  appears in it.
- JOURNAL §55 is "cerrando el hueco que N+54 dejo declarado abierto", about the
  A-013 lifecycle gate. It is not about advisories.
- `docs/prf/` is frozen historical evidence and is not reopened to fix this, so
  the backing record lives here instead.

The real reasoning was in `docs/roadmap/JOURNAL.md:3584-3588`, where CR-07 was
explicitly deferred because breaking `/metrics` in production would be worse
than the advisory. A real trade-off, recorded in the wrong place and cited
wrongly. It is quoted below rather than lost.

## Current ignores

Every row was verified against the advisory database on **2026-10-01** by
running `cargo deny check advisories` with the ignore list emptied and reading
the reported crate, version and solution line. The "why" and "fix path" columns
below are what the tool says, not what was assumed.

| Advisory ID | Crate | Version in tree | Class | Why it is ignored | Fix path | Owner | Authorising record |
|---|---|---|---|---|---|---|---|
| RUSTSEC-2024-0384 | `instant` | 0.1.13 | unmaintained | Crate is no longer maintained; its author recommends the maintained `web-time` crate instead. Advisory `Solution:` is "No safe upgrade is available!". Arrives as `cognicode-core -> notify 7.0.0 -> notify-types 1.0.1`. | Drop the transitive `notify 7` dependency, which requires moving `cognicode-core` to `notify 8` or replacing the file-watching use. No version bump fixes this on its own. | Repo maintainer (no `.github/CODEOWNERS` exists, so ownership is by convention, not by file) | `a886ecfd` (2026-09-22), which declared `cargo deny check advisories: ok` at the time |
| RUSTSEC-2025-0141 | `bincode` | 2.0.1 | unmaintained | The bincode team ceased development permanently after a doxxing and harassment incident. Advisory `Solution:` is "No safe upgrade is available!". It is a **direct** dependency of `cognicode-core`, not a transitive one. | Migrate the serialisation format. Advisory names `wincode`, `postcard`, `bitcode` and `rkyv` as alternatives. Any of them is a breaking change to the on-disk format. | Repo maintainer | `a886ecfd` (2026-09-22) |
| RUSTSEC-2026-0192 | `ttf-parser` | 0.25.1 | unmaintained | The author states the crate is unmaintained and will not receive further fixes. Advisory `Solution:` is "No safe upgrade is available!". Arrives as `cognicode-core -> mermaid-rs-renderer 0.2.2 -> fontdb 0.23.0`. | Wait for, or move to, `skrifa`, the actively maintained TrueType/OpenType parser named in the advisory. This is inside a transitive renderer, so the first move is upstream. | Repo maintainer | `a886ecfd` (2026-09-22) |
| RUSTSEC-2024-0437 | `protobuf` | 2.28.0 | **vulnerability** | Affected versions do not properly parse unknown fields in user-supplied input, allowing a stack overflow on untrusted data. This is a real vulnerability, not a hygiene notice. | Upgrade to `>= 3.7.2`. Requires `opentelemetry-prometheus 0.28` (`prometheus 0.14`, `protobuf 3`), which is the `opentelemetry 0.27 -> 0.28` major migration. Deliberately deferred, see the quoted decision below. | Repo maintainer | `a886ecfd` (2026-09-22) and the CR-07 deferral at `docs/roadmap/JOURNAL.md:3584-3588` |

### The CR-07 deferral, quoted rather than lost

> **NO** se tocó CR-07 (OTel 0.28 / protobuf advisory).
> Riesgo alto: API OTel cambia entre 0.27 → 0.28 → 0.33 (actual).
> Romper `/metrics` en producción sería peor que el advisory.
> Merece un ciclo dedicado con PR review del operador.

— `docs/roadmap/JOURNAL.md:3584-3588`

This is the only row carrying a deliberate, reviewed trade-off. The other three
are `unmaintained` notices with no available fix, which is a weaker category and
should not be read as equivalent to accepting a known vulnerability.

## Retired ignores

Retirements are recorded here rather than deleted silently. Each was verified as
`advisory-not-detected` by `cargo deny` before removal, and uses the same column
layout as the table above so that the contract treats both uniformly.

| Advisory ID | Crate | Version at retirement | Class | Why it was ignored while it was listed | Fix path applied | Owner | Authorising record |
|---|---|---|---|---|---|---|---|
| RUSTSEC-2023-0057 | `libc` | 0.2.189 | unsound | Claimed: "libc pre-main std access (via crossbeam-deque build path); fix: libc bump". The advisory matched nothing: `libc 0.2.189` is past the affected range, so the ignore carried no risk. | None needed — the dependency moved on its own. Recorded 2026-10-01 so the removal is auditable; `unused-ignored-advisory = "deny"` in `deny.toml` now makes a dead ignore impossible to leave behind quietly. | Repo maintainer | Retired 2026-10-01 under SDDK WorkItem `367ca65e`, evidence `a00449b0` |

## Known debt this register does NOT cover

**`yoke-derive 0.8.3` is yanked and the advisories gate fails because of it.**

`deny.toml` sets `yanked = "deny"`, no ignore covers it, and `cargo deny check
advisories` reports `error[yanked]: detected yanked crate`. It arrives purely
transitively: `cognicode-core -> lsp-types 0.93.2 -> url 2.5.8 -> idna 1.1.0 ->
icu_collections 2.3.0 -> yoke 0.8.3 -> yoke-derive 0.8.3`.

`cargo update -p yoke --dry-run` reports `Locking 0 packages to latest compatible
versions`, so there is no compatible bump available today. Resolving it means
advancing the `lsp-types` / `url` / `idna` chain or waiving the yank policy, and
then re-validating the workspace build. That is tracked separately; it is not
absorbed into the ignore list, because a yanked crate is not an advisory and
silencing it as one would hide a different problem behind this document.

Consequence, stated plainly: **the advisories gate is red as of 2026-10-01**,
on the yank alone. It is not reported as green anywhere in this repository.

## Adding or retiring an ignore

1. Get the row from `cargo deny check advisories`, not from memory. The tool
   reports the crate, the version and the `Solution:` line; the previous header
   in `deny.toml` was wrong about two of the four current rows precisely because
   it was written from memory.
2. Add or update the row in the table above, with a non-empty fix path and a
   named authorising record.
3. `advisory_ignore_backing_contract` fails if a listed ignore has no row, or if
   a row exists for an id that is neither listed nor retired.
4. `unused-ignored-advisory = "deny"` means an ignore that no longer matches
   anything is an error, not a warning.
