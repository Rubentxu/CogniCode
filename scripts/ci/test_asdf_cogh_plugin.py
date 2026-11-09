#!/usr/bin/env python3
"""Behaviour contract for the asdf `cogh` plugin (scripts/distribution/asdf-cogh).

The plugin is the only asdf route to cogh, and it runs on a user's machine, so
its guarantees are checked against a local fixture instead of the network:
`COGH_REPO_URL` points at a directory (curl reads `file://`) and at a local git
repository (for `git ls-remote`). Nothing here touches the real release.

Guarantees:
- list-all prints the semver of every `v*` tag, in version order;
- latest-stable prints the highest plain semver;
- download verifies the tarball against SHA256SUMS and REFUSES a tampered one;
- download refuses a tarball that has no SHA256SUMS entry;
- install puts an executable `cogh` under ASDF_INSTALL_PATH/bin.
"""

import hashlib
import io
import os
import platform
import subprocess
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLUGIN = ROOT / "scripts" / "distribution" / "asdf-cogh" / "bin"
TARGET = "x86_64-unknown-linux-gnu"
DEADLINE_SECONDS = 60
GIT_IDENTITY = {
    "GIT_AUTHOR_NAME": "Ruben",
    "GIT_COMMITTER_NAME": "Ruben",
    "GIT_AUTHOR_EMAIL": "rubentxu74@gmail.com",
    "GIT_COMMITTER_EMAIL": "rubentxu74@gmail.com",
}


def _supported_host():
    return platform.system() == "Linux" and platform.machine() == "x86_64"


def _env(extra=None):
    env = {
        "PATH": os.environ["PATH"],
        "HOME": tempfile.mkdtemp(prefix="asdf-cogh-home-"),
        "GIT_CONFIG_GLOBAL": "/dev/null",
        "GIT_CONFIG_SYSTEM": "/dev/null",
    }
    env.update(extra or {})
    return env


def _run(script, env, check=True):
    done = subprocess.run(
        ["bash", str(PLUGIN / script)],
        env=env,
        capture_output=True,
        text=True,
        timeout=DEADLINE_SECONDS,
    )
    if check:
        assert done.returncode == 0, f"{script} rc={done.returncode}: {done.stderr}"
    return done


def _tarball(version):
    payload = f"#!/bin/sh\necho 'cogh {version}'\n".encode()
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        info = tarfile.TarInfo("bin/cogh")
        info.size = len(payload)
        info.mode = 0o755
        tar.addfile(info, io.BytesIO(payload))
    return buf.getvalue()


def _release(root, version, sums=True, tamper=False):
    """Lay out `<root>/releases/download/v<version>/` like a GitHub release."""
    name = f"cogh-{version}-{TARGET}.tar.gz"
    folder = root / "releases" / "download" / f"v{version}"
    folder.mkdir(parents=True)
    data = _tarball(version)
    if sums:
        (folder / "SHA256SUMS").write_text(f"{hashlib.sha256(data).hexdigest()}  {name}\n")
    else:
        (folder / "SHA256SUMS").write_text(f"{'0' * 64}  other.tar.gz\n")
    (folder / name).write_bytes(data + b"x" if tamper else data)
    return name


def _git_repo(root, tags):
    def git(*args):
        subprocess.run(
            ["git", *args],
            cwd=root,
            env=_env(GIT_IDENTITY),
            check=True,
            capture_output=True,
            timeout=DEADLINE_SECONDS,
        )

    # Lightweight tags may point at any object, so the fixture tags the empty
    # tree and never makes a commit (no hook, no identity needed).
    git("init", "-q")
    empty_tree = "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
    subprocess.run(
        ["git", "mktree"],
        cwd=root,
        env=_env(),
        input="",
        text=True,
        check=True,
        capture_output=True,
        timeout=DEADLINE_SECONDS,
    )
    for tag in tags:
        git("tag", tag, empty_tree)


def _download(version, root, work):
    env = _env(
        {
            "COGH_REPO_URL": f"file://{root}",
            "ASDF_INSTALL_VERSION": version,
            "ASDF_DOWNLOAD_PATH": str(work),
        }
    )
    return _run("download", env, check=False), env


def test_list_all_prints_versions_in_semver_order():
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        _git_repo(root, ["v0.9.0", "v0.10.0", "v0.2.1", "not-a-version"])
        out = _run("list-all", _env({"COGH_REPO_URL": str(root)})).stdout.split()
        assert out == ["0.2.1", "0.9.0", "0.10.0"], out


def test_latest_stable_is_the_highest_plain_semver():
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        _git_repo(root, ["v0.9.0", "v0.10.0", "v0.10.0-rc1"])
        out = _run("latest-stable", _env({"COGH_REPO_URL": str(root)})).stdout.strip()
        assert out == "0.10.0", out


def test_download_accepts_a_matching_checksum():
    if not _supported_host():
        return
    with tempfile.TemporaryDirectory() as tmp:
        root, work = Path(tmp) / "repo", Path(tmp) / "work"
        _release(root, "1.2.3")
        done, _ = _download("1.2.3", root, work)
        assert done.returncode == 0, done.stderr
        assert (work / "bin" / "cogh").is_file()


def test_download_refuses_a_tampered_tarball():
    if not _supported_host():
        return
    with tempfile.TemporaryDirectory() as tmp:
        root, work = Path(tmp) / "repo", Path(tmp) / "work"
        _release(root, "1.2.3", tamper=True)
        done, _ = _download("1.2.3", root, work)
        assert done.returncode != 0, "a tampered tarball was accepted"
        assert not (work / "bin" / "cogh").exists(), "tampered payload was extracted"


def test_download_refuses_a_tarball_without_a_checksum_entry():
    if not _supported_host():
        return
    with tempfile.TemporaryDirectory() as tmp:
        root, work = Path(tmp) / "repo", Path(tmp) / "work"
        _release(root, "1.2.3", sums=False)
        done, _ = _download("1.2.3", root, work)
        assert done.returncode != 0, "a tarball with no checksum entry was accepted"
        assert not (work / "bin" / "cogh").exists()


def test_install_places_an_executable_cogh():
    if not _supported_host():
        return
    with tempfile.TemporaryDirectory() as tmp:
        root, work, dest = Path(tmp) / "repo", Path(tmp) / "work", Path(tmp) / "dest"
        _release(root, "1.2.3")
        done, env = _download("1.2.3", root, work)
        assert done.returncode == 0, done.stderr
        env.update({"ASDF_INSTALL_TYPE": "version", "ASDF_INSTALL_PATH": str(dest)})
        _run("install", env)
        binary = dest / "bin" / "cogh"
        assert binary.is_file() and os.access(binary, os.X_OK)
        ran = subprocess.run([str(binary)], capture_output=True, text=True, timeout=DEADLINE_SECONDS)
        assert ran.stdout.strip() == "cogh 1.2.3", ran.stdout


def test_install_refuses_a_ref_install():
    with tempfile.TemporaryDirectory() as tmp:
        env = _env(
            {
                "ASDF_INSTALL_TYPE": "ref",
                "ASDF_INSTALL_PATH": str(Path(tmp) / "dest"),
                "ASDF_DOWNLOAD_PATH": str(Path(tmp) / "work"),
            }
        )
        assert _run("install", env, check=False).returncode != 0


if __name__ == "__main__":
    import sys

    sys.path.insert(0, str(ROOT / "scripts"))
    from run_contract_tests import main

    raise SystemExit(main([__file__]))
