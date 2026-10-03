#!/usr/bin/env python3
"""El nombre del componente se recorta con el valor de Kotlin, no con una variable de shell.

MEDIDO 2026-10-03. `v0.101.3` fue el primer candidato que llego a la stage
`package-$target` — los dos anteriores nunca llegaron: `v0.101.1` seParo a
mano, `v0.101.2` murio en el trap de limpieza del preflight. La mitad de
construccion y empaquetado de la lane nunca se habia ejecutado. Fallo aqui:

    package-x86_64-unknown-linux-gnu  ...
    packaged  for linux-x86-64
    Error: unknown component `cognicode-0.101.3-x86_64-unknown-linux-gnu.tar.gz`
    cp: no se puede efectuar `stat' sobre
        '/var/home/rubentxu/cargo-targets/x86_64-unknown-linux-gnu/release/cognicode-0.101.3-...tar.gz'
    tar (child): staging/payloads-linux-x86-64/dist/: Es un directorio
    ...
    staging/payloads-linux-x86-64/dist:
    total 8                                    <- vacio, y la stage paso

Y dos stages despues, `archive-standalone` reporto `cogh missing or not
executable from the archive` — un mensaje sobre el archivo cuando la causa
estaba tres stages antes, en el empaquetado.

## La causa

Una linea:

    component="${'$'}{filename%-${'$'}version-*}"

`${'$'}` emite un `$` literal para el shell, asi que `${'$'}version` llega a
bash como una variable de shell llamada `version` — que esta lane nunca
exporta. La linea de al lado, `--version "$version"`, si usa el valor de
Kotlin, y por eso funcionaba. La confusion entre las dos notaciones es
invisible al leer: las dos parecen decir "la version".

Medido, no supuesto — con la variable de shell vacia el patron `%-$version-*`
se convierte en `---*`, que no casa con nada:

    version=''       component=cognicode-0.101.3-x86_64-unknown-linux-gnu.tar.gz
    version='0.101.3' component=cognicode

`component` se quedaba con el nombre completo del archivo, `cp` buscaba un
binario llamado `...tar.gz` que no existe, y `name --component` recibia lo
mismo. Tres subcomandos fallando, ninguno examinado.

La misma notacion estaba mal en dos sitios mas: los bundles de skills se
nombraban `staging/cognicode-.tar.gz`, y el mensaje de la stage de SBOM decia
`the sbom- stage`.

## Por que additionally el silencio era el otro defecto

Con el `cp` fallando, la stage de empaquetado no fallo: `cp` y `tar` no se
comprobaban, el bucle following no era Nada que comprobar porque `planned`
no existia, y `ls -la` de un directorio vacio sale con 0. La stage informo
exito sin haber producido un solo payload. El defecto no fallo donde estaba;
fallo dos stages despues, en un mensaje que senalaba el sitio equivocado.

Por eso el arreglo anade la guarda que la release lane ya tenia en su propio
lugar ("An empty candidate is not a candidate") pero en el punto donde ocurre:
si `plan` no devuelve nada, o si lo planificado y lo producido no coinciden,
la stage falla.

## Que se comprueba

Este contrato **ejecuta el cuerpo real de la stage**, no una copia. Extrae el
cuerpo del `sh(...)` de `package-$target` del fichero del pipeline, resuelve
las dos notaciones —`${'$'}x` es shell, `$x` es Kotlin— y lo corre contra un
`cognicode-release` de mentira. Si el cuerpo cambia, cambia lo que se ejecuta.

- Con un `plan` de verdad, produce un archivo por componente y sale 0.
- Con `plan` vacio, **falla**. Sin esta guarda la stage pasaba.
- Con un binario ausente, **falla** en vez de seguir en silencio.
- Estructural: en ningun `*.pipeline.kts` puede aparecer `${'$'}nombre` cuando
  `nombre` es un `val` o una variable de bucle de Kotlin. Es la clase entera,
  y habria atrapado los tres sitios de una vez.
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CANDIDATE = REPO_ROOT / "release-candidate.pipeline.kts"

VERSION = "0.101.3"
TARGET = "x86_64-unknown-linux-gnu"
PLATFORM = "linux-x86-64"
COMPONENTS = ("cogh", "cognicode", "cognicode-mcp")

# `cognicode-release` de mentira. Reproduce el contrato real: `plan` imprime
# un archivo por componente y `name` devuelve el nombre canonico. Con
# PLAN_EMPTY=1 responde vacio, que es el caso que la stage no detectaba.
FAKE_RELEASE = """#!/usr/bin/env bash
set -euo pipefail
mode="${{1:-}}"
component=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --component) component="$2"; shift 2 ;;
        *) shift ;;
    esac
