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
- la cabecera mas reciente del CHANGELOG;
- la tabla "Supported versions" de SECURITY.md, que declara en que rama vive la
  version vigente.

Las tres primeras son afirmaciones verificables sobre el estado del repositorio,
y ninguna estaba atada a nada. Un README que siguiera anunciando `v0.98.1`
mientras el manifiesto dice `0.101.2` habria tenido la suite de contratos en
verde: los generadores comparan contra la version del workspace, y el README no
es un documento generado.

La cuarta es la que sobrevivio dos cortes. MEDIDO 2026-10-04: la fila de
SECURITY.md decia `0.101.7 (current main, not yet released)` y `0.101.8
(current main, not yet released)`, mientras `origin/main` estaba en `0.101.0`.
El documento de seguridad afirmaba una rama que no lleva esa version, y ningun
contrato lo miraba: la suite entera verde. Una politica de soporte que nombra la
rama equivocada no es una politica de soporte.

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
  tampoco es un pasillo limpio;
- una fila de SECURITY.md que atribuya la version vigente a `main` cuando
  `origin/main` no lleva esa version.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
README = REPO_ROOT / "README.md"
CHANGELOG = REPO_ROOT / "CHANGELOG.md"
SECURITY = REPO_ROOT / "SECURITY.md"
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


