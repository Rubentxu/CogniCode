#!/usr/bin/env python3
"""La version que el repositorio dice tener y la version que de verdad tiene.

MEDIDO 2026-10-03. R1 ("Coherencia release publica") se enuncia como una
imposibilidad, y la redaccion es explicita:

    Debe ser imposible que:
        tag != published release
    o:
        README != install.sh latest
    o:
        manifest.version != binary --version

La mitad que esta vigilada es la de los documentos generados. Los dos
generadores —`generate_product_manifest.py` y `generate_support_matrix.py`—
declaran `--check`, la suite de contratos los ejecuta, y por eso el drift de
`product/*.json` salta en rojo. Eso ya esta; no se repite aqui.

Lo que no vigilaba nadie son las superficies que un generador no produce:

- el bloque "Versioning" del README, que declara la release vigente, lo que
  reportan los binarios y el tag;
- las instrucciones de pin del propio README (`COGNICODE_VERSION=v...`, `@v...`),
  que son lo que un usuario copia para fijar version;
- la cabecera mas reciente del CHANGELOG.

Las tres son afirmaciones verificables sobre el estado del repositorio, y
ninguna estaba atada a nada. Un README que siguiera anunciando `v0.98.1`
mientras el manifiesto dice `0.101.2` habria tenido la suite de contratos en
verde: los generadores comparan contra la version del workspace, y el README no
es un documento generado.

## Que NO comprueba este contrato

`tag != published release` y `install.sh latest` son afirmaciones sobre GitHub,
no sobre el arbol. Comprobarlas exige red y una release publicada, asi que
pertenecen al UAT de R1, que las mide despues de publicar. Un contrato que
midiera eso en cada commit seria un contrato que falla cuando no hay release, y
un gate que se apaga solo teaches a no mirar el绿灯.

Lo que si se comprueba aqui, y es la parte que hace posible lo de despues: que
el arbol sea internamente coherente, de modo que cuando la release se publique
no haya tres versiones distintas que puedan confundirse entre si.

## Mutaciones que este contrato tiene que detectar

- el README atrasado: bloque "Versioning" o pines en `0.98.1` con el
  workspace en `0.101.2`;
- el CHANGELOG con una cabecera mas reciente que la version del workspace;
- un README que nombre un tag que no existe y no es el corte en curso;
- un README sin bloque "Versioning": la invariante deja de enunciarse, y eso
  tampoco es un pasillo limpio.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
README = REPO_ROOT / "README.md"
CHANGELOG = REPO_ROOT / "CHANGELOG.md"
CARGO = REPO_ROOT / "Cargo.toml"

SEMVER = r"\d+\.\d+\.\d+"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def workspace_version(cargo_text: str | None = None) -> str:
    """La version del workspace: la unica que decide cual es la vigente.

    Se lee de `[workspace.package]`, no del primer `version =` del fichero: en
    un workspace con paquetes hijos, ese primero es el de un crate cualquiera.
    """
    text = cargo_text if cargo_text is not None else read(CARGO)
    assert "[workspace.package]" in text, (
        "Cargo.toml no declara [workspace.package]; no hay una version de la que "
        "hangear la coincidencia, y un contrato que no puede leer la verdad no "
        "puede comparar nada"
    )
    match = re.search(
        r'^version\s*=\s*"([^"]+)"', text.split("[workspace.package]", 1)[1], re.MULTILINE
    )
    assert match, "no se pudo leer la version de [workspace.package]"
    return match.group(1)


# ---------------------------------------------------------------------------
# Las tres superficies, como funciones puras
# ---------------------------------------------------------------------------
def release_block_problems(readme: str, current: str) -> list[str]:
    """El bloque "Versioning" del README."""
    block = re.search(
        r"^##\s+Versioning\s*$(.*?)(?=^##\s|\Z)", readme, re.MULTILINE | re.DOTALL
    )
    if block is None:
        return [
            "README.md no tiene seccion `## Versioning`: el README ya no declara "
            "cual es la release vigente, y una invariante que nadie enuncia no "
            "esta vigilada aunque el resto del repositorio sea coherente"
        ]
    body = block.group(1)
    problems: list[str] = []

    def bullet_version(label: str) -> str | None:
        """La version que afirma la viñeta, o None si no afirma ninguna.

        Se lee la viñeta entera y se busca el primer semver, en vez de intentar
       /tokenizar el markdown: `**Tag:**` va seguido de un enlace
        (`` [`origin/v1.2.3`](url) ``) y parsear esa sintaxis para extraer un
        numero es fragility que no aporta nada. Lo que la invariante afirma es
        que la version es la misma, no como esta compuesta la viñeta.
        """
        found = re.search(rf"\*\*{re.escape(label)}:\*\*(.*)", body)
        if found is None:
            return None
        version = re.search(rf"v?({SEMVER})", found.group(1))
        return version.group(1) if version else None

    release = bullet_version("Current release")
    if release is None:
        problems.append("el bloque Versioning no declara una version en `Current release`")
    elif release != current:
        problems.append(f"`Current release` dice {release} y el workspace esta en {current}")

    reported = bullet_version("Binaries report")
    if reported is None:
        problems.append("el bloque Versioning no declara una version en `Binaries report`")
    elif reported != current:
        problems.append(
            f"`Binaries report` dice {reported} y el workspace esta en {current}; "
            "el README afirma una version que los binarios no reportan"
        )

    tag = bullet_version("Tag")
    if tag is None:
        problems.append("el bloque Versioning no declara una version en `Tag`")
    elif tag != current:
        problems.append(
            f"`Tag` apunta a v{tag} y el workspace esta en {current}: el README "
            "enlaza a un tag que no es el de esta version"
        )
    return problems


def pin_problems(readme: str, current: str) -> list[str]:
    """Las instrucciones de pin: lo que un usuario copia para fijar version."""
    problems: list[str] = []
    pins = re.findall(rf"COGNICODE_VERSION=v({SEMVER})", readme)
    mise = re.findall(rf"`@v({SEMVER})`", readme)
    if not pins and not mise:
        return [
            "README.md no contiene ninguna instruccion de pin (`COGNICODE_VERSION=` "
            "o `@v...`). Sin pin, `install.sh` sirve 'la ultima release', que es "
            "justo la incoherencia que R1 viene a cerrar"
        ]
    for value in pins:
        if value != current:
            problems.append(
                f"README.md ensena `COGNICODE_VERSION=v{value}` y el workspace esta "
                f"en {current}: el usuario fijaria una version que no es la que se "
                "publica desde este tag"
            )
    for value in mise:
        if value != current:
            problems.append(
                f"README.md ensena el pin de mise `@v{value}` y el workspace esta "
                f"en {current}"
            )
    return problems


def changelog_problems(changelog: str, current: str) -> list[str]:
    """La cabecera mas reciente del CHANGELOG."""
    headings = re.findall(rf"^##\s+\[v({SEMVER})\]", changelog, re.MULTILINE)
    if not headings:
        return ["CHANGELOG.md no tiene ninguna cabecera `## [vX.Y.Z]`"]
    if headings[0] != current:
        return [
            f"la cabecera mas reciente del CHANGELOG es v{headings[0]} y el "
            f"workspace esta en {current}: la release que se prepara no es la "
            "primera que el CHANGELOG anuncia, y el changelog deja de ser la "
            "fuente de que se publico una version"
        ]
    return []


def named_tags_problems(readme: str, current: str, known: set[str]) -> list[str]:
    """Todo tag que nombra el README existe, o es el corte en curso.

    La excepcion importa y no es una concession: entre el commit que sube la
    version y el `git tag` hay una ventana real en la que el README ya nombra
    `vX.Y.Z` y ese tag todavia no existe. Fallar ahi obligaria a reordenar el
    proceso de release, no a arreglar el README.
    """
    if not known:
        # Sin tags visibles el clon no puede responder. Se degrada, y se dice:
        # es el mismo criterio que ya aplica test_oss_foundation.py.
        return []
    # `(?!\.\d)` y no `(?![.\d-])`: el lookahead solo debe rechazar un
    # COMPONENTE mas, no un punto cualquiera. Con `(?![.\d-])` una version al
    # final de una frase —`...y tambien v0.97.99.`— no casaba, porque el punto
    # de la frase cuenta como continuacion. Eso es un falso negativo en la regla
    # mas embarrassed posible: en prosa los puntos son la regla, no la excepcion.
    named = set(re.findall(rf"\bv({SEMVER})(?!\.\d)", readme))
    invented = sorted(v for v in named if v != current and v not in known)
    return [
        f"README.md nombra v{version}, que no es ningun tag de este repositorio"
        for version in invented
    ]


def known_tags() -> set[str]:
    """Tags visibles, locales o del remoto; vacio si el clon no puede responder."""
    local = subprocess.run(
        ["git", "tag", "--list"], cwd=REPO_ROOT, capture_output=True, text=True, check=False
    )
    if local.returncode == 0 and local.stdout.strip():
        return {t.lstrip("v") for t in local.stdout.split()}
    remote = subprocess.run(
        ["git", "ls-remote", "--tags", "origin"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if remote.returncode == 0 and remote.stdout.strip():
        return {
            line.rsplit("refs/tags/", 1)[-1].removesuffix("^{}").lstrip("v")
            for line in remote.stdout.splitlines()
            if "refs/tags/" in line
        }
    return set()


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------
def test_the_three_surfaces_agree_with_the_workspace() -> None:
    """La asercion de verdad, sobre los archivos de verdad."""
    current = workspace_version()
    readme, changelog = read(README), read(CHANGELOG)
    problems = (
        release_block_problems(readme, current)
        + pin_problems(readme, current)
        + changelog_problems(changelog, current)
    )
    assert not problems, "la release no es coherente consigo misma:\n  " + "\n  ".join(problems)


def test_the_readme_names_only_tags_that_exist() -> None:
    current = workspace_version()
    problems = named_tags_problems(read(README), current, known_tags())
    assert not problems, "README.md afirma tags que no existen:\n  " + "\n  ".join(problems)


def test_a_stale_readme_release_block_is_rejected() -> None:
    """La regresion que motivation este contrato: el README atraso.

    Con los generadores en verde —que comparan contra el workspace y no miran
    el README— un README en `0.98.1` pasaba la suite entera.
    """
    stale = (
        "## Versioning\n\n"
        "- **Current release:** `v0.98.1` (2026-05-01).\n"
        "- **Binaries report:** `0.98.1` (`cogh`).\n"
        "- **Tag:** [`origin/v0.98.1`](https://example.invalid/v0.98.1).\n"
    )
    problems = release_block_problems(stale, "0.101.2")
    assert len(problems) == 3, f"las tres afirmaciones del bloque estan desfasadas: {problems}"
    assert any("Current release" in p for p in problems), problems
    assert any("Binaries report" in p for p in problems), problems
    assert any("Tag" in p for p in problems), problems


def test_a_stale_install_pin_is_rejected() -> None:
    stale = "Instala con `COGNICODE_VERSION=v0.98.1`, o con mise y `@v0.98.1`.\n"
    problems = pin_problems(stale, "0.101.2")
    assert len(problems) == 2, f"los dos pines estan desfasados: {problems}"


def test_a_changelog_that_leads_with_another_version_is_rejected() -> None:
    changelog = "## [v0.98.1] — 2026-05-01\n\nviejo\n\n## [v0.97.0] — 2026-04-01\n"
    problems = changelog_problems(changelog, "0.101.2")
    assert problems, "el CHANGELOG anuncia otra version y tiene que fallar"


def test_a_missing_release_block_is_not_a_clean_bill() -> None:
    """Sin bloque que enuncies la invariante, no hay invariante que compares.

    Es el modo de fallo que este repositorio se niega a aceptar: una comprobacion
    que no encuentra nada que mirar y sale con 0.
    """
    problems = release_block_problems("# README\n\nSin seccion de version.\n", "0.101.2")
    assert problems, "un README sin bloque Versioning tiene que fallar"
    assert "Versioning" in problems[0], problems


def test_the_pending_cut_is_not_an_invented_tag() -> None:
    """La ventana entre subir la version y cortar el tag no es un defecto."""
    known = {"0.98.1", "0.99.2"}
    readme = "Instala `COGNICODE_VERSION=v0.101.2`; antes se servia v0.98.1.\n"
    assert named_tags_problems(readme, "0.101.2", known) == [], (
        "el corte en curso todavia no esta tagueado y eso no es un tag inventado"
    )
    problems = named_tags_problems(readme + "y tambien v0.97.3\n", "0.101.2", known)
    assert problems, "un tag que no existe y no es el corte en curso si es un defecto"
    assert "0.97.3" in problems[0], problems


def test_no_tag_visibility_degrades_instead_of_failing() -> None:
    """Un clon sin tags no puede responder; no puede tampoco mentir."""
    assert named_tags_problems("nombra v0.1.2 y v0.3.4\n", "0.101.2", set()) == []


def main() -> int:
    tests = [value for name, value in sorted(globals().items()) if name.startswith("test_")]
    failures: list[str] = []
    for test in tests:
        try:
            test()
        except Exception as error:  # noqa: BLE001 - un contrato reporta, no propaga
            failures.append(f"{test.__name__}: {error}")
    if failures:
        print(f"FAIL - {len(failures)} fallo(s):")
        for failure in failures:
            print(failure)
        return 1
    print(
        "PASS - el README, el CHANGELOG y el workspace declaran la misma version, "
        "y el README no nombra tags que no existan."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
