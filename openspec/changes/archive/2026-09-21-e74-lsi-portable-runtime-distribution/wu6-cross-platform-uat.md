# e74 WU6 — Cross-platform acceptance UAT

> Deliverable of e74 WU6. Spans real-runner acceptance + structured
> evidence collection. The "real runner" half cannot run on the
> development box (no macOS or Windows host available), so it is
> expressed as a script + a manifest. The script is the contract;
> the manifest is the ledger.

## 1. Acceptance contract (per lane)

Each lane must satisfy **all four** scenarios. Anything less is a
failure of PRT-005 for that lane and must produce a typed waiver
instead of being merged silently.

### Scenario A — extract + bootstrap on clean HOME

Given a tarball `cognicode-cli-vX.Y.Z-<target>.tar.gz` produced by
the `build` lane,
When the script extracts it into a clean `HOME` directory,
Then `cogh --version` exits 0 and prints a version string
containing `cognicode <version>`.

### Scenario B — install on a real user HOME

Given the same tarball and a writable user HOME,
When `cogh install --bundle <bundle-url>` runs,
Then the user ends up with `~/.cognicode/{bin/, shims/, bundles/}`,
and `cogh --version` from the installed location exits 0.

### Scenario C — wrong-platform tarball rejected loudly

Given a tarball built for a different platform,
When `cogh install` is run,
Then it exits non-zero with the message
`bundle.platform (<actual>) does not match host platform (<host>)`
or its localized equivalent
(`BundleManifest::assert_host_platform` is the gate).

### Scenario D — doctor reports capabilities honestly

Given the installed cogh,
When `cogh doctor` runs,
Then:
- Core health is Pass on a clean install.
- Isolation backend is either Pass or Unavailable
  (NEVER Fail if absent — per WU4 contract).
- The doctor output mentions each of the four dimensions:
  Core health, MCP, Native analysis, Isolation backend.

## 2. Evidence collection script

The script `scripts/e74-acceptance-evidence.sh` captures, per
invocation:

- host triple (`<arch>-<os>`),
- tarball path + SHA-256,
- each scenario's exit code + captured stdout/stderr,
- whether the doctor output passes the four-dimension contract,
- a timestamp and a run id.

It writes one `.jsonl` per lane into `evidence/e74-acceptance/`.
The format is deliberately plain so it can be diffed across runs
and concatenated across runners.

```text
evidence/
└── e74-acceptance/
    ├── linux-x86-64.jsonl
    ├── linux-aarch64.jsonl
    ├── mac-os-x86-64.jsonl
    ├── mac-os-aarch64.jsonl
    └── windows-x86-64.jsonl
```

A run that lands in the manifest must contain four records
(A, B, C, D) each with `"status": "pass"` or `"status": "fail"`
and a non-empty `"evidence"` string pointing at the log file.

## 3. Linux x86-64 lane — already evidenced

Because the `CogniCode` repository runs primarily on Linux x86_64,
the linux-x86-64 lane can be exercised live today. The script in §2
is the same one used in the install-smoke job of `release.yml`,
which makes the install-smoke lane the live source of truth for
this run.

The other four lanes (Linux aarch64, macOS x86_64, macOS arm64,
Windows x86_64) require runners. They are covered:

- by `release.yml`'s build + install-smoke steps on each native
  runner, which already satisfy scenarios A and B by construction,
- by the contract that scenarios C and D are tested in
  `cognicode-cli` unit tests on this Linux box (the `cogh install`
  wrong-platform path is exactly what `assert_host_platform`
  exercises; the doctor output contract is exactly what the
  `report_chip_layout_has_four_dimensions` test pins).

So the remaining gap is **executing scenarios A–D on a non-Linux
host**, which the agent doing e74 WU6 cannot do from this box.
That gap is the reason for the manifest.

## 4. Manifest of evidence per lane

| Lane              | Evidence type                       | Source                                | Status                              |
|-------------------|-------------------------------------|---------------------------------------|-------------------------------------|
| linux-x86-64      | Live execution + unit tests         | release.yml install-smoke + cogh cli  | ✅ evidenced                       |
| linux-aarch64     | Cross-compile install via qemu      | release.yml install-smoke + runner    | ⚠️ needs ARM runner (no qemu here) |
| mac-os-x86-64     | Runner only                         | macos-13 runner, install-smoke + cli  | ⚠️ needs macOS runner             |
| mac-os-aarch64    | Runner only                         | macos-latest runner, install-smoke + cli | ⚠️ needs macOS runner            |
| windows-x86-64    | Runner only                         | windows-latest runner, install-smoke + cli | ⚠️ needs Windows runner      |

The ⚠️ lanes are NOT merge-blockers in this cycle. They are
follow-up acceptance steps that must be executed on the matching
runner before promotion. Each ⚠️ lane carries a typed waiver
declared here:

| Waiver                          | Meaning                                                  |
|---------------------------------|----------------------------------------------------------|
| `e74-uat-pending-linux-aarch64` | install-smoke passes on runner; UAT A/B/C/D not run here  |
| `e74-uat-pending-macos-x86-64`  | install-smoke passes on runner; UAT A/B/C/D not run here  |
| `e74-uat-pending-macos-aarch64` | install-smoke passes on runner; UAT A/B/C/D not run here  |
| `e74-uat-pending-windows-x86-64`| install-smoke passes on runner; UAT A/B/C/D not run here  |

These waivers carry the same rule: silent substitution is forbidden.
The waiver is lifted by attaching real `evidence/e74-acceptance/*.jsonl`
files from a runner of that platform.

## 5. Lifting the waivers

For each ⚠️ lane, copy the `scripts/e74-acceptance-evidence.sh`
script onto a runner of that platform alongside the matching
tarball, then run:

```bash
bash scripts/e74-acceptance-evidence.sh \
    --platform mac-os-aarch64 \
    --tarball ./dist/cognicode-cli-vX.Y.Z-mac-os-aarch64.tar.gz \
    --output ./evidence/e74-acceptance/mac-os-aarch64.jsonl
```

The script writes four records (A, B, C, D). On a clean host all
four should record `"status": "pass"`. Attach the resulting `.jsonl`
to the PR that lifts the waiver; the waiver is automatically retired
when the file is merged.

## 6. Out-of-scope explicit (no surface creep)

The following are **explicitly excluded** from e74 WU6 scope and
from any waiver-lifting PR:

- signing GPG of tarballs (waivered separately, see WU5),
- notarization Apple (e76),
- Windows `.msi` / macOS `.pkg` (e76),
- self-hosted Podman orchestration (e75),
- remote workers (out of scope until e77+).

If a runner happens to be able to do these things, the evidence
script will still record only scenarios A–D. Other capabilities
have their own cycles and their own waivers.
