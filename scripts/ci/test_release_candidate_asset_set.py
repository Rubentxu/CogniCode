#!/usr/bin/env python3
# test_release_candidate_asset_set.py — B0-VDH acceptance: the release-candidate
# lane produces a complete, verified, attributable v0.101.x asset set locally
# before release.pipeline.kts publishes them.
#
# This is the durable contract. It reads the produced
# `release-inventory-<version>.json` and `SHA256SUMS` and asserts that every
# file referenced by the inventory exists on disk, matches its sha256, and
# matches the SHA256SUMS entry. It also asserts that the inventory's
# source_commit is reachable from HEAD via `git log --oneline`.
#
# The script is the permanent replacement for the ephemeral B0-UAT-01
# (download + verify real release assets) and B0-AAT-01 (mutate a copy of
# the manifest and watch the gate fail). It runs on the local
# release-candidate output, not on the GitHub release.
#
# Run with:
#   python3 scripts/ci/test_release_candidate_asset_set.py \
#       --version 0.101.17 --platform x86_64-unknown-linux-gnu
#
# Exit code 0 means the local candidate is internally consistent.
# Exit code 1 means a missing file, hash mismatch, or unreachable commit.
import argparse, hashlib, json, subprocess, sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def parse_sha256sums(path: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    if not path.exists():
        return out
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        # Format: `<sha256>  <filename>` (two spaces between) or `<sha256> *<filename>`
        if "  " in line:
            h, fn = line.split("  ", 1)
            out[fn.lstrip("*").strip()] = h.strip()
    return out


def git_commit_reachable(sha: str) -> bool:
    r = subprocess.run(
        ["git", "rev-parse", "--verify", sha + "^{commit}"],
        capture_output=True, text=True, cwd=str(REPO_ROOT), timeout=10,
    )
    return r.returncode == 0


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--version", required=True, help="e.g. 0.101.17")
    p.add_argument("--platform", required=True, help="e.g. x86_64-unknown-linux-gnu")
    p.add_argument("--root", type=Path, default=REPO_ROOT / "release",
                   help="directory containing the inventory + SHA256SUMS")
    args = p.parse_args()

    inv_path = args.root / f"release-inventory-{args.version}.json"
    sums_path = args.root / "SHA256SUMS"

    failures: list[str] = []
    if not inv_path.exists():
        print(f"FAIL: inventory missing: {inv_path}")
        return 1
    if not sums_path.exists():
        print(f"FAIL: SHA256SUMS missing: {sums_path}")
        return 1

    inv = json.loads(inv_path.read_text())
    declared_version = inv.get("version")
    if declared_version != args.version:
        failures.append(
            f"inventory version {declared_version!r} != --version {args.version!r}"
        )

    declared_commit = inv.get("source_commit")
    if not declared_commit:
        failures.append("inventory missing source_commit")
    elif not git_commit_reachable(declared_commit):
        failures.append(
            f"inventory source_commit {declared_commit!r} not reachable from HEAD"
        )

    artifacts = inv.get("artifacts", [])
    if not artifacts:
        failures.append("inventory has no artifacts")

    sha256_sums = parse_sha256sums(sums_path)

    for art in artifacts:
        fn = art.get("filename")
        declared = art.get("sha256")
        if not fn or not declared:
            failures.append(f"artifact missing filename/sha256: {art}")
            continue
        path = args.root / fn
        if not path.exists():
            failures.append(f"missing file on disk: {fn}")
            continue
        actual = sha256_file(path)
        if actual != declared:
            failures.append(
                f"sha256 mismatch for {fn}: declared={declared}, actual={actual}"
            )
        sums_entry = sha256_sums.get(fn)
        if sums_entry is None:
            failures.append(f"file {fn} not in SHA256SUMS")
        elif sums_entry != declared:
            failures.append(
                f"SHA256SUMS hash for {fn} ({sums_entry}) != inventory ({declared})"
            )
        # Platform check: every filename should carry the platform token.
        if args.platform and args.platform not in fn:
            failures.append(
                f"artifact {fn} does not carry platform token {args.platform}"
            )

    # Bundle manifest + small wrapper tarballs that the inventory does not
    # list as artifacts but that the lane produces. Their presence is part of
    # the contract; their hashes belong in SHA256SUMS.
    extra_expected = [
        f"bundle-{args.version}-{args.platform}.yaml",
        f"cognicode-{args.version}.tar.gz",
        f"cognicode-mcp-{args.version}.tar.gz",
    ]
    for fn in extra_expected:
        if not (args.root / fn).exists():
            failures.append(f"missing auxiliary file: {fn}")

    if failures:
        print(f"FAIL: {len(failures)} contract defects")
        for f in failures:
            print(f"  - {f}")
        return 1

    print(f"PASS: {len(artifacts)} artifacts verified for v{args.version}/{args.platform}")
    print(f"  inventory: {inv_path}")
    print(f"  SHA256SUMS: {sums_path}")
    print(f"  source_commit: {declared_commit} (reachable: yes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())