done
version="{version}"
token="{target}"
case "$mode" in
    plan)
        if [ "${{PLAN_EMPTY:-0}}" = "1" ]; then exit 0; fi
        for c in {components}; do
            printf '%s-%s-%s.tar.gz\\n' "$c" "$version" "$token"
        done
        ;;
    name)
        [ -n "$component" ] || {{ echo "Error: no component given" >&2; exit 2; }}
        case "$component" in
            cogh|cognicode|cognicode-mcp)
                printf '%s-%s-%s.tar.gz\\n' "$component" "$version" "$token" ;;
            *)
                echo "Error: unknown component \\`$component\\`" >&2
                exit 1 ;;
        esac
        ;;
    *) echo "unknown subcommand $mode" >&2; exit 2 ;;
esac
"""


def kotlin_scoped_names(text: str) -> set[str]:
    """Nombres que Kotlin ya tiene resueltos en el momento de renderizar.

    Un `val`, o una variable de bucle de Kotlin. Escribir cualquiera de ellos
    como `${'$'}nombre` — es decir, como variable de shell — es siempre un
    error: el valor ya existe en el lado de Kotlin.
    """
    names = set(re.findall(r"^\s*val\s+(\w+)\s*[:=]", text, re.M))
    names |= set(re.findall(r"\bfor\s*\(\s*(\w+)\s+in\b", text))
    names |= set(re.findall(r"\bwhile\s*\(\s*(\w+)\s+in\b", text))
    return names


def strip_comments(text: str) -> str:
    """Quita las lineas que son solo comentario: shell (`#`) y Kotlin (`//`).

    Un comentario no se ejecuta, asi que no puede ser la confusion que se
    busca. Sin esta excepcion, el comentario que explica el defecto —que
    necesita nombrar la forma equivocada para ser util— haria fallar el
    contrato que existe precisamente por documentar ese defecto. Solo se
    descartan lineas completas: un comentario al final de una linea de codigo
    si se cuenta, porque ahi no se puede distinguir de una cadena.
    """
    kept = []
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("#") or stripped.startswith("//"):
            continue
        kept.append(line)
    return "\n".join(kept)


def extract_stage_body(text: str, stage_name: str) -> str:
    """Saca el cuerpo de shell del `sh(...)` de una stage, sin editarlo.

    El `sh(` se busca y luego el raw string que abre, porque no todas las stages
    tienen la misma forma: `package-$target` concatena `releasePaths` con un raw
    string, y `toolchain-for-$target` pasa el raw string directamente. Buscando
    la forma exacta de la primera, la busqueda caia en la stage equivocada —el
    siguiente `sh(releasePaths` ya era el de empaquetado— y el contrato ejecutaba
    el cuerpo de otra stage y informaba sobre lo que esa hacia.
    """
    marker = f'stage("{stage_name}") {{'
    start = text.index(marker)
    sh_at = text.index("sh(", start)
    open_quote = text.index('"""', sh_at) + len('"""')
    body_end = text.index('""".trimIndent())', open_quote)
    return text[open_quote:body_end]


# `cognicode-release` de mentira para la stage de skills. GH_SKILLS_MODE
# reproduce las tres respuestas que la stage tiene que distinguir:
#   ok      -> imprime los bundles publicados
#   empty   -> imprime nada, con salida 0
#   error   -> sale 2 sin imprimir, como hacia el binario real cuando el
#              subcommand no existia
FAKE_SKILLS = """#!/usr/bin/env bash
set -euo pipefail
case "${GH_SKILLS_MODE:-ok}" in
    ok) printf 'cognicode\\ncognicode-mcp\\n' ;;
    empty) exit 0 ;;
    error) echo "error: unrecognized subcommand 'skills'" >&2; exit 2 ;;
    *) echo "unknown mode" >&2; exit 3 ;;
esac
"""


