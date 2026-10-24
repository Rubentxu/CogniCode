#!/usr/bin/env python3
"""Una etapa que no puede hacer caer la puerta declara qué observó.

El hueco
-------

`PipelineK` no tiene `continue-on-error`. La forma que la casa usa para que
una etapa reporte en vez de bloquear es terminar el comando con
`|| echo 'ADVISORY: …'`: el fallo queda escrito y el código de salida sigue
siendo cero.

El problema no es el mecanismo: es que **nadie sabe qué vale**. La palabra
`ADVISORY` no aparece en ningún contrato de `scripts/ci/`. Una etapa que
falla y no bloquea se declara en la prosa del comentario del pipeline, y en
ningún sitio más. Ese comentario ni siquiera es verificable: cita ficheros
que ya no están en el repositorio, porque la casa migró de GitHub Actions a
PipelineK y el nombre del workflow viejo se quedó en la prosa.

MEDIDO 2026-10-24, sobre el árbol de este repositorio:

    13 etapas tragan un fallo
        11 se declaran solas con el marcador `ADVISORY`
         2 se lo tragan con `|| echo` sin decir nada

Las trece están en la tabla de este fichero, con su motivo. El recuento se
comprueba contra el árbol: una etapa nueva que se marque advisory sin
declararse pone el contrato en rojo, y una etapa declarada que deja de
tragar un fallo también.

La regla
--------

1. **Tragar un fallo obliga a declararse.** El marcador `ADVISORY` es el
   mecanismo de declaración de la casa, y toda etapa que traga un fallo lo
   lleva. Las dos etapas que se tragan un fallo en silencio dejan de
   hacerlo.
2. **El motivo no cita ficheros que no están en el árbol.** Si cita un
   fichero, el fichero existe: un motivo que no se puede comprobar no es un
   motivo.
3. **Una etapa cuyo nombre dice que instala, instala.** La etapa de HOME
   limpio ejecuta el instalador de verdad, con `--apply`, y fija la raíz de
   instalación dentro de su propio HOME.

Lo que este contrato NO comprueba
---------------------------------

- **Que las etapas observen algo útil.** Declaran qué hacen; si lo que
  hacen sirve, es una decisión humana y la razón escrita es lo que
  sobrevive.
- **La genealogía de una lane.** Que un pipeline narré de dónde salió es
  historia y no es el motivo de una etapa. La regla de citas comprobables
  alcanza al motivo que justifica una etapa traga el fallo, no al relato de
  la migración.
- **Una etapa que captura un código de salida para comprobarlo.** `set +e`
  seguido de `rc=$?` y una aserción sobre ese código no se traga el fallo:
  lo exige. Sólo se traga el fallo la etapa que convierte un error en un
  código de salida cero.
- **Que una etapa optada por el entorno se haya ejecutado.** Una etapa que
  se salta porque su variable de entorno no está puesta no se traga un
  fallo: no llegó a observar nada.

Controles
---------

- (control) el inventario encuentra etapas; un inventario vacío pasaría
  siempre;
- (control) una etapa que traga un fallo y no se declara sale en rojo;
- (control) un motivo que cita un fichero ausente sale en rojo;
- (control) la comprobación de la instalación distingue una etapa que
  instala de una que sólo planifica.

Ejecutar:
    python3 scripts/ci/test_advisory_stages_are_declared.py
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

# El renderizador de los cuerpos de stage es compartido: el orden en que
# sustituye los valores de Kotlin ES la prueba, y dos contratos que
# ejecutan un cuerpo tienen que acordar ese orden. `run_contract_tests.py`
# pone el directorio del contrato en el `sys.path` justamente para que una
# suite pueda importar a su hermana.
import test_candidate_packaging as packaging

REPO_ROOT = Path(__file__).resolve().parents[2]

# El marcador de la casa: una etapa que lo lleva dice que reporta en vez de
# bloquear. Es el mismo texto que la prosa del pipeline usa.
ADVISORY_MARKER = "ADVISORY"

# El idiom que convierte un error en un código de salida cero. La etapa que
# lo usa se traga el fallo; la que usa `|| exit 1` o `|| { … exit 1 }` no.
SWALLOW = "|| echo"

# Las etapas que tragan un fallo, con el motivo por el que se quedan así.
#
# La clave es (fichero del pipeline, nombre de la etapa). El motivo es la
# declaración: qué observa la etapa y por qué su fallo no detiene la lane.
ADVISORY_STAGES: dict[str, dict[str, str]] = {
    "integration.pipeline.kts": {
        "parser-adversarial": (
            "el corpus que ataca al parser observa y se revisa; su rojo se "
            "registra para revisión en vez de detener la lane, porque "
            "endurecerlo sería una política de CI vestida de migración"
        ),
        "architecture-drift-e2e": (
            "la comprobación de deriva arquitectónica observa y se revisa; un "
            "fallo se registra y la lane continúa, por la misma razón que el "
            "corpus adversarial"
        ),
        "findings-canonical-grounding": (
            "el anclaje canónico de los hallazgos observa; un fallo se registra "
            "para revisión y no detiene la lane de integración"
        ),
        "workspace-isolation": (
            "el aislamiento de espacios de trabajo observa que dos espacios no "
            "colisionan; su fallo se registra y la lane continúa"
        ),
        "cogh-clean-home-install": (
            "la instalación en HOME limpio observa que el instalador funciona de "
            "verdad; si falla, la lane lo registra y sigue, y lo que queda "
            "escrito es un fallo de instalación en un HOME aislado"
        ),
        "benchmarks": (
            "los benchmarks miden tiempo: una regresión de rendimiento es un "
            "dato para revisar, y un rojo por la máquina que mide no es un "
            "veredicto sobre el producto"
        ),
        "version-sync": (
            "la sincronía de versiones entre el workspace y el manifiesto "
            "comprueba coherencia; su rojo se revisa y no detiene la lane"
        ),
    },
    "certification.pipeline.kts": {
        "sandbox-pull": (
            "las imágenes del sandbox se descargan de un registro cuyos digests "
            "se mueven aguas arriba: un cambio en el registro no es una "
            "regresión del sandbox, así que la stage lo registra y la lane "
            "continúa"
        ),
        "sandbox-setup": (
            "la preparación del sandbox observa; el smoke que va después es lo "
            "que ejercita de verdad, así que un fallo aquí se registra y no "
            "detiene la lane"
        ),
        "sandbox-ci-probe": (
            "los sondeos de capacidad clasifican fallos esperados por "
            "capacidad y son informativos sobre el host, no un veredicto sobre "
            "el producto; la lane los registra y continúa"
        ),
        "perf-budget-verdict": (
            "el veredicto de rendimiento no puede bloquear la lane todavía: el "
            "verificador sale con UNMEASURED cuando una operación presupuestada "
            "no tiene benchmark, y la stage dice cuál fue el veredicto en vez "
            "de convertirlo en un rojo"
        ),
    },
    "release-candidate.pipeline.kts": {
        "install-cargo-deny": (
            "la instalación de la herramienta no es el veredicto: si falla, la "
            "stage siguiente que la usa falla, y ese rojo sí detiene la lane"
        ),
    },
    "release.pipeline.kts": {
        "create-draft": (
            "la generación de las notas puede fallar y el borrador se crea con "
            "un título por defecto; la stage lo dice, y la publicación sigue "
            "con un título que no es el que el generador habría escrito"
        ),
    },
}

# Cuántas etapas tragan un fallo en cada lane. El número va escrito para que
# un cambio se vea en el diff, y se comprueba contra el árbol: una tabla que
# crece sin que el número cambie, o un número que cambia sin que la tabla
# cambie, ponen el contrato en rojo. No es una segunda fuente de verdad
# sobre *qué* etapas son —eso lo dice la tabla— sino el recuento declarado
# de las mismas, y las dos cosas se cruzan.
DECLARED_PER_LANE: dict[str, int] = {
    "integration.pipeline.kts": 7,
    "certification.pipeline.kts": 4,
    "release-candidate.pipeline.kts": 1,
    "release.pipeline.kts": 1,
}

# El nombre de la etapa que dice que instala. La regla: si el nombre dice
# «install», la etapa instala.
INSTALL_STAGE = "cogh-clean-home-install"

# Una etapa que nombra una instalación tiene que pedir la instalación de
# verdad. El instalador de la casa imprime un plan y sale con cero si no se
# le pasa esta opción, así que sin ella la etapa no instala nada.
APPLY_FLAG = "--apply"

# La variable con la que la casa fija la raíz de instalación. Es la primera
# de la precedencia de `resolve_install_root`
# (`crates/cognicode-cli/src/lifecycle.rs`), y por encima de `CARGO_HOME`,
# que en un entorno de CI está puesto y desviaría la instalación fuera del
# HOME que la etapa declara.
INSTALL_ROOT_VARIABLE = "COGNICODE_BIN_DIR"

# Las extensiones que hacen que un token sea el nombre de un fichero. La
# lista es cerrada a propósito: una extensión que no está aquí no es una
# cita, y un texto que parece un nombre (`e.g`, una versión) no es un
# fichero que no exista.
FILE_EXTENSIONS = frozenset(
    {
        "cfg",
        "ini",
        "json",
        "kts",
        "lock",
        "md",
        "py",
        "rs",
        "sh",
        "toml",
        "ts",
        "txt",
        "yaml",
        "yml",
    }
)

# Una cita a un fichero: un nombre con extensión, con o sin directorio. La
# forma entre acentos graves y la forma suelta son la misma cita, y las dos
# aparecen en la prosa de los pipelines.
CITATION = re.compile(
    r"(?<![\w/.-])"
    r"([A-Za-z0-9_][A-Za-z0-9_.-]*(?:/[A-Za-z0-9_.-]+)*\.[A-Za-z]{1,9})"
    r"(?![\w-])"
)

# La marca de una etapa. Se busca el nombre y luego la llave, que es donde
# está el cuerpo.
STAGE_MARKER = re.compile(r'stage\("([^"]+)"\)')

# Una llamada a `sh(`. El límite de palabra evita que `bash(` cuente.
SH_CALL = re.compile(r"\bsh\(")

# Los hallazgos se acumulan aquí y `main()` los convierte en un código de
# salida. Un contrato que informa de un fallo y sale con cero informa de un
# verde que no ocurrió.
failures: list[str] = []


# ---------------------------------------------------------------------------
# Lectura de los pipelines
# ---------------------------------------------------------------------------


def strip_comments(text: str) -> str:
    """Sustituye los comentarios por espacios, conservando offsets y líneas.

    Un comentario no se ejecuta, así que no puede ser la etapa que se
    busca. Y el desplazamiento importa: las posiciones se usan para
    reportar la línea, y un contrato que reporta la línea equivocada es un
    contrato que no se puede usar.

    Las cadenas se respetan: un `//` dentro de una URL no es un comentario,
    y las cadenas crudas de Kotlin se tragan enteras, con lo que un `#` de
    shell dentro de un cuerpo de stage deja de ser confusion.
    """
    out = list(text)
    index = 0
    length = len(text)
    while index < length:
        if text.startswith('"""', index):
            end = text.find('"""', index + 3)
            index = length if end == -1 else end + 3
            continue
        if text[index] == '"':
            index += 1
            while index < length:
                if text[index] == "\\":
                    index += 2
                    continue
                if text[index] == '"':
                    index += 1
                    break
                index += 1
            continue
        if text.startswith("//", index):
            end = text.find("\n", index)
            end = length if end == -1 else end
            for position in range(index, end):
                out[position] = " "
            index = end
            continue
        index += 1
    return "".join(out)