def security_supported_problems(
    security: str, current: str, main_version: str | None, published: str | None = None
) -> list[str]:
    """La politica de soporte de SECURITY.md, y la rama que dice tener.

    Se comprueba una sola cosa, y es la que fallo dos veces: si la fila de la
    version vigente menciona `main`, `main` tiene que llevar esa version. La
    fila no afirma "el release"; afirma "donde vive y quien la parchea", y una
    fila que dice `current main` cuando main va tres versiones atras envia a
    quien reporte un fallo a una linea que no existe.

    `main_version` es None cuando el clon no puede resolver `origin/main`. No es
    el mismo criterio que `main` lleve o no la version: es que la respuesta no
    existe, y un contrato que no puede mirar no puede afirmar. Se degrada y lo
    dice, igual que hace `named_tags_problems` con la visibilidad de tags.

    `published` es la version mas alta que existe como tag remoto, y vigila una
    segunda afirmacion de la misma fila: `not yet released` sobre una version
    que ya se publico. La fila entonces no miente sobre la rama —la rama
    sigue siendo `integrate/v1015`— pero miente sobre el mundo, y eso manda a
    quien lea la politica a esperar un aviso que no va a llegar. Es el mismo
    defecto que el de la rama, una instruccion mas alla: **la superficie
    vigilaba la mitad de la afirmacion**.
    """
    block = re.search(
        r"^##\s+Supported versions\s*$(.*?)(?=^##\s|\Z)", security, re.MULTILINE | re.DOTALL
    )
    if block is None:
        return [
            "SECURITY.md no tiene seccion `## Supported versions`: el documento de "
            "seguridad ya no declara que versiones reciben soporte, y una politica "
            "que no se enuncia no se aplica"
        ]

    row: str | None = None
    for line in block.group(1).splitlines():
        found = re.match(rf"^\|\s*({SEMVER})\b(.*)$", line.strip())
        if found is not None and found.group(1) == current:
            row = found.group(2)
            break

    if row is None:
        return [
            f"SECURITY.md no declara la version vigente ({current}) en la tabla de "
            "versiones soportadas: la version que se corta no dice si recibe soporte"
        ]

    if main_version is not None and main_version != current and re.search(r"`main`", row):
        return [
            f"SECURITY.md describe {current} como `main`, pero origin/main esta en "
            f"{main_version}. La fila afirma una rama que no lleva esa version: quien "
            "lea la politica de soporte buscara un corte que no existe ahi."
        ]

    if (
        published is not None
        and published == current
        and re.search(r"not yet released", row, re.IGNORECASE)
    ):
        return [
            f"SECURITY.md dice que {current} no esta publicada, y el tag v{current} "
            "ya existe en el remoto. Una politica que manda esperar un aviso de "
            "seguridad que no va a llegar es peor que no tener politica: consume la "
            "confianza de quien la lee y la gasta en una mentira."
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


def main_version() -> str | None:
    """La version de [workspace.package] en `origin/main`, o None si no responde.

    Se lee del arbol remoto directamente, no del checkout: la pregunta que hace
    este contrato es "que lleva main", y el checkout puede estar tres commits
    por delante o por detras. Un `main` que no existe todavia en un clon nuevo
    no es un fallo: es una respuesta ausente, y se degrada.
    """
    shown = subprocess.run(
        ["git", "show", "origin/main:Cargo.toml"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if shown.returncode != 0 or not shown.stdout.strip():
        return None
    try:
        return workspace_version(shown.stdout)
    except AssertionError:
        return None


def _highest_remote_tag(ls_remote_stdout: str) -> str | None:
    """La version mas alta de un `git ls-remote --tags`, o None si no hay.

    Se ignoran las lineas de peel (`refs/tags/vX.Y.Z^{}`): describen el MISMO
    tag que la linea sin `^{}`, y contarlas como dos seria una segunda fuente
    de verdad sobre que existe. Un nucleo con un solo filtro, y una sola razon
    para el filtro.

    None cuando no hay ningun tag semver. Mismo criterio que el resto del
    fichero: una respuesta ausente se degrada y se dice.
    """
    versions = {
        m.group(1)
        for m in re.finditer(r"refs/tags/v(\d+\.\d+\.\d+)$", ls_remote_stdout, re.MULTILINE)
    }
    if not versions:
        return None
    return max(versions, key=lambda v: tuple(int(p) for p in v.split(".")))


def published_version() -> str | None:
    """La version mas alta publicada como tag remoto, o None si no responde.

    MEDIDO 2026-10-04: publicar `v0.101.9` dejo `SECURITY.md` diciendo
    `0.101.9 (cut from integrate/v1015, not yet released)` cuando la release
    existia. La fila era correcta en la rama y falsa en el mundo, y ningun
    contrato lo miraba: los cuatro que vigilan Release Truth vigilan la
    version, la rama, la etiqueta y el changelog. Ninguno vigila si el corte
    salio de su tarima.

    Se pregunta al remoto por `git ls-remote --tags`, que no necesita
    credenciales ni red autenticada. Da igual que la version mas alta del
    remoto no sea la que se esta cortando: la pregunta es "hay algo mas alto
    publicado que esta fila", y si lo hay, esta fila deberia hablar de el.

    None cuando el remoto no responde. Mismo criterio que `main_version` y que
    la visibilidad de tags: una respuesta ausente no es un veredicto.
    """
    out = subprocess.run(
        ["git", "ls-remote", "--tags", "origin"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    if out.returncode != 0 or not out.stdout.strip():
        return None
    return _highest_remote_tag(out.stdout)


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
def test_the_four_surfaces_agree_with_the_workspace() -> None:
    """La asercion de verdad, sobre los archivos de verdad."""
    current = workspace_version()
    readme, changelog, security = read(README), read(CHANGELOG), read(SECURITY)
    problems = (
        release_block_problems(readme, current)
        + pin_problems(readme, current)
        + changelog_problems(changelog, current)
        + security_supported_problems(security, current, main_version(), published_version())
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


# --- la cuarta superficie: donde dice vivir la version que se corta ----------
#
# MEDIDO 2026-10-04. SECURITY.md declaro `0.101.7 (current main, not yet
# released)` y despues `0.101.8 (current main, not yet released)`, con
# origin/main en `0.101.0`. Sobrevive a dos cortes porque ningun contrato leia
# el fichero: la superficie estaba vigilada, la rama no.


def _security_table(row: str) -> str:
    return f"## Supported versions\n\n| Version | Supported |\n|---|---|\n{row}\n"


def test_a_support_row_claiming_main_is_rejected_when_main_is_behind() -> None:
    """La regresion que motivo esta superficie: la rama equivocada.

    Con `main_version` por detras de la version vigente, una fila que dice
    `current main` esta mintiendo sobre donde vive la release. Es exactamente
    lo que hacia SECURITY.md desde `v0.101.7`.
    """
    table = _security_table("| 0.101.8 (current `main`, not yet released) | yes |")
    problems = security_supported_problems(table, "0.101.8", "0.101.0")
    assert problems, "una fila que atribuye la version a main, cuando main no la lleva, tiene que fallar"
    assert "origin/main" in problems[0], problems


def test_a_support_row_naming_a_real_branch_is_accepted() -> None:
    """El caso bueno: la fila nombra la rama que de verdad lleva la version."""
    table = _security_table("| 0.101.8 (cut from `integrate/v1015`, not yet released) | yes |")
    assert security_supported_problems(table, "0.101.8", "0.101.0") == []


def test_a_support_row_is_accepted_when_main_carries_the_version() -> None:
    """`main` lleva la version: la fila es cierta y no hay nada que senalar."""
    table = _security_table("| 0.101.8 (current `main`, not yet released) | yes |")
    assert security_supported_problems(table, "0.101.8", "0.101.8") == []


def test_an_unresolvable_main_degrades_instead_of_failing() -> None:
    """Sin `origin/main` no hay respuesta, y sin respuesta no hay veredicto.

    Es el mismo criterio que la visibilidad de tags: un clon que no puede
    resolver la referencia se degrada y lo dice, en vez de aprobar por no haber
    mirado nada.
    """
    table = _security_table("| 0.101.8 (current `main`, not yet released) | yes |")
    assert security_supported_problems(table, "0.101.8", None) == []


# ---------------------------------------------------------------------------
# La segunda mitad de la misma fila: no solo donde vive, sino si salio
# ---------------------------------------------------------------------------
def test_a_support_row_saying_not_yet_released_is_rejected_once_it_is_published() -> None:
    """La fila que sobrevive a la rama y muere a la publicacion.

    MEDIDO 2026-10-04. Publicar `v0.101.9` no cambio ni una linea de
    `SECURITY.md`, que seguia diciendo `not yet released`. La fila era
    correcta sobre la rama —el corte segue viniendo de `integrate/v1015`— y
    falsa sobre el mundo. Los cuatro contratos que vigilan Release Truth
    vigilan version, rama, etiqueta y changelog; ninguno vigilaba si el corte
    habia salido de su tarima.

    Y la consecuencia no es cosmetica: una politica de soporte que dice
    `not yet released` sobre algo publicado manda a quien reporte una
    vulnerabilidad a esperar un aviso que no va a llegar. Es peor que no
    declarar nada, porque gasta la confianza de quien lee.
    """
    table = _security_table(
        "| 0.101.9 (cut from `integrate/v1015`, not yet released) | yes |"
    )
    problems = security_supported_problems(table, "0.101.9", "0.101.0", published="0.101.9")
    assert problems, "una fila que dice `not yet released` sobre un tag ya publicado tiene que fallar"
    assert "no esta publicada" in problems[0], problems


def test_a_published_version_may_name_the_branch_it_was_cut_from() -> None:
    """Lo que hay que decir AL publicar: la rama, sin la etiqueta temporal.

    La fila sigue nombrando `integrate/v1015` porque es de ahi de donde sale
    el corte, y eso no cambia por publicar. Lo que cambia es que ya no hay
    release pendiente: quitar `not yet released` y el contrato pasa.
    """
    table = _security_table("| 0.101.9 (cut from `integrate/v1015`) | yes |")
    assert security_supported_problems(table, "0.101.9", "0.101.0", published="0.101.9") == []


def test_a_pending_cut_may_still_say_it_is_not_released() -> None:
    """La afirmacion opuesta: mientras no haya tag remoto, la frase es cierta.

    Sin este caso, un contrato que prohibe `not yet released` estaria
    prohibiendo tambien la verdad, y la forma correcta de arreglar un ratchet
    asi no es borrarlo: es darle el otro lado.
    """
    table = _security_table(
        "| 0.102.0 (cut from `integrate/v1015`, not yet released) | yes |"
    )
    assert (
        security_supported_problems(table, "0.102.0", "0.101.0", published="0.101.9") == []
    ), "un corte que aun no existe como tag si puede decir que no esta publicado"


def test_an_unresolvable_remote_degrades_instead_of_failing() -> None:
    """Sin `origin` no se puede saber que hay publicado, y sin saber no se afirma.

    Mismo criterio que `main_version` y que la visibilidad de tags: una
    respuesta ausente se degrada y se dice. Un clon sin remoto no tiene por
    que aprobar la fila, pero tampoco tiene por que reprobarla.
    """
    table = _security_table(
        "| 0.101.9 (cut from `integrate/v1015`, not yet released) | yes |"
    )
    assert security_supported_problems(table, "0.101.9", "0.101.0", published=None) == []


def test_published_version_ignores_a_tag_that_only_exists_locally() -> None:
    """Un tag local sin remoto no cuenta como publicado.

    La distincion importa justo en esta cadena: `v0.101.9` se creo en local
    mucho antes de empujarse, precisamente para poder moverlo si la lane
    fallaba. Durante esa ventana el tag existia y no estaba publicado, y un
    contrato que lo tomara por publicado haria fallar la fila de `SECURITY.md`
    en el momento de cortar, que es cuando el corte todavia puede corregirse.
    """
    # Salida de `ls-remote` de un remoto sin v0.101.9: solo esta la linea sin
    # peel, porque `--tags` devuelve las dos y la que importa es la simple.
    out = "a1b2c3\trefs/tags/v0.101.8\nd4e5f6\trefs/tags/v0.101.8^{}\n"
    assert _highest_remote_tag(out) == "0.101.8", "el peel no es un tag distinto"


def test_published_version_reads_the_higher_remote_tag_not_the_lower() -> None:
    """El maximo, no el ultimo de la lista.

    `git ls-remote` no ordena. Una rama vieja puede haber dejado
    `v0.101.0` en el remoto mientras la linea publicada es `v0.98.1`, y el
    orden de salida no dice nada. La unica forma de saber cual manda es
    comparar componente a componente, que es lo que hace la clave.
    """
    out = "a1b2c3\trefs/tags/v0.98.1\nd4e5f6\trefs/tags/v0.101.0\n"
    assert _highest_remote_tag(out) == "0.101.0", "101 es mayor que 98 aunque vaya despues"


def test_published_version_degrades_on_an_empty_or_unparsable_remote() -> None:
    """Sin tags semver no hay respuesta, y sin respuesta no hay veredicto."""
    assert _highest_remote_tag("") is None
    assert _highest_remote_tag("a1b2c3\trefs/tags/release-candidate\n") is None


def test_a_missing_supported_versions_table_is_not_a_clean_bill() -> None:
    """Sin tabla, la politica de soporte deja de enunciarse."""
    problems = security_supported_problems("# SECURITY\n\nSin politica.\n", "0.101.8", "0.101.0")
    assert problems, "un SECURITY.md sin tabla de versiones soportadas tiene que fallar"
    assert "Supported versions" in problems[0], problems


def test_a_support_table_without_the_current_version_is_rejected() -> None:
    """Una tabla que no menciona la version que se corta no dice si la sostiene."""
    table = _security_table("| 0.98.1 (latest published release) | yes |")
    problems = security_supported_problems(table, "0.101.8", "0.101.0")
    assert problems, "una tabla sin la version vigente tiene que fallar"
    assert "0.101.8" in problems[0], problems


# --- la mitad que comparaba la version y no los bytes ------------------------
#
# MEDIDO 2026-10-03. `release-tag-coherence.sh` comparaba la version del
# workspace contra el nombre del tag y ahi terminaba. Con el tag `v0.101.6`
# publicado y el workspace todavia en 0.101.6, `release-tag-coherence.sh
# v0.101.6 HEAD` respondio OK con HEAD dos commits por delante del tag. La
# version cuadraba y los bytes no: exactamente el incidente que el gate dice
# existir para cerrar, con la mitad de la comparacion sin hacer.
#
# Estos contratos ejecutan el gate real contra un tag y un commit de este
# repositorio, porque un gate que solo se puede razonar no se puede vigilar.

COHERENCE = REPO_ROOT / "scripts" / "ci" / "release-tag-coherence.sh"


def coherence_rc(*args: str) -> tuple[int, str]:
    done = subprocess.run(
        ["bash", str(COHERENCE), *args],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        timeout=120,
    )
    return done.returncode, done.stdout + done.stderr


def _a_published_tag() -> str:
    """El tag mas reciente que este clon puede resolver a un commit."""
    listed = subprocess.run(
        ["git", "tag", "--list", "--sort=-creatordate"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )
    for name in listed.stdout.split():
        peeled = subprocess.run(
            ["git", "rev-parse", "--verify", "--quiet", f"{name}^{{commit}}"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        if peeled.returncode == 0 and peeled.stdout.strip():
            return name
    raise AssertionError("este clon no resuelve ningun tag, y el contrato no puede medirse")


def test_the_tag_commit_itself_passes_coherence() -> None:
    """El caso bueno, primero: el commit del tag tiene que pasar."""
    tag = _a_published_tag()
    rc, output = coherence_rc(tag, f"{tag}^{{commit}}")
    assert rc == 0, f"el commit del propio tag {tag} no pasa el gate:\n{output}"


def test_a_commit_beyond_the_tag_is_refused_even_with_the_right_version() -> None:
    """La brecha, medida contra el gate real y sin depender del estado del HEAD.

    Importa por que NO se prueba con HEAD. En el momento en que se corta el tag,
    HEAD ES el commit del tag, asi que un contrato que usara HEAD haria `return`
    y no comprobaria nada: pasaria en verde con el gate roto. Este usa un commit
    que se sabe distinto —el padre del tag— de modo que la asercion se ejecuta
    siempre, o no que HEAD haya avanzado.

    Y el orden importa: la identidad se comprueba ANTES que la version. Al pasar
    un commit cuya version es distinta, un gate que solo mirase la version
    fallaria igualmente, pero por el motivo equivocado. Por eso se exige el
    mensaje de identidad: sin el, esta comprobacion no distingue las dos
    mitades del gate.
    """
    tag = _a_published_tag()
    tagged = subprocess.run(
        ["git", "rev-parse", f"{tag}^{{commit}}"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    parent = subprocess.run(
        ["git", "rev-parse", f"{tagged}^"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    ).stdout.strip()
    if parent == tagged:
        raise AssertionError("el tag no tiene padre, y el caso no se puede construir")

    rc, output = coherence_rc(tag, parent)
    assert rc != 0, f"el gate acepto un commit que no es el de {tag}:\n{output}"
    assert "is not the commit of tag" in output, (
        "el fallo no es de identidad del commit, con lo que este contrato no "
        f"distingue la mitad que falta.\n{output}"
    )


def test_an_unresolvable_tag_is_a_failure_not_a_skip() -> None:
    """Un tag que no existe no es una respuesta ausente, es un fallo."""
    rc, output = coherence_rc("v0.0.0-nonexistent", "HEAD")
    assert rc == 1, f"un tag inexistente paso el gate:\n{output}"
    assert "cannot resolve tag" in output, output


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