def run_skill_stage(
    mode: str, *, stale_root: bool = False, keep_lane_dir: bool = False
) -> subprocess.CompletedProcess:
    """Ejecuta el cuerpo real de `skill-bundles` con un release de mentira.

    `stale_root` siembra en la raiz de `staging/` lo que deja una corrida
    anterior: el aplanado copia ahi sus payloads y sus SBOM, y la copia se
    queda. `keep_lane_dir` siembra un directorio `payloads-*`, que en una lane
    real acaba de producir `package-$target` en ESTA corrida.
    """
    text = CANDIDATE.read_text(encoding="utf-8")
    body = render(
        extract_stage_body(text, "skill-bundles"), VERSION, TARGET, PLATFORM
    )

    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        target_dir = root / "cargo-targets"
        bindir = target_dir / "release"
        bindir.mkdir(parents=True)
        tool = bindir / "cognicode-release"
        tool.write_text(FAKE_SKILLS, encoding="utf-8")
        tool.chmod(0o755)

        for bundle in ("cognicode", "cognicode-mcp"):
            skill = root / "skills" / bundle
            skill.mkdir(parents=True)
            (skill / "manifest.yaml").write_text("name: " + bundle + "\n", encoding="utf-8")
        (root / "staging").mkdir()
        if stale_root:
            # Lo que deja una corrida anterior en la raiz de staging: el
            # aplanado copia ahi los payloads y los SBOM, y la copia se queda.
            (root / "staging" / "cog-x86_64-unknown-linux-gnu.cdx.json").write_bytes(b"{}\n")
            (root / "staging" / "cogh-0.99.99-x86_64-unknown-linux-gnu.tar.gz").write_bytes(b"x")
        if keep_lane_dir:
            # `package-$target` deja el directorio de la lane, y lo ha producido
            # ESTA corrida unas stages antes: limpiarlo seria tirar el build.
            lane_dir = root / "staging" / f"payloads-{PLATFORM}" / "dist"
            lane_dir.mkdir(parents=True)
            (lane_dir / "cogh-0.101.3-x86_64-unknown-linux-gnu.tar.gz").write_bytes(b"this-run")

        script = root / "skill-stage.sh"
        script.write_text(
            f"#!/usr/bin/env bash\ncd {root}\n"
            f'TARGET_DIR="{target_dir}"\n' + body,
            encoding="utf-8",
        )
        script.chmod(0o755)

        result = subprocess.run(
            [str(script)],
            capture_output=True,
            text=True,
            timeout=120,
            env={
                "PATH": "/usr/bin:/bin",
                "HOME": str(root),
                "TMPDIR": str(root),
                "GH_SKILLS_MODE": mode,
            },
        )
        result.stdout = result.stdout.replace(str(root), "<tmp>")
        result.stderr = result.stderr.replace(str(root), "<tmp>")
        result.staged = sorted(
            p.name for p in (root / "staging").glob("*.tar.gz")
        )
        result.stale_left = sorted(
            p.name
            for p in (root / "staging").iterdir()
            if p.is_file() and p.suffix == ".json"
        )
        result.lane_dir_kept = (
            root / "staging" / f"payloads-{PLATFORM}" / "dist"
        ).is_dir()
        return result