def _skip_call(text: str, index: int) -> int:
    """Devuelve la posición justo después del `)` que cierra la llamada."""
    depth = 1
    length = len(text)
    while index < length:
        if text.startswith('"""', index):
            end = text.find('"""', index + 3)
            index = length if end == -1 else end + 3
            continue
        if text[index] == '"':
            index += 1
            while index < length:
                if text[index] == "\\":
                    index += 2
                    continue
                if text[index] == '"':
                    index += 1
                    break
                index += 1
            continue
        if text[index] == "(":
            depth += 1
        elif text[index] == ")":
            depth -= 1
            if depth == 0:
                return index + 1
        index += 1
    return length


def stages_of(text: str) -> list[dict]:
    """Las etapas del pipeline, con su línea, su profundidad y sus ancestros.

    Una etapa puede contener otras: `adversarial` contiene las cuatro
    corpora. Para saber quién justifica a quién hace falta el anidamiento,
    y se mide con las llaves, que están fuera de las cadenas.
    """
    found: list[dict] = []
    stack: list[str] = []
    depth = 0
    pending: str | None = None
    index = 0
    length = len(text)
    while index < length:
        if text.startswith('"""', index):
            end = text.find('"""', index + 3)
            index = length if end == -1 else end + 3
            continue
        if text[index] == '"':
            index += 1
            while index < length:
                if text[index] == "\\":
                    index += 2
                    continue
                if text[index] == '"':
                    index += 1
                    break
                index += 1
            continue
        if text[index] == "{":
            depth += 1
            if pending is not None:
                stack.append(pending)
                found.append(
                    {
                        "name": pending,
                        "line": text.count("\n", 0, index) + 1,
                        "depth": depth,
                        "ancestors": tuple(stack[:-1]),
                    }
                )
                pending = None
            index += 1
            continue
        if text[index] == "}":
            depth -= 1
            if stack:
                stack.pop()
            index += 1
            continue
        match = STAGE_MARKER.match(text, index)
        if match:
            pending = match.group(1)
            index = match.end()
            continue
        index += 1
    return found


