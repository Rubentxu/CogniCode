#!/usr/bin/env python3
"""El ROADMAP no puede quedarse atras de la rama activa sin que el contrato lo diga.

## El hueco

`docs/roadmap/ROADMAP.md` declara ser la unica autoridad de agenda
(`AGENTS.md` § "Como distinguir el contexto"). `docs/roadmap/CURRENT.md`
es snapshot puntual y el propio fichero dice "regenerar antes de citar".
Pero nada pineaba la coherencia entre:

  1. la version que declara `Cargo.toml [workspace.package].version`, y
  2. una entrada reciente (ROADMAP o JOURNAL) que cite esa misma version, y
  3. el SHA del snapshot en `CURRENT.md` que coincide con el HEAD real.

El resultado MEDIDO 2026-10-09: workspace `0.101.9`, ROADMAP documenta
hasta `v0.99.2`, 9 releases patch consumidos sin que el ROADMAP los
refleje, `CURRENT.md` declarando su propia obsolescencia en cabecera.
Eso es exactamente "dos fuentes de verdad", el anti-patron §6 del
ROADMAP. Un PR que toque la version puede arrastrar al workspace
lejos del ROADMAP sin que nada se entere.

## Lo que se anade

Cuatro mutaciones, todas vistas caer sobre ficheros reales:

  - la version del workspace no aparece en ROADMAP ni JOURNAL reciente -> 1 rojo
    (es un solo caso pineado por la guarda "O ROADMAP O JOURNAL":
    MEDIDO, el JOURNAL de 615K cita `0.101.9` 121 veces, asi que el
    camino real para que esto falle es bumpear el workspace a una
    version no documentada en absoluto).
  - el SHA del snapshot en CURRENT.md no es el HEAD real                -> 1 rojo
  - CURRENT.md lleva mas de 14 dias sin regenerarse                    -> 1 rojo
  - la declaracion de snapshot en CURRENT.md falta o esta mal formada  -> 1 rojo
  - (control, sin mutacion)                                            -> todo verde

La ventana de 14 dias es el contrato: el snapshot es puntual, no
eterno; tras dos semanas se considera stale aunque el SHA coincida.
Una discusion razonable es mover esa ventana (30 dias?), pero
cambiarla aqui exige re-pasar todas las mutaciones vistas caer y
documentar la razon en el commit.

El contrato NO decide si la version documentada es la correcta: eso
es decision del operador. Lo que decide es que la declaracion y el
codigo no se contradigan en silencio, que es la pieza que faltaba.

## Limites del contrato

Solo conoce los nombres de su tabla. Si la version cambia de nombre
(e.g. el operador pasa de `0.101.9` a `1.0.0-rc.1`), el contrato
seguira pineando "version X aparece en ROADMAP o JOURNAL" — no sabe
que X es la ultima. La autoridad de que X es la version correcta
sigue siendo humana. El contrato es una red minima: detecta la
divergencia silenciosa, no la corrige.
"""
from __future__ import annotations

import datetime as _dt
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
CARGO_TOML = REPO_ROOT / "Cargo.toml"
ROADMAP = REPO_ROOT / "docs" / "roadmap" / "ROADMAP.md"
CURRENT = REPO_ROOT / "docs" / "roadmap" / "CURRENT.md"
JOURNAL = REPO_ROOT / "docs" / "roadmap" / "JOURNAL.md"
FRESHNESS_DAYS = 14


def _read(path: Path) -> str:
    if not path.is_file():
        return ""
    return path.read_text(encoding="utf-8")


def workspace_version() -> str:
    """Parse `[workspace.package].version` from Cargo.toml."""
    text = _read(CARGO_TOML)
    m = re.search(r'^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"',
                  text, flags=re.MULTILINE | re.DOTALL)
    if not m:
        raise SystemExit(f"FAIL: no se encontro [workspace.package].version en {CARGO_TOML}")
    return m.group(1)


def head_sha() -> str:
    """Long SHA of HEAD."""
    return subprocess.check_output(
        ["git", "-C", str(REPO_ROOT), "rev-parse", "HEAD"],
        text=True,
    ).strip()


def _mentions(text: str, version: str) -> bool:
    """A document mentions a version if `vX.Y.Z` or `X.Y.Z` appears as a token."""
    escaped = re.escape(version)
    # accept both `v0.101.9` and bare `0.101.9`, but not substrings like `1.0`
    pat = re.compile(rf"(?:^|\W)v?{escaped}(?=\W|$)")
    return bool(pat.search(text))