def render(
    body: str,
    version: str,
    target: str,
    platform: str,
    repo_root: str = "/",
) -> str:
    """Convierte el cuerpo de la stage en el shell que PipelineK ejecutaria.

    EL ORDEN ES LA PRUEBA, Y CAMBIARLO ROMPE EL CONTRATO.

    Primero se sustituyen los valores de Kotlin, despues se convierte
    `${'$'}x` en `$x`. Invertido, el renderizador arregla el defecto que deberia
    detectar: al pasar `${'$'}` a `$` las dos notaciones se vuelven la misma
    cadena, y la sustitucion posterior de `$version` pisa tambien la que el
    pipeline escribio deliberadamente como variable de shell. El pipeline roto
    se ejecutaba como el arreglado, y `test_packages_every_planned_component`
    pasaba en los dos.

    En el orden correcto no hay colision: `${'$'}version` no contiene la
    subcadena `$version` —la preceden los signos de dollar y llave— asi que
    sobrevive intacta y llega a bash como la variable no definida que es.

    `repoRoot` y `cd` tambien son `val` de Kotlin, y no son la misma cosa:
    `val repoRoot` es la ruta y `val cd` es el fragmento `cd "$repoRoot"`. Una
    stage que necesite el path de un repo tiene que usar el primero; si el
    renderizador no conoce los dos, el cuerpo se ejecuta contra rutas que la
    lane nunca ve —`cd /ruta || exit 1/.cargo/config.toml` no es un camino que
    exista— y el contrato pasa o falla por una razon que no es la que dice.
    """
    out = body.replace("${platformOf(target)}", platform)
    out = out.replace("$version", version)
    out = out.replace("$target", target)
    out = out.replace("$repoRoot", repo_root)
    out = out.replace("$cd", f'cd "{repo_root}"')
    out = re.sub(r"\$\{'\$'\}", "$", out)
    return out


def run_package_stage(
    *,
    plan_empty: bool = False,
    binaries: tuple[str, ...] = COMPONENTS,
    stale: tuple[str, ...] = (),
) -> subprocess.CompletedProcess:
    """Ejecuta el cuerpo real de `package-$target` con un release de mentira.

    `stale` son archivos que ya estan en `staging/payloads-<platform>/dist/`
    antes de que corra la stage: lo que una lane anterior dejo en un worktree
    que se reutiliza. MEDIDO 2026-10-03, lane `v0.101.5`.
    """
    text = CANDIDATE.read_text(encoding="utf-8")
    body = render(
        extract_stage_body(text, "package-$target"), VERSION, TARGET, PLATFORM
    )

    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        target_dir = root / "cargo-targets"
        bindir = target_dir / "release"
        bindir.mkdir(parents=True)
        (target_dir / TARGET / "release").mkdir(parents=True)

        tool = bindir / "cognicode-release"
        tool.write_text(
            FAKE_RELEASE.format(
                version=VERSION, target=TARGET, components=" ".join(COMPONENTS)
            ),
            encoding="utf-8",
        )
        tool.chmod(0o755)

        for component in binaries:
            binary = target_dir / TARGET / "release" / component
            binary.write_text(f"#!/bin/sh\necho {component}\n", encoding="utf-8")
            binary.chmod(0o755)
            sbom = root / "crates" / f"{component}-{TARGET}.cdx.json"
            sbom.parent.mkdir(parents=True, exist_ok=True)
            sbom.write_text("{}\n", encoding="utf-8")

        staging = root / "staging"
        lane = staging / f"payloads-{PLATFORM}"
        (lane / "dist").mkdir(parents=True)
        for name in stale:
            (lane / "dist" / name).write_bytes(b"payload de otra lane")
        script = root / "package-stage.sh"
        script.write_text(
            f"#!/usr/bin/env bash\n"
            f"cd {root}\n"
            f'TARGET_DIR="{target_dir}"\n' + body,
            encoding="utf-8",
        )
        script.chmod(0o755)

        env = {
            "PATH": "/usr/bin:/bin",
            "HOME": str(root),
            "TMPDIR": str(root),
            "PLAN_EMPTY": "1" if plan_empty else "0",
        }
        result = subprocess.run(
            [str(script)], capture_output=True, text=True, timeout=120, env=env
        )
        result.stdout = result.stdout.replace(str(root), "<tmp>")
        result.stderr = result.stderr.replace(str(root), "<tmp>")
        result.produced = sorted(
            p.name
            for p in (root / "staging" / f"payloads-{PLATFORM}" / "dist").glob("*")
        )
        return result
# --- Tests ------------------------------------------------------------------


def test_the_gate_file_exists() -> None:
    assert CANDIDATE.is_file(), f"no existe {CANDIDATE}"