def calls_of(text: str) -> list[dict]:
    """Las llamadas `sh(...)` del pipeline, con su etapa y su argumento."""
    calls: list[dict] = []
    stack: list[str] = []
    depth = 0
    pending: str | None = None
    index = 0
    length = len(text)
    while index < length:
        if text.startswith('"""', index):
            end = text.find('"""', index + 3)
            index = length if end == -1 else end + 3
            continue
        if text[index] == '"':
            index += 1
            while index < length:
                if text[index] == "\\":
                    index += 2
                    continue
                if text[index] == '"':
                    index += 1
                    break
                index += 1
            continue
        if text[index] == "{":
            depth += 1
            if pending is not None:
                stack.append(pending)
                pending = None
            index += 1
            continue
        if text[index] == "}":
            depth -= 1
            if stack:
                stack.pop()
            index += 1
            continue
        match = STAGE_MARKER.match(text, index)
        if match:
            pending = match.group(1)
            index = match.end()
            continue
        match = SH_CALL.match(text, index)
        if match:
            end = _skip_call(text, match.end())
            calls.append(
                {
                    "stage": stack[-1] if stack else None,
                    "ancestors": tuple(stack[:-1]),
                    "argument": text[match.end() : end - 1],
                    "line": text.count("\n", 0, index) + 1,
                }
            )
            index = end
            continue
        index += 1
    return calls


