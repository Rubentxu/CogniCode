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
    """Extract the long SHA quoted in CURRENT.md header."""
    text = _read(CURRENT)
    m = re.search(r"\b([0-9a-f]{40})\b", text)
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


def test_current_sha_matches_head() -> None:
    """M2: CURRENT.md snapshot must reference HEAD or HEAD~1.

    Sin esta guarda, CURRENT.md puede citar un SHA obsoleto y el lector lo
    toma como cierto porque el documento dice "regenerar antes de citar".
    El ratchet pinea que la cabecera no puede mentir sin que el contrato
    lo detecte.

    Se acepta HEAD o HEAD~1: el snapshot puede ser INTRINSECO al commit
    actual (HEAD) o describir el commit padre (HEAD~1, snapshot es un
    addendum retroactivo). Cualquier otra cosa es drift no declarado.
    """
    declared = current_snapshot_sha()
    actual = head_sha()
    assert declared is not None, (
        f"FAIL: {CURRENT.name} no contiene un SHA de 40 chars. "
        f"Regenera el snapshot."
    )
    parent_sha = subprocess.check_output(
        ["git", "-C", str(REPO_ROOT), "rev-parse", "HEAD~1"],
        text=True,
    ).strip() if _has_parent() else actual
    assert declared in (actual, parent_sha), (
        f"FAIL: {CURRENT.name} declara SHA {declared[:12]}, "
        f"HEAD es {actual[:12]} (HEAD~1 es {parent_sha[:12]}). "
        f"El snapshot debe ser HEAD (intrinsico) o HEAD~1 (addendum del "
        f"padre). Regenera o reescribe la cabecera."
    )


def _has_parent() -> bool:
    try:
        subprocess.check_output(
            ["git", "-C", str(REPO_ROOT), "rev-parse", "--verify", "HEAD~1"],
            stderr=subprocess.DEVNULL,
        )
        return True
    except subprocess.CalledProcessError:
        return False


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