def test_packages_every_planned_component() -> None:
    """El camino que la stage recorria sin empaquetar nada."""
    done = run_package_stage()
    assert done.returncode == 0, (
        f"la stage debe empaquetar y salir 0, y salio {done.returncode}.\n"
        f"stdout:\n{done.stdout}\nstderr:\n{done.stderr}"
    )
    expected = sorted(f"{c}-{VERSION}-{TARGET}.tar.gz" for c in COMPONENTS)
    assert done.produced == expected, (
        f"un archivo por componente.\nobtenido: {done.produced}\n"
        f"stdout:\n{done.stdout}"
    )


def test_the_lane_does_not_inherit_a_previous_candidate_payloads() -> None:
    """Un directorio de salida de la lane se crea; no se hereda.

    MEDIDO 2026-10-03, lane `v0.101.5`, que murio aqui:

        package-x86_64-unknown-linux-gnu
        packaged cogh-0.101.5-x86_64-unknown-linux-gnu.tar.gz
        packaged cognicode-0.101.5-x86_64-unknown-linux-gnu.tar.gz
        packaged cognicode-mcp-0.101.5-x86_64-unknown-linux-gnu.tar.gz
        FAIL: planned 3 artifacts for linux-x86-64, produced 6.

    Los otros tres eran de `0.101.4`. El worktree que construye el candidato es
    de larga vida, y la stage creaba su directorio de lane con `mkdir -p` sin
    limpiarlo nunca, asi que la salida de la lane anterior seguia ahi.

    `mkdir -p` sobre un directorio que ya existe no es un no-op innocuo: es
    exactamente el mecanismo por el que un directorio de build hereda estado
    entre ejecuciones. La stage que escribe en el directorio es su duena, asi
    que es la que tiene que empezar desde uno nuevo — no un script de limpieza
    nuevo al que recourse cuando esto vuelva a pasar.
    """
    done = run_package_stage(stale=(f"cognicode-0.101.2-{TARGET}.tar.gz",))
    expected = sorted(f"{c}-{VERSION}-{TARGET}.tar.gz" for c in COMPONENTS)
    assert done.produced == expected, (
        f"la lane empaqueto encima de lo que habia encontrado.\n"
        f"obtenido: {done.produced}\n"
        f"esperado: {expected}\n"
        f"stdout:\n{done.stdout}\nstderr:\n{done.stderr}\n"
        f"  Un payload de otra version dentro de `dist/` no es ruido: el "
        f"ensamblador elige por nombre de componente, asi que cualquier "
        f"candidato que se arme sobre este directorio puede acabar "
        f"publicando los binarios de la release anterior."
    )


def test_the_staging_root_does_not_carry_the_previous_run() -> None:
    """MEDIDO 2026-10-03, en la lane de v0.101.5.

    La corrida anterior si llego a `generate`, y el aplanado copia los payloads
    y los SBOM de `staging/payloads-<plataforma>/` a la raiz de `staging/`. La
    copia se queda. En la corrida siguiente, el propio aplanado rechaza su
    salida de antes:

        ::error::unexpected file at staging root:
        cogh-aarch64-unknown-linux-gnu.cdx.json

    y la lane cae en `payloads`, dos stages despues de haber exitado bien.
    32fb1ff6 habia limpiado el directorio de la lane; lo que quedaba era la
    raiz. El aplanado tiene razon en rechazar un fichero que no reconoce —es un
    orphan de otra corrida, no parte de este candidato— asi que el arreglo no es
    relajar esa guarda sino no dejar que exista el fichero.
    """
    done = run_skill_stage("ok", stale_root=True)
    assert done.returncode == 0, done.stdout + done.stderr
    assert done.stale_left == [], (
        f"la raiz de staging arrastra ficheros de la corrida anterior: "
        f"{done.stale_left}"
    )
    expected = sorted(f"{b}-{VERSION}.tar.gz" for b in ("cognicode", "cognicode-mcp"))
    assert done.staged == expected, f"los bundles siguen prepping:\n{done.staged}"