def roadmap_or_journal_cites(version: str) -> tuple[bool, str]:
    """Returns (mentioned, source_label). Either ROADMAP or JOURNAL must cite."""
    rm = _read(ROADMAP)
    jn = _read(JOURNAL)
    if _mentions(rm, version):
        return True, "ROADMAP"
    if _mentions(jn, version):
        return True, "JOURNAL"
    return False, "none"


def current_snapshot_sha() -> str | None:
    """Extract the long SHA the CURRENT.md snapshot header declares.

    Anchored to the same `Snapshot**:` declaration that carries the date,
    not to the first 40-hex token in the file. CURRENT.md also quotes SHAs
    in its historical body — the previous snapshot, the "SHA funcional"
    line, per-release receipts — and reading the first match anywhere meant
    a stale SHA in that body could satisfy the ratchet while the header
    declared a different tree, or the reverse: a header update could be
    masked by an unrelated historical line. A ratchet that measures a
    different SHA than the one it claims to measure is not a ratchet.
    """
    text = _read(CURRENT)
    m = re.search(r"Snapshot\*\*:[^\n]*?\b([0-9a-f]{40})\b", text)
    return m.group(1) if m else None


def current_snapshot_date() -> _dt.date | None:
    """Extract the date quoted in CURRENT.md snapshot declaration (YYYY-MM-DD)."""
    text = _read(CURRENT)
    m = re.search(r"Snapshot\*\*:\s*(\d{4}-\d{2}-\d{2})", text)
    if not m:
        return None
    return _dt.date.fromisoformat(m.group(1))


def days_since(date: _dt.date) -> int:
    return (_dt.date.today() - date).days


# ---------- assertions (each is one mutation seen to fail) ----------

def test_workspace_version_exists() -> None:
    """Sanity: workspace version parses; without this nothing else is meaningful."""
    v = workspace_version()
    assert re.match(r"^\d+\.\d+\.\d+", v), f"version no es semver: {v!r}"


def test_roadmap_or_journal_cites_the_workspace_version() -> None:
    """M1: cargo.toml version must appear in ROADMAP.md or JOURNAL.md recent entries.

    Sin esta guarda, el operador puede bumpear `Cargo.toml` y olvidar el ROADMAP;
    el contrato pinea que no.
    """
    v = workspace_version()
    mentioned, source = roadmap_or_journal_cites(v)
    assert mentioned, (
        f"FAIL: workspace version {v!r} no aparece en {ROADMAP.name} "
        f"ni en {JOURNAL.name}. Anota el delta en uno antes de promover."
    )


SNAPSHOT_MAX_DRIFT = 5  # commits: a snapshot describing HEAD~5 is still honest


def test_current_sha_matches_head() -> None:
    """M2: CURRENT.md snapshot must reference a recent ancestor of HEAD.

    Sin esta guarda, CURRENT.md puede citar un SHA obsoleto y el lector lo
    toma como cierto porque el documento dice "regenerar antes de citar".
    El ratchet pinea que la cabecera no puede mentir sin que el contrato
    lo detecte.

    Se acepta cualquier ancestro dentro de SNAPSHOT_MAX_DRIFT commits **de
    la base donde esta historia se encontró con `main`**. La distancia se
    mide ahí y no contra HEAD: los commits propios de una rama son trabajo
    sin publicar, que ningun lector puede depender aun, y medirlos hacia
    atras hacia que el guard disparase en casi cualquier rama real — y la
    unica salida era citar la cabeza de la rama, que un squash-merge
    invalida enseguida. En `main` los dos refs coinciden y la ventana
    sigue mordiendo igual. Ver `_is_recent_ancestor`.
    """
    declared = current_snapshot_sha()
    actual = head_sha()
    assert declared is not None, (
        f"FAIL: {CURRENT.name} no contiene un SHA de 40 chars. "
        f"Regenera el snapshot."
    )
    base = _merge_base(actual, "main") or actual
    assert _is_recent_ancestor(declared, actual, SNAPSHOT_MAX_DRIFT, base), (
        f"FAIL: {CURRENT.name} declara SHA {declared[:12]}, "
        f"HEAD es {actual[:12]} y la base con main es {base[:12]}. El SHA "
        f"declarado debe ser alcanzable desde HEAD y estar a mas de "
        f"{SNAPSHOT_MAX_DRIFT} commits de esa base. Regenera el snapshot."
    )


def _merge_base(a: str, b: str) -> str | None:
    """Where the two histories last met, or None if they are unrelated."""
    out = subprocess.run(
        ["git", "-C", str(REPO_ROOT), "merge-base", a, b],
        capture_output=True, text=True,
    )
    return out.stdout.strip() or None


