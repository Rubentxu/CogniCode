"""ChangeSet desde git: merge-base, working tree y untracked (TH-S01).

Tres decisiones que no son negociables, porque son las que hacen que el plan sea
honesto:

  * La base por defecto es el **merge-base** con la rama base, no `HEAD~1`. Con
    `HEAD~1`, una rama con dos commits aparece medio vacía y el gate cree que no
    hay nada que probar.
  * Todo se lee con formatos `-z`. Un nombre con espacios o saltos de línea es un
    nombre legal en git, y partirlo por líneas convierte un cambio de test en un
    "no cambió nada".
  * Si no hay base confiable, el resultado es `UNKNOWN`, nunca `noop`. Un plan
    vacío por accidente es un gate que aprueba sin mirar.
"""

from __future__ import annotations

import hashlib
import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

CHANGESET_OK = "OK"
CHANGESET_UNKNOWN = "UNKNOWN"

# Nombres que, en la raíz del repositorio, no pueden afectar a la compilación de
# ningún crate. No es una lista de excusas: es corta, y el planner además exige
# que ningún paquete ni script los cite para tratarlos como inertes.
METADATA_ONLY_NAMES = (
    "README.md",
    "LICENSE",
    "LICENSE.md",
    "NOTICE",
    "NOTICE.md",
    "COPYING",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "CODE_OF_CONDUCT.md",
    "CODEOWNERS",
    "AGENTS.md",
    "CLAUDE.md",
)


class GitError(RuntimeError):
    pass


def _run_git(repo: Path, *args: str, check: bool = True) -> tuple[int, bytes]:
    proc = subprocess.run(
        ["git", *args],
        cwd=str(repo),
        capture_output=True,
        check=False,
    )
    if check and proc.returncode != 0:
        raise GitError(
            f"git {' '.join(args)} falló con {proc.returncode}: "
            f"{proc.stderr.decode('utf-8', 'replace').strip()}"
        )
    return proc.returncode, proc.stdout


def _split_z(raw: bytes) -> list[str]:
    parts = raw.split(b"\x00")
    return [p.decode("utf-8", "surrogateescape") for p in parts if p]


@dataclass(frozen=True)
class ChangeSet:
    repo_root: Path
    base_ref: str | None
    merge_base: str | None
    head: str
    changed: tuple[str, ...] = ()
    untracked: tuple[str, ...] = ()
    deleted: tuple[str, ...] = ()
    renamed: tuple[tuple[str, str], ...] = ()
    dirty: bool = False
    status: str = CHANGESET_OK
    fallback_reason: str | None = None
    _content_cache: dict[str, str] = field(default_factory=dict, repr=False, compare=False)

    @property
    def is_unknown(self) -> bool:
        return self.status == CHANGESET_UNKNOWN

    def all_paths(self) -> tuple[str, ...]:
        """Todas las rutas tocadas, ordenadas y sin duplicados.

        Incluye las borradas: que un fichero de test deje de existir es un cambio
        de la fuente, y dejarlo fuera de la huella haría que la reutilización no
        notara su desaparición.
        """
        seen = set(self.changed) | set(self.untracked) | set(self.deleted)
        for old, new in self.renamed:
            seen.add(old)
            seen.add(new)
        return tuple(sorted(seen))

    def touches_only_metadata(self) -> bool:
        """¿Todo lo tocado es documentación de raíz verificable como inocua?

        `docs/` **no** entra aquí: TH-S01 avisa de que puede alimentar análisis y
        fixtures, así que se trata como desconocido y cae al fallback conservador.
        """
        paths = self.all_paths()
        if not paths:
            return True
        for path in paths:
            name = Path(path).name
            if name not in METADATA_ONLY_NAMES:
                return False
            if "/" in path:
                return False
        return True

    def digest_for(self, paths: tuple[str, ...]) -> str:
        """Digest del contenido efectivo de esas rutas relativas al repo.

        "Contenido efectivo" = lo que hay en el working tree, no lo que hay en
        HEAD: si el árbol está sucio, lo que se va a compilar es el árbol, y una
        huella que ignore eso reutilizaría evidencia de otro código.
        """
        entries: list[str] = []
        for rel in sorted(set(paths)):
            entries.append(f"{rel}\0{self.file_digest(rel)}")
        return hashlib.sha256("\n".join(entries).encode("utf-8", "surrogateescape")).hexdigest()

    def file_digest(self, rel: str) -> str:
        cached = self._content_cache.get(rel)
        if cached is not None:
            return cached
        target = self.repo_root / rel
        if not target.exists():
            cached = "DELETED"
        elif target.is_dir():
            cached = "DIRECTORY"
        elif target.is_symlink():
            cached = "SYMLINK:" + hashlib.sha256(
                os.readlink(target).encode("utf-8", "surrogateescape")
            ).hexdigest()
        else:
            digest = hashlib.sha256()
            with target.open("rb") as handle:
                for chunk in iter(lambda: handle.read(1 << 20), b""):
                    digest.update(chunk)
            cached = digest.hexdigest()
        if isinstance(self._content_cache, dict):
            self._content_cache[rel] = cached
        return cached


def repo_root(path: Path) -> Path:
    _, out = _run_git(Path(path), "rev-parse", "--show-toplevel")
    return Path(out.decode("utf-8", "surrogateescape").strip())