def test_cleaning_the_root_does_not_throw_away_this_runs_build() -> None:
    """El otro lado del mismo arreglo, que es donde puede hacer dano.

    `package-$target` corre unas stages ANTES que `skill-bundles` y deja su
    directorio en `staging/payloads-<plataforma>/`. Limpiar la raiz para quitar
    un residuo de una corrida vieja no puede llevarse por delante lo que esta
    corrida acaba de construir: eso seria arreglar un arbol sucio tirando el
    trabajo del build.
    """
    done = run_skill_stage("ok", stale_root=True, keep_lane_dir=True)
    assert done.returncode == 0, done.stdout + done.stderr
    assert done.lane_dir_kept, (
        "el directorio payloads-* lo ha producido package-$target en ESTA "
        "corrida; la limpieza de la raiz no puede borrarlo"
    )


def test_an_empty_plan_fails_instead_of_succeeding_silently() -> None:
    """`plan` sin respuesta es una stage que no ve el producto, no un release vacio.

    Este es el caso que hizo que `v0.101.3` llegara a `archive-standalone` con un
    `dist/` vacio: la stage de empaquetado salia 0 sin haber producido nada.
    """
    done = run_package_stage(plan_empty=True)
    assert done.returncode != 0, (
        f"un plan vacio debe fallar; antes salia {done.returncode} produciendo "
        f"{done.produced}.\nstdout:\n{done.stdout}"
    )
    assert "empty candidate is not a candidate" in done.stdout.lower(), (
        f"el motivo deberia quedar escrito.\nstdout:\n{done.stdout}"
    )
    assert done.produced == [], f"no debe producir archivos.\n{done.produced}"


def test_a_missing_binary_fails_loudly() -> None:
    """Un `cp` que falla no puede pasar desapercibido dentro de un bucle."""
    done = run_package_stage(binaries=("cogh", "cognicode"))
    assert done.returncode != 0, (
        f"un binario ausente debe fallar la stage, y salio {done.returncode}.\n"
        f"stdout:\n{done.stdout}"
    )
    assert "FAIL" in done.stdout, f"el motivo deberia quedar escrito.\n{done.stdout}"


def test_the_component_strip_yields_the_component_not_the_archive() -> None:
    """La asercion que observo el fallo, aislada de todo lo demas.

    Se corre en bash porque el recorte lo hace bash: `${var%patron}` es una
    expansion de shell, y reproducirlo en otro lenguaje seria probar otra cosa.
    """
    script = (
        f'filename="cognicode-{VERSION}-{TARGET}.tar.gz"\n'
        # Lo que Kotlin produce: el valor ya esta resuelto en el texto.
        f'echo "literal: ${{filename%-{VERSION}-*}}\"\n'
        f'version="{VERSION}"\n'
        'echo "set:    ${filename%-$version-*}"\n'
        'unset version\n'
        'echo "unset:  ${filename%-$version-*}"\n'
    )
    done = subprocess.run(
        ["bash", "-c", script],
        capture_output=True,
        text=True,
        timeout=30,
        env={"PATH": "/usr/bin:/bin"},
    )
    assert done.returncode == 0, done.stderr
    lines = dict(line.split(":", 1) for line in done.stdout.strip().splitlines())
    assert lines["literal"].strip() == "cognicode", (
        f"con el valor resuelto en el texto el sufijo derivado se recorta.\n{done.stdout}"
    )
    assert lines["set"].strip() == "cognicode", (
        f"con la variable de shell fijada tambien.\n{done.stdout}"
    )
    # Esta es la trampa: vacia, el patron `%--*` no casa con nada y el nombre
    # completo sobrevive intacto, sin error y sin aviso.
    assert lines["unset"].strip() == f"cognicode-{VERSION}-{TARGET}.tar.gz", (
        f"sin valor, el recorte no hace nada: ese es el fallo medido.\n{done.stdout}"
    )


# --- La stage de skills, y por que preguntar no es responder -----------------


def test_the_skill_stage_stages_every_published_bundle() -> None:
    """El camino que la stage deberia recorrer siempre."""
    done = run_skill_stage("ok")
    assert done.returncode == 0, (
        f"la stage debe empaquetar los bundles y salir 0, y salio "
        f"{done.returncode}.\nstdout:\n{done.stdout}\nstderr:\n{done.stderr}"
    )
    assert done.staged == [
        f"cognicode-{VERSION}.tar.gz",
        f"cognicode-mcp-{VERSION}.tar.gz",
    ], f"un bundle por id publicado.\nobtenido: {done.staged}"


