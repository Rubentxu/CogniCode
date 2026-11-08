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