def current_head(repo: Path) -> str:
    _, out = _run_git(repo, "rev-parse", "HEAD")
    return out.decode("utf-8").strip()


def _parse_name_status_z(raw: bytes) -> tuple[list[str], list[str], list[tuple[str, str]]]:
    tokens = _split_z(raw)
    changed: list[str] = []
    deleted: list[str] = []
    renamed: list[tuple[str, str]] = []
    index = 0
    while index < len(tokens):
        status = tokens[index]
        index += 1
        if status.startswith("R") or status.startswith("C"):
            if index + 1 >= len(tokens):
                break
            old, new = tokens[index], tokens[index + 1]
            index += 2
            renamed.append((old, new))
            changed.append(new)
        else:
            if index >= len(tokens):
                break
            path = tokens[index]
            index += 1
            if status.startswith("D"):
                deleted.append(path)
            changed.append(path)
    return changed, deleted, renamed


def read_changeset(repo: Path, base_ref: str | None) -> ChangeSet:
    """Lee el ChangeSet de un checkout, con base explícita o sin ella."""
    root = repo_root(repo)
    head = current_head(root)

    if not base_ref:
        return ChangeSet(
            repo_root=root,
            base_ref=None,
            merge_base=None,
            head=head,
            status=CHANGESET_UNKNOWN,
            fallback_reason=(
                "no se pidió base: sin base confiable no se puede saber qué cambió, "
                "y un plan vacío se leería como 'nada que probar'"
            ),
        )

    code, _ = _run_git(root, "rev-parse", "--verify", "--quiet", f"{base_ref}^{{commit}}", check=False)
    if code != 0:
        return ChangeSet(
            repo_root=root,
            base_ref=base_ref,
            merge_base=None,
            head=head,
            status=CHANGESET_UNKNOWN,
            fallback_reason=f"la base '{base_ref}' no resuelve a un commit en este checkout",
        )

    code, out = _run_git(root, "merge-base", base_ref, head, check=False)
    if code != 0 or not out.strip():
        return ChangeSet(
            repo_root=root,
            base_ref=base_ref,
            merge_base=None,
            head=head,
            status=CHANGESET_UNKNOWN,
            fallback_reason=(
                f"'{base_ref}' y HEAD no comparten ancestro (merge-base vacío): "
                "no hay diff confiable contra el que planificar"
            ),
        )
    merge_base = out.decode("utf-8").strip()

    changed: list[str] = []
    deleted: list[str] = []
    renamed: list[tuple[str, str]] = []

    if merge_base != head:
        _, raw = _run_git(root, "diff", "--name-status", "-z", merge_base, head)
        c, d, r = _parse_name_status_z(raw)
        changed.extend(c)
        deleted.extend(d)
        renamed.extend(r)
        # Un fichero añadido y borrado dentro de la rama no aparece en el diff
        # neto, pero su cobertura desapareció igual. Se busca en la historia.
        _, raw = _run_git(
            root,
            "log",
            "--diff-filter=D",
            "--name-only",
            "-z",
            "--format=",
            f"{merge_base}..{head}",
        )
        deleted.extend(_split_z(raw))

    # Working tree y staged juntos: `git diff HEAD` es exactamente eso. Se cuenta
    # aparte porque `dirty` describe el árbol de trabajo, no la rama: el diff
    # contra la base incluye commits ya hechos y esos no son "suciedad".
    uncommitted: list[str] = []
    uncommitted_deleted: list[str] = []
    uncommitted_renamed: list[tuple[str, str]] = []
    _, raw = _run_git(root, "diff", "--name-status", "-z", "HEAD")
    c, d, r = _parse_name_status_z(raw)
    uncommitted.extend(c)
    uncommitted_deleted.extend(d)
    uncommitted_renamed.extend(r)
    changed.extend(uncommitted)
    deleted.extend(uncommitted_deleted)
    renamed.extend(uncommitted_renamed)

    _, raw = _run_git(root, "ls-files", "-z", "--others", "--exclude-standard")
    untracked = _split_z(raw)

    dirty = bool(uncommitted or uncommitted_deleted or uncommitted_renamed or untracked)

    return ChangeSet(
        repo_root=root,
        base_ref=base_ref,
        merge_base=merge_base,
        head=head,
        changed=tuple(sorted(set(changed))),
        untracked=tuple(sorted(untracked)),
        deleted=tuple(sorted(set(deleted))),
        renamed=tuple(sorted(renamed)),
        dirty=dirty,
    )


def worktree_id(repo: Path, head: str) -> str:
    """Identidad estable del checkout, para distinguir worktrees en los recibos."""
    digest = hashlib.sha256()
    digest.update(str(repo).encode("utf-8", "surrogateescape"))
    digest.update(b"\0")
    digest.update(head.encode("utf-8"))
    return digest.hexdigest()[:16]


def repo_id(repo: Path) -> str:
    """Identidad del repositorio, independiente de la ruta del checkout.

    Usa el remoto `origin` cuando existe y cae a la ruta real: los worktrees de un
    mismo repositorio comparten ledger, que es justo lo que se quiere para
    reutilizar evidencia entre ellos.
    """
    code, out = _run_git(repo, "config", "--get", "remote.origin.url", check=False)
    if code == 0 and out.strip():
        return out.decode("utf-8").strip()
    return str(repo.resolve())
