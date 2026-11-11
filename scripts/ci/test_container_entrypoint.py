#!/usr/bin/env python3
"""The container entrypoint launches the server with flags it declares.

`docker-entrypoint.sh` once passed `--postgres` to `cognicode-mcp-server`.
The server no longer declares that flag, so clap exits 2 and the container
never serves. Nothing in the suite builds the image, so the only place the
mismatch is visible is the two files read side by side.

The contract reads the entrypoint and the server's `Args` and checks that
every `--flag` on the `cognicode-mcp-server` line is declared. It is its own
control: unreadable files fail instead of passing on nothing to compare.
"""

from __future__ import annotations

import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
ENTRYPOINT = REPO_ROOT / "docker-entrypoint.sh"
DOCKERFILE = REPO_ROOT / "Dockerfile"
SERVER = REPO_ROOT / "crates/cognicode-mcp/src/server.rs"


def _read(path: Path) -> str:
    text = path.read_text(encoding="utf-8")
    assert text.strip(), f"{path} is empty: nothing to compare"
    return text


def _declared_flags() -> set[str]:
    """Long flags of the server's `Args` struct: field names, kebab-cased."""
    source = _read(SERVER)
    body = source.split("struct Args", 1)[1].split("\n}\n", 1)[0]
    fields = re.findall(r"^\s+(?:pub\s+)?([a-z_][a-z0-9_]*)\s*:", body, re.M)
    assert fields, "no fields found in the server's Args struct"
    return {"--" + name.replace("_", "-") for name in fields}


def _server_invocation_flags() -> set[str]:
    text = _read(ENTRYPOINT)
    lines = [ln for ln in text.splitlines() if "cognicode-mcp-server" in ln and "exec" in ln]
    assert lines, "the entrypoint has no exec of cognicode-mcp-server"
    return set(re.findall(r"(?<!\S)(--[a-z][a-z0-9-]*)", " ".join(lines)))


def test_entrypoint_passes_only_flags_the_server_declares() -> None:
    unknown = _server_invocation_flags() - _declared_flags()
    assert not unknown, f"entrypoint passes flags the server rejects: {sorted(unknown)}"


def test_the_control_reads_the_declared_flags() -> None:
    assert {"--cwd", "--listen"} <= _declared_flags()


def test_entrypoint_demands_a_token_before_a_non_loopback_bind() -> None:
    text = _read(ENTRYPOINT)
    assert "COGNICODE_MCP_AUTH_TOKEN" in text, (
        "a 0.0.0.0 bind is refused by the server without a token; "
        "the entrypoint must say so before exec"
    )


def test_container_surface_does_not_ship_postgresql() -> None:
    offenders = [
        f"{path.name}:{number}"
        for path in (DOCKERFILE, ENTRYPOINT)
        for number, line in enumerate(_read(path).splitlines(), 1)
        if re.search(r"postgres|pg_ctl|pg_isready|psql|PGDATA|DATABASE_URL", line, re.I)
    ]
    assert not offenders, f"PostgreSQL leftovers: {offenders}"


JUSTFILE = REPO_ROOT / "justfile"
SANDBOX_RECIPES = ("sandbox-iac", "sandbox-all", "sandbox-run", "sandbox-plan")


def _recipe_body(name: str) -> str:
    text = _read(JUSTFILE)
    match = re.search(rf"^{name}\b[^\n]*:\s*\n((?:[ \t]+[^\n]*\n|\n)+)", text, re.M)
    assert match, f"recipe {name} not found in the justfile"
    return match.group(1)


def test_no_compose_file_for_a_database_the_runtime_does_not_use() -> None:
    assert not (REPO_ROOT / "docker-compose.yml").exists()


def test_sandbox_recipes_do_not_export_a_variable_nobody_reads() -> None:
    offenders = [n for n in SANDBOX_RECIPES if "DATABASE_URL" in _recipe_body(n)]
    assert not offenders, f"recipes still export DATABASE_URL: {offenders}"


EXPLORER_SCRIPTS = REPO_ROOT / "scripts/explorer"
VITE_CONFIG = REPO_ROOT / "apps/explorer-ui/vite.config.ts"


def test_the_real_stack_scripts_left_with_the_in_tree_explorer() -> None:
    # The scripts/explorer/ helpers built `-p cognicode-runtime --bin
    # explorer-api`, which left the workspace with the decommission. The
    # dev-up story for the real API lives in the external repository now;
    # a half-present scripts directory would be a path that names a crate
    # this tree no longer has.
    assert not EXPLORER_SCRIPTS.exists(), (
        "scripts/explorer still exists: its recipes build a crate that is "
        "no longer a workspace member"
    )


def test_justfile_no_longer_builds_the_in_tree_explorer() -> None:
    text = _read(JUSTFILE)
    offenders = [
        line.strip()
        for line in text.splitlines()
        if re.search(r"cognicode-runtime|explorer-api", line)
    ]
    assert not offenders, f"justfile still names the decommissioned crates: {offenders}"


def test_vite_config_does_not_cite_a_compose_file_that_no_longer_exists() -> None:
    assert "docker-compose" not in _read(VITE_CONFIG)


TOOLCHAIN = REPO_ROOT / "rust-toolchain.toml"


def test_dockerfile_builder_matches_the_pinned_toolchain() -> None:
    channel = re.search(r'^channel\s*=\s*"(\d+\.\d+\.\d+)"', _read(TOOLCHAIN), re.M)
    assert channel, "rust-toolchain.toml has no numeric channel"
    builder = re.search(
        r"^FROM\s+rust:(\d+\.\d+(?:\.\d+)?)\S*\s+AS\s+builder", _read(DOCKERFILE), re.M
    )
    assert builder, "the Dockerfile has no `FROM rust:<version> AS builder` stage"
    pinned = channel.group(1)
    got = builder.group(1)
    assert pinned == got or pinned.rsplit(".", 1)[0] == got, (
        f"builder rust:{got} does not match toolchain {pinned}"
    )


SKILL = REPO_ROOT / "skills" / "cognicode-developer" / "SKILL.md"


def test_developer_docs_do_not_name_recipes_or_backends_that_are_gone() -> None:
    recipes = set(re.findall(r"^@?([A-Za-z][\w-]*)\b[^\n=:]*:(?!=)", _read(JUSTFILE), re.M))
    skill = _read(SKILL)
    offenders = []
    for name in re.findall(r"^just ([\w-]+)", skill, re.M):
        if name not in recipes:
            offenders.append(f"SKILL.md names `just {name}`, which is not a recipe")
    if re.search(r"Postgres", skill, re.I):
        offenders.append("SKILL.md still names Postgres")
    assert not offenders, f"stale statements in SKILL.md: {offenders}"