def _is_recent_ancestor(
    sha: str, ancestor: str, max_drift: int, distance_to: str | None = None
) -> bool:
    """True if `sha` is reachable from `ancestor` and near `distance_to`.

    Two independent questions, and the old code answered only the first
    badly:

    * **Is it on this history at all?** `rev-list --count A..B` alone
      counts the commits B has that A does not, which a SHA from a
      squash-merged side branch satisfies with 1 — so a snapshot could
      cite a tree that was never on this branch. Reachability is checked
      first, because a guard that cannot detect what its docstring names
      is not a guard.

    * **How far behind is it?** Measured to `distance_to`, which is
      where this line of history last met `main` — not to `ancestor`.
      On a feature branch the commits in between are unreleased work
      that no reader can depend on yet, so charging the snapshot for
      them made this fire on almost every real branch. And the only way
      to silence it was to cite the branch head, which a squash-merge
      then invalidates — the two halves of the old rule could not both
      be satisfied by any value once a branch passed five commits.

      On `main` the two refs coincide, so the window bites exactly as
      before: a `main` commit that nobody regenerated against is still
      a `main` commit nobody regenerated against.
    """
    if sha == ancestor:
        return True
    try:
        reachable = subprocess.run(
            ["git", "-C", str(REPO_ROOT), "merge-base", "--is-ancestor", sha, ancestor],
            capture_output=True, text=True,
        )
        if reachable.returncode != 0:
            return False
        out = subprocess.check_output(
            ["git", "-C", str(REPO_ROOT), "rev-list", "--count", f"{sha}..{distance_to or ancestor}"],
            text=True, stderr=subprocess.DEVNULL,
        ).strip()
        distance = int(out)
        return 0 <= distance <= max_drift
    except (subprocess.CalledProcessError, ValueError):
        return False


def test_distance_is_measured_to_the_merge_base_not_the_branch_head() -> None:
    """The window follows the base, not the tip — on any ref.

    Pinned with the repository's own root commit so it holds on a
    branch and on `main` alike: the root is reachable from every tip,
    and it is certainly more than `SNAPSHOT_MAX_DRIFT` commits back
    from one.
    """
    head = head_sha()
    root = subprocess.check_output(
        ["git", "-C", str(REPO_ROOT), "rev-list", "--max-parents=0", head],
        text=True,
    ).strip().splitlines()[-1]

    assert not _is_recent_ancestor(root, head, SNAPSHOT_MAX_DRIFT, head), (
        "measured against the tip, a commit far behind must be refused"
    )
    assert _is_recent_ancestor(root, head, SNAPSHOT_MAX_DRIFT, root), (
        "measured against a base at that commit, the distance is zero and it "
        "must be accepted — this is what lets a long feature branch keep a "
        "snapshot that names the last merge"
    )


def test_current_snapshot_is_fresh() -> None:
    """M3: CURRENT.md must be regenerated within FRESHNESS_DAYS days.

    El snapshot es puntual. Aunque el SHA coincida por casualidad, una
    declaracion de mas de 14 dias sin regenerar es opaca al estado real.
    """
    d = current_snapshot_date()
    assert d is not None, (
        f"FAIL: {CURRENT.name} no tiene cabecera 'Snapshot**: YYYY-MM-DD'. "
        f"Regenera."
    )
    age = days_since(d)
    assert age <= FRESHNESS_DAYS, (
        f"FAIL: {CURRENT.name} lleva {age} dias sin regenerarse "
        f"(snapshot {d.isoformat()}, hoy {_dt.date.today().isoformat()}). "
        f"Limite contractual: {FRESHNESS_DAYS} dias."
    )


def test_control_all_green() -> None:
    """Sanity: en estado nominal, los tres assertions anteriores pasan.

    Si esto falla en HEAD limpio, hay un bug en el contrato, no en el repo.
    """
    test_workspace_version_exists()
    test_roadmap_or_journal_cites_the_workspace_version()
    test_current_sha_matches_head()
    test_current_snapshot_is_fresh()


def main() -> int:
    failures: list[str] = []
    for fn in (
        test_workspace_version_exists,
        test_roadmap_or_journal_cites_the_workspace_version,
        test_current_sha_matches_head,
        test_distance_is_measured_to_the_merge_base_not_the_branch_head,
        test_current_snapshot_is_fresh,
        test_control_all_green,
    ):
        try:
            fn()
        except AssertionError as e:
            failures.append(f"{fn.__name__}: {e}")
        except Exception as e:
            failures.append(f"{fn.__name__}: {type(e).__name__}: {e}")

    if failures:
        print("ROADMAP version ratchet: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1

    print(
        f"ROADMAP version ratchet: OK "
        f"(workspace={workspace_version()}, head={head_sha()[:12]})"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())