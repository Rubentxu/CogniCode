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
STALE_DB_WORDS = re.compile(r"postgres|ensure_pg|DATABASE_URL|DB_URL|pg_isready|dev-pg", re.I)


def test_explorer_real_stack_scripts_do_not_start_or_wait_for_postgresql() -> None:
    scripts = sorted(EXPLORER_SCRIPTS.glob("*.sh"))
    assert scripts, "no scripts found under scripts/explorer"
    offenders = [
        f"{path.name}:{number}"
        for path in scripts
        for number, line in enumerate(_read(path).splitlines(), 1)
        if STALE_DB_WORDS.search(line)
    ]
    assert not offenders, f"PostgreSQL leftovers: {offenders}"


def _comment_above(recipe: str) -> str:
    lines = _read(JUSTFILE).splitlines()
    index = next(i for i, ln in enumerate(lines) if ln.startswith(f"{recipe}"))
    block = []
    for line in reversed(lines[:index]):
        if not line.startswith("#"):
            break
        block.append(line)
    assert block, f"recipe {recipe} has no comment above it"
    return "\n".join(block)


def test_justfile_explorer_real_comments_do_not_name_postgresql() -> None:
    offenders = [
        r
        for r in ("explorer-real-ingest", "explorer-real-api-up")
        if re.search("postgres", _comment_above(r), re.I)
    ]
    assert not offenders, f"comments still name PostgreSQL: {offenders}"


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
MCP_TOOLS = REPO_ROOT / "crates" / "cognicode-explorer" / "MCP-TOOLS.md"


def test_developer_docs_and_dev_recipe_do_not_describe_a_removed_postgresql_backend() -> None:
    recipes = set(re.findall(r"^@?([A-Za-z][\w-]*)\b[^\n=:]*:(?!=)", _read(JUSTFILE), re.M))
    skill = _read(SKILL)
    offenders = []
    for name in re.findall(r"^just ([\w-]+)", skill, re.M):
        if name not in recipes:
            offenders.append(f"SKILL.md names `just {name}`, which is not a recipe")
    if re.search(r"Postgres", skill, re.I):
        offenders.append("SKILL.md still names Postgres")
    for recipe in ("dev",):
        if re.search(r"\bPG\b", _comment_above(recipe)) or re.search(r"PG:\s", _recipe_body(recipe)):
            offenders.append(f"justfile recipe {recipe} still names PG")
    if re.search(r"One command to rule them all: PG", _read(JUSTFILE)):
        offenders.append("justfile explorer-local comment still names PG")
    tools = _read(MCP_TOOLS)
    if re.search(r"stored in PostgreSQL|enables `postgres`", tools):
        offenders.append("MCP-TOOLS.md still describes a PostgreSQL store/feature")
    assert not offenders, f"stale PostgreSQL statements: {offenders}"