def test_the_skill_stage_fails_when_the_tool_errors() -> None:
    """La condicion exacta que se llevo la stage por delante.

    El binario real salia con 2 y `unrecognized subcommand` porque el
    subcommand no existia. La stage leia la respuesta por una tuberia, no
    consumia nada, y terminaba el bucle con exito: cero bundles, stage verde.
    """
    done = run_skill_stage("error")
    assert done.returncode != 0, (
        f"una herramienta que falla tiene que tumbar la stage; antes salia "
        f"{done.returncode} con cero bundles.\nstdout:\n{done.stdout}"
    )
    assert done.staged == [], f"no debe haber producido nada.\n{done.staged}"


def test_the_skill_stage_fails_on_an_empty_answer() -> None:
    """Salida 0 con la lista vacia tampoco es una respuesta valida.

    El contrato publica bundles; una lista vacia significa que la herramienta
    y el contrato discrepan, no que el producto no tenga skills.
    """
    done = run_skill_stage("empty")
    assert done.returncode != 0, (
        f"una lista vacia debe fallar; antes salia {done.returncode}.\n"
        f"stdout:\n{done.stdout}"
    )
    assert done.staged == [], f"no debe haber producido nada.\n{done.staged}"


# --- La clase entera, en todos los lanes -------------------------------------


def test_no_kotlin_value_is_written_as_a_shell_variable() -> None:
    offenders: list[str] = []
    for pipeline in sorted(REPO_ROOT.glob("*.pipeline.kts")):
        text = strip_comments(pipeline.read_text(encoding="utf-8"))
        for name in sorted(kotlin_scoped_names(pipeline.read_text(encoding="utf-8"))):
            if f"${{'$'}}{name}" in text:
                offenders.append(f"{pipeline.name}: ${{'$'}}{name}")
    assert not offenders, (
        "un valor que Kotlin ya tiene resuelto no se escribe como variable de "
        "shell; asi se recorta mal el nombre del componente y la stage falla "
        f"dos etapas mas tarde:\n{offenders}"
    )


def test_the_candidate_lane_uses_the_kotlin_version_in_the_strip() -> None:
    """La linea exacta del defecto, para que reintroducirla se note."""
    text = CANDIDATE.read_text(encoding="utf-8")
    escape = "${" + "'$'" + "}"
    assert f"component=\"{escape}{{filename%-$version-*}}\"" in text, (
        "el recorte del sufijo debe usar el valor de Kotlin"
    )
    assert f"filename%-{escape}version-}}" not in text, (
        "el recorte volvio a usar una variable de shell sin definir"
    )


def main() -> int:
    # `dir(globals())` NO lista los nombres del modulo: `globals()` devuelve un
    # dict, y `dir()` de un dict devuelve sus METODOS — `clear`, `keys`,
    # `values`. La lista salia vacia, el bucle no corria nunca, y este main()
    # imprimia PASS sin haber ejecutado un solo test. Un contrato que se declara
    # verde sin correr nada es exactamente el fallo que este repositorio se
    # niega a publicar, y lo traia el propio `main()` del contrato. El objeto
    # modulo si lista sus atributos, que es lo que usa `run_contract_tests.py`.
    module = sys.modules[__name__]
    tests = sorted(
        (
            name
            for name in dir(module)
            if name.startswith("test_") and callable(getattr(module, name))
        ),
        key=lambda name: getattr(module, name).__code__.co_firstlineno,
    )
    assert tests, (
        "ningun test_ encontrado: este main() no ejecutaria nada y reportaria "
        "PASS. Un contrato vacio es un fallo, no una linea verde."
    )
    failures: list[str] = []
    for test in tests:
        try:
            getattr(module, test)()
            print(f"  PASS {test}")
        except Exception as error:  # noqa: BLE001 - un contrato reporta, no propaga
            print(f"  FAIL {test}: {error}")
            failures.append(f"{test}: {error}")
    if failures:
        print(f"FAIL - {len(failures)} fallo(s)")
        return 1
    print(
        "PASS - la stage de empaquetado usa los valores de Kotlin, falla cuando "
        "no produce nada, ni recortar un componente del nombre del archivo "
        "con una variable de shell que nadie define."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