def pipelines() -> dict[str, dict]:
    """Cada pipeline, con su código y sus etapas."""
    out: dict[str, dict] = {}
    for path in sorted(REPO_ROOT.glob("*.pipeline.kts")):
        raw = path.read_text(encoding="utf-8")
        stripped = strip_comments(raw)
        out[path.name] = {
            "raw": raw,
            "lines": raw.splitlines(),
            "stages": stages_of(stripped),
            "calls": calls_of(stripped),
        }
    return out


def swallowing_stages(lanes: dict[str, dict]) -> set[tuple[str, str]]:
    """Las etapas que tragan un fallo: por el idiom o por el marcador.

    Se cuenta por el efecto, no por la forma: una etapa se traga el fallo si
    convierte un error en un código de salida cero. El idiom `|| echo` lo
    hace por construcción, y el marcador `ADVISORY` lo declara, así que las
    dos señales cubren las dos formas que la casa usa.
    """
    found: set[tuple[str, str]] = set()
    for name, lane in lanes.items():
        for call in lane["calls"]:
            if call["stage"] is None:
                continue
            argument = call["argument"]
            if SWALLOW in argument or ADVISORY_MARKER in argument:
                found.add((name, call["stage"]))
    return found


def declared_stages() -> set[tuple[str, str]]:
    """Las etapas declaradas en la tabla de este fichero."""
    return {
        (name, stage)
        for name, stages in ADVISORY_STAGES.items()
        for stage in stages
    }


# ---------------------------------------------------------------------------
# Citas a ficheros
# ---------------------------------------------------------------------------


