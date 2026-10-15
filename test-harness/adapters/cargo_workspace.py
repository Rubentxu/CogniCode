"""Grafo de paquetes del workspace desde `cargo metadata` (TH-S01).

Se pregunta a cargo, no se adivina: los paquetes, sus targets y sus dependencias
de path salen de `cargo metadata`. Un cambio en `crates/cognicode-core/src/lib.rs`
tiene que arrastrar a quien dependa de core, y esa relación solo la conoce cargo.

Si metadata no se puede leer, el grafo queda `UNKNOWN` y el planner ejecuta todo.
No hay una tercera opción: un grafo inventado seleccionaría de menos.
"""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

GRAPH_OK = "OK"
GRAPH_UNKNOWN = "UNKNOWN"

# Un cambio aquí arriba obliga a revisar todo el workspace: afecta a cómo compila
# y cómo se ejecuta cada suite, y ninguna clausura fina lo puede demostrar.
GLOBAL_PATH_PREFIXES = (
    ".cargo/",
    "scripts/ci/",
    "sandbox/",
)
GLOBAL_PATH_NAMES = (
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain",
    "rust-toolchain.toml",
    "build.rs",
)
GLOBAL_PATH_SUFFIXES = (".pipeline.kts",)


def is_global_path(rel: str) -> tuple[bool, str | None]:
    """¿Este path obliga a reevaluar todo? Devuelve (global, razón)."""
    if rel in GLOBAL_PATH_NAMES:
        return True, f"{rel} cambia cómo compila o resuelve el workspace entero"
    if any(rel.startswith(prefix) for prefix in GLOBAL_PATH_PREFIXES):
        return True, f"{rel} pertenece a la infraestructura de CI o de sandbox"
    if rel.endswith(GLOBAL_PATH_SUFFIXES):
        return True, f"{rel} define obligaciones del gate"
    name = Path(rel).name
    if name in ("Cargo.toml", "Cargo.lock", "build.rs", "rust-toolchain", "rust-toolchain.toml"):
        return True, f"{rel} es un manifest o configuración de build dentro del workspace"
    return False, None


@dataclass(frozen=True)
class Member:
    name: str
    dir: str
    manifest: str
    targets: tuple[dict[str, str], ...] = ()
    is_proc_macro: bool = False


@dataclass
class WorkspaceGraph:
    root: Path
    members: tuple[Member, ...] = ()
    dependents: dict[str, tuple[str, ...]] = field(default_factory=dict)
    dependencies: dict[str, tuple[str, ...]] = field(default_factory=dict)
    status: str = GRAPH_OK
    reason: str | None = None

    @property
    def is_unknown(self) -> bool:
        return self.status == GRAPH_UNKNOWN

    def member_dirs(self) -> dict[str, str]:
        return {m.name: m.dir for m in self.members}

    def owner_of(self, rel: str) -> str | None:
        """Paquete dueño de un path: el prefijo de miembro más largo que encaja."""
        best: tuple[int, str] | None = None
        for member in self.members:
            prefix = member.dir.rstrip("/") + "/"
            if rel.startswith(prefix) or rel == member.dir:
                length = len(prefix)
                if best is None or length > best[0]:
                    best = (length, member.name)
        return best[1] if best else None

    def reverse_closure(self, names: set[str]) -> set[str]:
        """Los paquetes dados más todos los que dependen de ellos, transitivamente."""
        closure = set(names)
        frontier = list(names)
        while frontier:
            current = frontier.pop()
            for dependent in self.dependents.get(current, ()):  # quien depende de `current`
                if dependent not in closure:
                    closure.add(dependent)
                    frontier.append(dependent)
        return closure

    def proc_macro_members(self) -> set[str]:
        return {m.name for m in self.members if m.is_proc_macro}


def graph_from_metadata(root: Path, metadata: dict) -> WorkspaceGraph:
    """Traduce la salida de `cargo metadata` al grafo del dominio. Puro."""
    root = root.resolve()
    packages = metadata.get("packages") or []
    member_names: set[str] = set()
    members: list[Member] = []
    edges: dict[str, set[str]] = {}

    for package in packages:
        manifest = Path(package.get("manifest_path", "")).resolve()
        if not str(manifest).startswith(str(root)):
            continue
        name = package.get("name", "")
        if not name:
            continue
        member_names.add(name)
        # `kind` es una lista en cargo metadata (["lib"], ["proc-macro"], ...).
        is_proc_macro = any(
            "proc-macro" in (target.get("kind") or []) for target in package.get("targets", [])
        )
        members.append(
            Member(
                name=name,
                dir=str(manifest.parent.relative_to(root)),
                manifest=str(manifest.relative_to(root)),
                targets=tuple(
                    {
                        "name": target.get("name", ""),
                        "kind": ",".join(target.get("kind") or []),
                        "src_path": str(Path(target.get("src_path", "")).resolve()),
                    }
                    for target in package.get("targets", [])
                ),
                is_proc_macro=is_proc_macro,
            )
        )
        deps: set[str] = set()
        for dependency in package.get("dependencies", []):
            dep_path = dependency.get("path")
            if dep_path:
                dep_dir = Path(dep_path).resolve()
                if str(dep_dir).startswith(str(root)):
                    deps.add(dependency.get("name", ""))
        edges[name] = deps

    dependents: dict[str, set[str]] = {name: set() for name in member_names}
    for package, deps in edges.items():
        for dep in deps:
            if dep in member_names:
                dependents.setdefault(dep, set()).add(package)

    return WorkspaceGraph(
        root=root,
        members=tuple(sorted(members, key=lambda m: m.name)),
        dependents={k: tuple(sorted(v)) for k, v in dependents.items()},
        dependencies={k: tuple(sorted(v)) for k, v in edges.items()},
        status=GRAPH_OK,
    )


def load_graph(root: Path, offline: bool = True) -> WorkspaceGraph:
    """Ejecuta `cargo metadata`. Si falla, devuelve un grafo `UNKNOWN` razonado."""
    argv = ["cargo", "metadata", "--format-version", "1"]
    if offline:
        argv.insert(2, "--offline")
    try:
        proc = subprocess.run(
            argv,
            cwd=str(root),
            capture_output=True,
            check=False,
            timeout=300,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return WorkspaceGraph(
            root=root.resolve(),
            status=GRAPH_UNKNOWN,
            reason=f"no se pudo ejecutar cargo metadata: {exc}",
        )
    if proc.returncode != 0:
        detail = proc.stderr.decode("utf-8", "replace").strip().splitlines()
        tail = detail[-1] if detail else "sin salida de error"
        return WorkspaceGraph(
            root=root.resolve(),
            status=GRAPH_UNKNOWN,
            reason=f"cargo metadata falló con {proc.returncode}: {tail}",
        )
    try:
        metadata = json.loads(proc.stdout.decode("utf-8"))
    except json.JSONDecodeError as exc:
        return WorkspaceGraph(
            root=root.resolve(),
            status=GRAPH_UNKNOWN,
            reason=f"cargo metadata devolvió JSON ilegible: {exc}",
        )
    return graph_from_metadata(root, metadata)