def tracked_files() -> set[str]:
    """Los ficheros versionados del repositorio.

    `git ls-files` es la autoridad: un checkout de terceros que la lane de
    release clona vive en `sandbox/repos/` y no está versionado, así que sus
    ficheros no son superficies de este repositorio. Si git no está, se
    recorre el árbol saltando los directorios que no son código.
    """
    completed = subprocess.run(
        ["git", "ls-files"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        timeout=120,
    )
    if completed.returncode == 0:
        return set(completed.stdout.splitlines())
    skipped = {
        ".git",
        ".pipelinek",
        ".venv",
        "__pycache__",
        "build",
        "dist",
        "node_modules",
        "target",
        "venv",
    }
    found: set[str] = set()
    for directory, subdirectories, names in os.walk(REPO_ROOT):
        subdirectories[:] = [
            name
            for name in subdirectories
            if name not in skipped and name != "repos"
        ]
        for name in names:
            found.add(str((Path(directory) / name).relative_to(REPO_ROOT)))
    return found


def citations(text: str) -> set[str]:
    """Los ficheros que el texto cita."""
    found: set[str] = set()
    for match in CITATION.finditer(text):
        token = match.group(1)
        if token.rsplit(".", 1)[-1].lower() in FILE_EXTENSIONS:
            found.add(token)
    return found


def resolves(token: str, tracked: set[str]) -> bool:
    """Si la cita apunta a un fichero que existe."""
    if (REPO_ROOT / token).exists():
        return True
    if "/" not in token and token in {Path(name).name for name in tracked}:
        return True
    return False


def comment_block_above(lines: list[str], line: int) -> list[str]:
    """El bloque de comentarios inmediatamente por encima de una línea."""
    block: list[str] = []
    index = line - 2
    while index >= 0:
        stripped = lines[index].strip()
        if not stripped.startswith("//"):
            break
        block.append(stripped)
        index -= 1
    return list(reversed(block))


def justifying_blocks(lanes: dict[str, dict], stages: set[tuple[str, str]]):
    """La prosa que justifica a cada etapa declarada.

    Son el bloque de comentarios justo encima de la etapa y el de cada una
    de sus etapas madre: una etapa dentro de un grupo hereda el motivo del
    grupo.
    """
    blocks: dict[tuple[str, str], list[str]] = {}
    for name, lane in lanes.items():
        by_name = {stage["name"]: stage for stage in lane["stages"]}
        for stage_name in sorted({stage for file, stage in stages if file == name}):
            stage = by_name.get(stage_name)
            if stage is None:
                continue
            lines: list[str] = []
            for ancestor in list(stage["ancestors"]) + [stage_name]:
                info = by_name.get(ancestor)
                if info is None:
                    continue
                lines.extend(comment_block_above(lane["lines"], info["line"]))
            blocks[(name, stage_name)] = lines
    return blocks


# ---------------------------------------------------------------------------
# La etapa que dice que instala
# ---------------------------------------------------------------------------


def run_install_stage(body: str | None = None) -> dict:
    """Ejecuta el cuerpo de una etapa de instalación con un `cogh` de mentira.

    El instalador real se mide en otro sitio; aquí lo que se mide es lo que
    la ETAPA le pide. El `cogh` de mentira anota sus argumentos y el entorno
    que ve, y no hace nada más.

    Sin `body`, se usa el cuerpo real de la etapa, renderizado contra el
    repositorio de mentira: el cuerpo hace `cd` al repositorio y llama a
    `scripts/ci/target-dir.sh`, y los dos tienen que ser los de mentira.

    El entorno lleva un `CARGO_HOME` señuelo a propósito: la precedencia de
    la raíz de instalación lo pone por delante de `XDG_DATA_HOME`, así que
    sin una fijación explícita la etapa instalaría fuera de su HOME limpio.
    """
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        repo = root / "repo"
        (repo / "scripts" / "ci").mkdir(parents=True)
        target_dir = root / "cargo-targets"
        (target_dir / "release").mkdir(parents=True)

        resolver = repo / "scripts" / "ci" / "target-dir.sh"
        resolver.write_text(
            "#!/usr/bin/env bash\nprintf '%s\\n' \"" + str(target_dir) + "\"\n",
            encoding="utf-8",
        )
        resolver.chmod(0o755)

        argv_file = root / "argv.txt"
        fake = target_dir / "release" / "cogh"
        fake.write_text(
            "#!/usr/bin/env bash\n"
            'printf "%s\\n" "$@" > "$COGNICODE_ARGV_FILE"\n'
            'printf "%s\\n" "$HOME" > "$COGNICODE_ARGV_FILE.home"\n'
            'printf "%s\\n" "${COGNICODE_BIN_DIR:-}" > "$COGNICODE_ARGV_FILE.root"\n'
            'printf "%s\\n" "${CARGO_HOME:-}" > "$COGNICODE_ARGV_FILE.cargo"\n'
            'printf "%s\\n" "${XDG_DATA_HOME:-}" > "$COGNICODE_ARGV_FILE.xdg"\n'
            "exit 0\n",
            encoding="utf-8",
        )
        fake.chmod(0o755)

        if body is None:
            body = install_body(str(repo))

        script = root / "stage.sh"
        script.write_text("#!/usr/bin/env bash\n" + body, encoding="utf-8")
        script.chmod(0o755)

        done = subprocess.run(
            [str(script)],
            capture_output=True,
            text=True,
            timeout=120,
            env={
                "PATH": "/usr/bin:/bin:/usr/local/bin",
                "HOME": str(root / "home"),
                "TMPDIR": str(root),
                "CARGO_HOME": str(root / "cargo-home-decoy"),
                "COGNICODE_ARGV_FILE": str(argv_file),
                "COGNICODE_FAKE_COGH": str(fake),
            },
        )
        argv = []
        if argv_file.exists():
            argv = argv_file.read_text(encoding="utf-8").split("\n")

        def sidecar(suffix: str) -> str:
            path = Path(str(argv_file) + suffix)
            if not path.exists():
                return ""
            return path.read_text(encoding="utf-8").strip()

        return {
            "argv": [line for line in argv if line],
            "home": sidecar(".home"),
            "install_root": sidecar(".root"),
            "cargo_home": sidecar(".cargo"),
            "xdg_data_home": sidecar(".xdg"),
            "returncode": done.returncode,
            "stdout": done.stdout,
            "stderr": done.stderr,
        }


def install_body(repo_root: str) -> str:
    """El cuerpo real de la etapa que dice que instala, ya renderizado.

    El renderizador es el de la casa y su orden es la prueba: primero los
    valores de Kotlin, después la conversión de `${'$'}` a `$`. Invertido,
    el cuerpo roto se ejecutaría como el arreglado.
    """
    text = (REPO_ROOT / "integration.pipeline.kts").read_text(encoding="utf-8")
    body = packaging.extract_stage_body(text, INSTALL_STAGE)
    return packaging.render(
        body,
        version="0.0.0",
        target="x86_64-unknown-linux-gnu",
        platform="linux-x86_64",
        repo_root=repo_root,
    )


def asks_for_the_install(observation: dict) -> bool:
    """Si la etapa le pidió al instalador que instalara de verdad."""
    argv = observation["argv"]
    return "install" in argv and APPLY_FLAG in argv


def installs_inside_its_home(observation: dict) -> bool:
    """Si la raíz de instalación cae dentro del HOME que la etapa declara.

    La raíz la elige el entorno, con esta precedencia: la fijación
    explícita, `CARGO_HOME`, `XDG_DATA_HOME` y, por último, el HOME. La
    etapa declara su HOME y su `XDG_DATA_HOME`; lo que no puede dejar es
    que el entorno redireccione la instalación a otra parte.
    """
    home = observation["home"]
    if not home:
        return False
    prefix = home.rstrip("/") + "/"

    root = observation["install_root"]
    if root:
        return root == home or root.startswith(prefix)

    # Sin fijación explícita, la decisión la toma el entorno: si
    # `CARGO_HOME` está puesto, gana él, y la instalación sale del HOME.
    if observation["cargo_home"]:
        return False
    xdg = observation["xdg_data_home"]
    return bool(xdg) and (xdg == home or xdg.startswith(prefix))


# ---------------------------------------------------------------------------
# Las comprobaciones
# ---------------------------------------------------------------------------


def test_every_stage_that_swallows_a_failure_is_declared() -> None:
    """Ninguna etapa traga un fallo sin decir qué observa."""
    lanes = pipelines()
    found = swallowing_stages(lanes)
    declared = declared_stages()
    missing = sorted(found - declared)
    assert not missing, (
        "estas etapas tragan un fallo y no están declaradas aquí, así que "
        "nadie puede decir qué valen:\n  "
        + "\n  ".join(f"{name}: {stage}" for name, stage in missing)
        + "\nDecláralas con su motivo, o deja de tragar el fallo."
    )


def test_every_stage_that_swallows_a_failure_says_it_is_advisory() -> None:
    """Tragar un fallo obliga a declararse.

    El marcador `ADVISORY` es el mecanismo de declaración de la casa: dice
    que la etapa reporta en vez de bloquear. Una etapa que se traga un
    fallo con `|| echo` y no lo dice se traga el fallo en silencio, que es
    exactamente el hueco que este contrato cierra.
    """
    lanes = pipelines()
    silent: list[str] = []
    for name, lane in lanes.items():
        for call in lane["calls"]:
            if call["stage"] is None:
                continue
            argument = call["argument"]
            if SWALLOW in argument and ADVISORY_MARKER not in argument:
                silent.append(f"{name}: {call['stage']}")
    assert not silent, (
        "estas etapas se tragan un fallo y no lo declaran:\n  "
        + "\n  ".join(sorted(silent))
        + "\nEl marcador de la casa dice que la etapa reporta en vez de "
        "bloquear. Sin él, el fallo se traga en silencio."
    )


def test_every_declared_stage_still_swallows_a_failure() -> None:
    """Una etapa declarada que deja de tragar un fallo es una entrada vieja."""
    lanes = pipelines()
    found = swallowing_stages(lanes)
    declared = declared_stages()
    stale = sorted(declared - found)
    assert not stale, (
        "estas etapas están declaradas aquí pero ya no tragan un fallo: o la "
        "declaración quedó vieja, o la etapa se movió:\n  "
        + "\n  ".join(f"{name}: {stage}" for name, stage in stale)
        + "\nUna declaración que describe una etapa que ya no existe no "
        "declara nada."
    )


def test_the_declared_count_matches_the_tree() -> None:
    """El recuento declarado es el recuento del árbol, no una memoria."""
    lanes = pipelines()
    found = swallowing_stages(lanes)
    counted: dict[str, int] = {}
    for name, _ in found:
        counted[name] = counted.get(name, 0) + 1
    wrong = [
        f"{name}: declarado {DECLARED_PER_LANE.get(name, 0)}, encontrado {count}"
        for name, count in sorted(counted.items())
        if DECLARED_PER_LANE.get(name, 0) != count
    ]
    missing_lanes = [
        f"{name}: declarado {count}, encontrado 0"
        for name, count in sorted(DECLARED_PER_LANE.items())
        if name not in counted
    ]
    assert not wrong and not missing_lanes, (
        "el recuento de etapas que tragan un fallo no coincide con el árbol:\n  "
        + "\n  ".join(wrong + missing_lanes)
        + "\nUna etapa que aparece o desaparece detrás del recuento es un "
        "cambio que alguien tiene que decidir."
    )
    assert len(found) == sum(DECLARED_PER_LANE.values()), (
        f"el total declarado es {sum(DECLARED_PER_LANE.values())} y el árbol "
        f"tiene {len(found)} etapas que tragan un fallo."
    )


def test_every_declared_stage_says_why() -> None:
    """Una etapa declarada sin motivo no declara nada."""
    silent = [
        f"{name}: {stage}"
        for name, stages in ADVISORY_STAGES.items()
        for stage, reason in sorted(stages.items())
        if not reason.strip()
    ]
    assert not silent, (
        "estas etapas están declaradas sin motivo:\n  "
        + "\n  ".join(silent)
        + "\nUna etapa que no dice por qué se queda así no dice nada."
    )


def test_no_reason_cites_a_file_that_does_not_exist() -> None:
    """El motivo se tiene que poder comprobar donde vive."""
    tracked = tracked_files()
    missing: list[str] = []
    for name, stages in ADVISORY_STAGES.items():
        for stage, reason in sorted(stages.items()):
            for token in sorted(citations(reason)):
                if not resolves(token, tracked):
                    missing.append(f"{name}: {stage} cita `{token}`")
    assert not missing, (
        "estos motivos citan ficheros que no existen en el árbol:\n  "
        + "\n  ".join(missing)
        + "\nUn motivo que apunta a un fichero ausente no se puede comprobar: "
        "describe un mundo que ya no está."
    )


def test_the_prose_that_justifies_a_stage_cites_files_that_exist() -> None:
    """La prosa que justifica una etapa tampoco puede citar fantasmas."""
    lanes = pipelines()
    tracked = tracked_files()
    missing: set[str] = set()
    for (name, stage), lines in sorted(
        justifying_blocks(lanes, declared_stages()).items()
    ):
        text = "\n".join(lines)
        for token in sorted(citations(text)):
            if not resolves(token, tracked):
                missing.add(f"{name}: {stage} cita `{token}`")
    assert not missing, (
        "la prosa que justifica estas etapas cita ficheros que no existen:\n  "
        + "\n  ".join(sorted(missing))
        + "\nEl motivo de una etapa tiene que ser verificable donde vive: si "
        "cita un fichero, el fichero existe."
    )


def test_the_install_stage_installs() -> None:
    """La etapa que dice que instala, instala."""
    observation = run_install_stage()
    assert observation["returncode"] == 0, (
        "el cuerpo de la etapa de instalación no terminó con éxito:\n"
        f"stdout:\n{observation['stdout']}\nstderr:\n{observation['stderr']}"
    )
    assert observation["argv"], (
        "la etapa de instalación no llegó a invocar al instalador:\n"
        f"stdout:\n{observation['stdout']}\nstderr:\n{observation['stderr']}"
    )
    assert asks_for_the_install(observation), (
        "la etapa que se llama «instalación en HOME limpio» no pidió la "
        f"instalación: invocó al instalador con {observation['argv']}. Sin "
        "la opción de aplicar, el instalador imprime un plan y sale con cero: "
        "la etapa comprueba que el planificador sabe dónde instalaría, no que "
        "instalara."
    )


def test_the_install_stage_installs_inside_its_clean_home() -> None:
    """La instalación cae dentro del HOME que la etapa declara."""
    observation = run_install_stage()
    assert installs_inside_its_home(observation), (
        "la etapa declara un HOME limpio pero la instalación caería fuera: "
        f"la raíz elegida fue `{observation['install_root']}` y el HOME de la "
        f"etapa es `{observation['home']}`. Con el entorno puesto, la raíz la "
        f"decide `CARGO_HOME` (`{observation['cargo_home']}`), no el HOME que "
        "la etapa declara."
    )


# ---------------------------------------------------------------------------
# Controles
# ---------------------------------------------------------------------------


def test_the_inventory_finds_stages() -> None:
    """Control: el inventario tiene que encontrar etapas.

    Un contrato que no encuentra nada pasa siempre. Este control mide que
    el inventario está vivo, sobre el árbol real: si nadie hubiera declarado
    nada, el contrato no podría distinguir «todo correcto» de «no miré nada».
    """
    lanes = pipelines()
    calls = [
        call
        for lane in lanes.values()
        for call in lane["calls"]
        if call["stage"] is not None
    ]
    assert calls, (
        "el inventario no ha encontrado ninguna etapa con cuerpo. Si el "
        "escáner dejó de leer los pipelines, este contrato pasaría siempre."
    )


def test_a_stage_that_swallows_without_declaring_is_reported() -> None:
    """Control: una etapa que traga un fallo y no se declara sale en rojo."""
    lanes = {
        "inventado.pipeline.kts": {
            "raw": "",
            "lines": [],
            "stages": [],
            "calls": [
                {
                    "stage": "inventada",
                    "ancestors": (),
                    "argument": "cargo test || echo 'ADVISORY: something'",
                    "line": 1,
                }
            ],
        }
    }
    found = swallowing_stages(lanes)
    assert ("inventado.pipeline.kts", "inventada") in found, (
        "FAIL: una etapa que traga un fallo no fue detectada. La regla no "
        "está mirando lo que dice mirar."
    )
    assert ("inventado.pipeline.kts", "inventada") not in declared_stages(), (
        "FAIL: la etapa inventada figura como declarada. La detección y la "
        "declaración se han mezclado."
    )


def test_a_reason_citing_an_absent_file_is_reported() -> None:
    """Control: un motivo que cita un fichero ausente sale en rojo."""
    tracked = tracked_files()
    assert not resolves("ci.yml", tracked), (
        "FAIL: `ci.yml` se resuelve en este árbol, así que el control no "
        "demuestra nada. El fichero que este control usa tiene que estar "
        "ausente de verdad."
    )
    assert not resolves("workflow-que-no-existe.yml", tracked), (
        "FAIL: una cita a un fichero inventado se resolvió. La comprobación "
        "no está mirando lo que dice mirar."
    )


def test_the_install_check_distinguishes_a_plan_from_an_install() -> None:
    """Control: la comprobación distingue instalar de planear."""
    without = run_install_stage('"$COGNICODE_FAKE_COGH" install --profile core\n')
    with_apply = run_install_stage(
        '"$COGNICODE_FAKE_COGH" install --profile core --apply\n'
    )
    assert not asks_for_the_install(without), (
        "FAIL: una etapa que sólo planifica pasó como si instalara. La "
        "comprobación no distingue las dos."
    )
    assert asks_for_the_install(with_apply), (
        "FAIL: una etapa que instala de verdad no pasó como instaladora. La "
        "comprobación no puede pasar nada, que no es lo mismo que comprobar."
    )


def main() -> int:
    """Ejecuta las comprobaciones y devuelve 1 si alguna falla.

    El contrato de los contratos exige esta forma: un fichero que acumula
    sus hallazgos en una lista tiene que convertir esa lista en un código de
    salida distinto de cero, o ejecutarlo a mano informaría de un éxito que
    no ocurrió.
    """
    for name, function in sorted(globals().items()):
        if not name.startswith("test_") or not callable(function):
            continue
        try:
            function()
            print(f"PASS: {name}")
        except AssertionError as error:
            failures.append(f"FAIL: {name}: {error}")
            print(f"FAIL: {name}: {error}", file=sys.stderr)
        except Exception as error:  # noqa: BLE001 — el contrato informa, no aborta
            failures.append(f"ERROR: {name}: {error}")
            print(f"ERROR: {name}: {error}", file=sys.stderr)

    if failures:
        print(f"\n{len(failures)} contract(s) failed", file=sys.stderr)
        return 1
    print("\nall contracts passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
