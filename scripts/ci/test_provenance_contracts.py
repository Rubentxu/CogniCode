#!/usr/bin/env python3
"""Contrato: la provenance de la release vive en un sitio y se firma con una versión fijada.

Dos contratos del mismo carril de provenance, reunidos en un solo fichero. No
comparten un sujeto único: cada sección vigila el suyo.

  I.  `scripts/ci/verify-provenance.sh`: la stage de provenance no puede ser
      más estricta que la política que documenta, y el verificador es la única
      implementación de esa política.
  II. `scripts/ci/cosign-tool.sh`, el generador y el instalador: el carril
      firma con la versión de cosign que dice, no con la que aparece en el PATH.

# I. La stage de provenance no puede ser mas estricta que la politica que documenta


MEDIDO 2026-10-03. Pre-auditoria de `release.pipeline.kts` antes de publicar
`v0.101.3`, leyendo el gate en vez de confiar en la intencion del header. La
seccion de cabecera (lineas 36-47) dice:

    The generation half is an open item, not a silent drop: `provenance` below
    fails closed when `RELEASE_REQUIRE_PROVENANCE=1` and no attestation exists
    ... publishing runs without generated provenance and the lane does not
    pretend otherwise.

El codigo de esa misma stage no hace eso —la stage, abreviada:

    stage("attestations") {
        for f in release/*.tar.gz; do
            gh attestation verify "$f" --repo "$GITHUB_REPOSITORY"
        done
        gh attestation verify release/SHA256SUMS --repo "$GITHUB_REPOSITORY"
    }

No hay condicion. La stage verifica siempre y falla siempre, porque nada en el
repositorio genera una atestation -- `actions/attest-build-provenance` era una
GitHub Action y PipelineK no tiene runtime de actions. Medido, no supuesto:

    $ gh attestation verify CHANGELOG.md --repo Rubentxu/CogniCode
    Error: HTTP 404: Not Found (.../attestations/sha256:2f4a6c13...)
    rc=1

La consecuencia no era una release sin provenance: era una release imposible.
`provenance` esta antes de `publish`, asi que la lane moria ahi y `v0.101.3` no
llegaba nunca a existir. El header describia una politica; el gate aplicaba otra.

Dos stages leian la misma variable: `attestations` sin switch, y
`provenance-required` con el. Que dos etapas implementen una politica es
exactamente como empezzo la discrepancia, asi que ahora hay una sola
implementacion —`scripts/ci/verify-provenance.sh`— y una sola stage que la
llama.

## Por que un contrato y no un comentario

Un comentario sobre la stage habria recreado el mismo fallo: describia la
intencion y el gate seguia roto. Es la forma del hallazgo de `--abbrev-ref` y
del trap del preflight —una guarda cuya condicion de disparo se cumple en el
caso normal y cuyo fallo nadie ve porque el camino de error no se ejecuta nunca.

La propiedad se observa en milisegundos con un `gh` de mentira. El preflight
real cuesta veinte minutos y 4G de clon; el falso reproduce exactamente la regla
que fallo, incluida la salida 1 con el 404 que devolvio `gh` de verdad.

## Que se comprueba

- Sin atestation y sin `RELEASE_REQUIRE_PROVENANCE=1`: la release **pasa**, y la
  salida lo dice. Esto es lo que hacia inalcanzable a `v0.101.3`.
- Sin atestation y con `RELEASE_REQUIRE_PROVENANCE=1`: la release **falla**. Un
  arreglo que dejase pasar ambos casos convertiria el gate en decoracion.
- Con atestation: pasa en los dos modos. El gate no es un fallo constante
  disfrazado de politica.
- Se comprueba **todos** los artefactos, no solo el primero.
- Un artefacto que no existe es fatal en los dos modos: un gate al que se le
  pide verificar algo ausente se ha invocado mal, y eso no es un veredicto de
  provenance.
- Estructural: `release.pipeline.kts` llama al script y no contiene un
  `gh attestation verify` suelto, que es la forma exacta del defecto.

# II. El lane de provenance tiene que firmar con la version que dice, no con la que aparece


## El hueco

`cosign attest-blob` cambio de comportamiento en la v3.1.4: `--tlog-upload` ya no
existe, el registro de transparencia paso a ser obligatorio, y cada llamada pide
aceptar los terminos del servicio de sigstore.

MEDIDO, con y sin terminal:

- sin terminal, el prompt se resuelve como `Error: user declined the prompt` y el
  comando sale con rc=1;
- con terminal, respondiendo `y`, firma y publica la huella en el registro
  publico de Rekor;
- la aceptacion no se cachea: el prompt vuelve en cada llamada.

Un lane no tiene terminal. MEDIDO: un step de PipelineK arranca con
`STDIN: NO TTY` y `STDOUT: NO TTY`, y si encuentra `cosign` en el PATH
(`/usr/bin/cosign`).

El resultado fue que `scripts/ci/test_provenance_generation.py` dejaba dos
comprobaciones en rojo, y que el `merge-gate` —que corre en esta misma
maquina— no podia pasar su etapa de contratos.

## Lo que se anade

Una regla derivada de la decision, no una segunda version: **el lane firma con
la version fijada, y nadie usa el `cosign` que aparezca en el PATH**.

- la version fijada vive en un solo sitio (`scripts/ci/cosign-tool.sh`), y el
  generador y el contrato beben de ahi;
- el generador y el contrato no pueden invocar un `cosign` suelto;
- el `merge-gate` y la lane que firma instalan la version fijada antes de
  usarla;
- el binario instalado tiene que ser el fijado: misma version, misma suma.

La ultima comprobacion es la que cierra el hueco de verdad: un generador que
resuelve la herramienta por el PATH firma con lo que haya, y un binario que se
instala sin verificar su suma no es el binario que el lane cree que tiene.

## Lo que NO se comprueba, y por que

- **La custodia de la clave.** No cambia: sigue siendo la decision abierta del
  ADR de procedencia. Este contrato no fabrica ninguna clave.
- **Si la version fijada es la mejor.** Que 3.1.3 sea la correcta es una
  decision del maintainer, escrita en el ADR. Aqui solo se comprueba que el
  repo y el lane dicen la misma.
- **Que la red funcione en el momento de instalar.** La suma se comprueba; que
  el binario se pueda descargar es una propiedad del momento, y el instalador
  falla con un mensaje que lo dice.

## El por que de fijarla y no de migrar

Las tres vias se midieron con la v3.1.4 antes de decidir:

| Via | Firmar | Verificar | Sale algo de la maquina |
|---|---|---|---|
| `--yes` | sin prompt | limpio | **si**, al registro publico de Rekor |
| `--signing-config` sin tlog | sin prompt, sin red | pide `--insecure-ignore-tlog` | no |
| version fijada (3.1.3) | sin prompt, sin red | limpio | no |

Fijar la version conserva exactamente lo que el ADR ya midio —firmar offline,
sin registro, con la clave que acompana a la release— y no convierte al
consumidor en alguien que tiene que pasar un flag que el propio cosign declara
inseguro. Publicar en Rekor sigue siendo una decision valida, pero es otra
decision, y no la toma un gate.

Run:
    python3 scripts/ci/test_provenance_contracts.py
"""

from __future__ import annotations

import hashlib
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]

# ---------------------------------------------------------------------------
# Compartido: constantes del carril de cosign, hallazgos y ayudantes de lectura
# ---------------------------------------------------------------------------

TOOL = REPO_ROOT / "scripts" / "ci" / "cosign-tool.sh"
INSTALLER = REPO_ROOT / "scripts" / "ci" / "install-cosign.sh"
GENERATOR = REPO_ROOT / "scripts" / "ci" / "attest-provenance.sh"
CONTRACT = REPO_ROOT / "scripts" / "ci" / "test_provenance_generation.py"
ADR = REPO_ROOT / "docs" / "adr" / "ADR-RELEASE-PROVENANCE-PIPELINEK.md"

# The version this repository signs with. It is the one the ADR measured as
# signing offline, with no transparency log, and the one whose behaviour the
# release's provenance statement was written against. The declaration here is
# deliberately separate from the one in `cosign-tool.sh`: a contract that read
# the value it is checking would agree with whatever the code says.
PINNED_COSIGN = "3.1.3"

failures: list[str] = []


def check(condition: bool, message: str) -> None:
    """Record a finding. The runner reads the list; nothing raises."""
    if not condition:
        failures.append(message)


def _text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return ""


def _tool_version() -> str | None:
    """The version the shared tool declares, or None if it does not say."""
    match = re.search(r'COGNICODE_COSIGN_VERSION="([^"]+)"', _text(TOOL))
    return match.group(1) if match else None


def _tool_binary() -> Path | None:
    """Where the pinned binary lives, if it is installed."""
    result = subprocess.run(
        ["bash", str(TOOL), "--path"],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if result.returncode != 0:
        return None
    path = Path(result.stdout.strip())
    return path if path.is_file() else None


def _repo_scripts() -> list[Path]:
    """Every shell script that belongs to THIS repository.

    `rglob` over the repository root also walks `.claude/worktrees/`, where an
    agent's checkout holds its own copy of every script. A copy in another
    working tree is not a second installer.
    """
    skip = {".git", "node_modules", "target", ".claude", "leases", "staging"}
    return [
        path
        for path in REPO_ROOT.rglob("*.sh")
        if not any(part in skip for part in path.relative_to(REPO_ROOT).parts)
    ]


# ---------------------------------------------------------------------------
# I. La stage de provenance no es mas estricta que la politica que documenta
# ---------------------------------------------------------------------------

GATE = REPO_ROOT / "scripts" / "ci" / "verify-provenance.sh"

# `gh` de mentira. Reproduce lo medido el 2026-10-03 — exit 1 y el 404 que
# devuelve la API cuando el repositorio no tiene atestationes.
#
# `set -euo pipefail` no es decoracion: la primera version de este falso tenia
# una sustitucion mal escrita, `mode` quedaba vacio, el `case` caia en su rama
# por defecto y el gate informaba **VERIFIED para todos los artefactos**. Un
# falso que no puede fallar es peor que no tener falso, porque convierte
# cualquier conclusion en un sello de goma. Este aborta ante un modo desconocido
# en vez de asumir exito.
FAKE_GH = """#!/usr/bin/env bash
set -euo pipefail

# GH_MODE=ok        toda atestation verifica
# GH_MODE=missing   ninguna verifica (404 medido)
# GH_MATCH=<sub>    solo los artefactos cuyo path contenga <sub> dan 404
#
# El fallo se decide por NOMBRE y no por orden de llamada: cada invocacion de
# `gh` es un proceso distinto, asi que un contador de posicion seria siempre 1
# y el falso no podria distinguir el segundo artefacto del primero.
mode="${GH_MODE:-ok}"
match="${GH_MATCH:-}"

target=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --repo) shift 2 ;;
        *) target="$1"; shift ;;
    esac
done
if [ -z "$target" ]; then
    echo "fake gh: no artifact given" >&2
    exit 2
fi

should_fail=0
case "$mode" in
    ok) should_fail=0 ;;
    missing) should_fail=1 ;;
    *)
        echo "fake gh: unknown GH_MODE '$mode'" >&2
        exit 2
        ;;
esac

if [ -n "$match" ]; then
    case "$target" in
        *"$match"*) should_fail=1 ;;
    esac
fi

if [ "$should_fail" -eq 1 ]; then
    echo "Error: HTTP 404: Not Found (https://api.github.com/repos/o/r/attestations/sha256:deadbeef)" >&2
    exit 1
fi
echo "Loaded digest sha256:deadbeef for $target"
"""


def run_gate(
    artifacts: list[str],
    *,
    enforce: str | None,
    gh_mode: str = "ok",
    gh_match: str = "",
) -> subprocess.CompletedProcess:
    """Ejecuta el gate con un `gh` de mentira y devuelve su estado real."""
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        bindir = root / "bin"
        bindir.mkdir()

        gh = bindir / "gh"
        gh.write_text(FAKE_GH, encoding="utf-8")
        gh.chmod(0o755)

        made: list[str] = []
        for index, name in enumerate(artifacts):
            path = root / name
            path.write_text(f"artefacto {index}\n", encoding="utf-8")
            made.append(str(path))

        env = {
            "PATH": f"{bindir}:/usr/bin:/bin",
            "HOME": str(root),
            "TMPDIR": str(root),
            "GH_MODE": gh_mode,
            "GH_MATCH": gh_match,
        }
        if enforce is not None:
            env["RELEASE_REQUIRE_PROVENANCE"] = enforce

        return subprocess.run(
            [str(GATE), "owner/name", *made],
            capture_output=True,
            text=True,
            timeout=60,
            env=env,
        )
# --- Tests ------------------------------------------------------------------


def test_the_gate_exists_and_is_executable() -> None:
    assert GATE.is_file(), f"no existe {GATE}"
    assert os.access(GATE, os.X_OK), f"{GATE} no es ejecutable"


def test_unattested_passes_when_provenance_is_not_enforced() -> None:
    """La condicion que hacia la release inalcanzable."""
    done = run_gate(["a.tar.gz", "SHA256SUMS"], enforce=None, gh_mode="missing")
    assert done.returncode == 0, (
        f"sin atestation y sin exigirla, la release debe poder publicarse, y "
        f"salio {done.returncode}.\nstdout:\n{done.stdout}\nstderr:\n{done.stderr}"
    )
    assert "NO PROVENANCE" in done.stdout, (
        f"la stage tiene que decir que no hay atestation, no omitirlo en "
        f"silencio.\nstdout:\n{done.stdout}"
    )
    assert "WARNING" in done.stdout, (
        f"publicar sin provenance es una decision y tiene que aparecer como "
        f"tal.\nstdout:\n{done.stdout}"
    )


def test_unattested_fails_when_provenance_is_enforced() -> None:
    """La otra mitad de la politica: exigirla tiene que significar algo."""
    done = run_gate(["a.tar.gz", "SHA256SUMS"], enforce="1", gh_mode="missing")
    assert done.returncode != 0, (
        f"con RELEASE_REQUIRE_PROVENANCE=1 y sin atestation debe fallar, y salio "
        f"{done.returncode}. Un arreglo que dejase pasar ambos casos habria "
        f"convertido el gate en decoracion.\nstdout:\n{done.stdout}"
    )
    assert "FAIL" in done.stdout, f"el motivo deberia quedar escrito.\n{done.stdout}"


def test_attested_passes_in_both_modes() -> None:
    """El gate no puede ser un fallo constante disfrazado de politica."""
    for enforce in (None, "1"):
        done = run_gate(["a.tar.gz", "SHA256SUMS"], enforce=enforce, gh_mode="ok")
        assert done.returncode == 0, (
            f"con atestation verificada debe pasar (enforce={enforce}), y salio "
            f"{done.returncode}.\nstdout:\n{done.stdout}"
        )
        assert "VERIFIED" in done.stdout, done.stdout


def test_an_enforcement_value_other_than_one_is_not_enforcement() -> None:
    """`RELEASE_REQUIRE_PROVENANCE=0` no es `1`: no debe fallar por error."""
    done = run_gate(["a.tar.gz"], enforce="0", gh_mode="missing")
    assert done.returncode == 0, f"0 no es 1, pero salio {done.returncode}.\n{done.stdout}"


def test_every_artifact_is_checked_not_only_the_first() -> None:
    """Un gate que solo mira el primero no puede afirmar nada del resto.

    Solo el ULTIMO artefacto carece de atestation. Un gate que se quedara en el
    primero informaria cero ausencias; este informa exactamente una.
    """
    done = run_gate(
        ["a.tar.gz", "b.tar.gz", "c.tar.gz"],
        enforce=None,
        gh_mode="ok",
        gh_match="c.tar.gz",
    )
    assert done.returncode == 0, done.stdout
    assert done.stdout.count("VERIFIED") == 2, (
        f"los dos primeros deben verificarse.\nstdout:\n{done.stdout}"
    )
    assert done.stdout.count("NO PROVENANCE") == 1, (
        f"solo el ultimo carece de atestation.\nstdout:\n{done.stdout}"
    )


def test_a_missing_artifact_is_fatal_in_both_modes() -> None:
    """Un artefacto ausente es una invocacion rota, no un veredicto.

    Sin esto, un gate al que se le pide verificar algo que no esta reportaria
    `NO PROVENANCE` y dejaria pasar lo que de verdad es un error de llamada.
    """
    for enforce in (None, "1"):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            bindir = root / "bin"
            bindir.mkdir()
            gh = bindir / "gh"
            gh.write_text(FAKE_GH, encoding="utf-8")
            gh.chmod(0o755)
            env = {
                "PATH": f"{bindir}:/usr/bin:/bin",
                "HOME": str(root),
                "TMPDIR": str(root),
                "GH_MODE": "ok",
                "GH_MATCH": "",
            }
            if enforce is not None:
                env["RELEASE_REQUIRE_PROVENANCE"] = enforce
            done = subprocess.run(
                [str(GATE), "owner/name", str(root / "nope.tar.gz")],
                capture_output=True,
                text=True,
                timeout=60,
                env=env,
            )
        assert done.returncode != 0, (
            f"un artefacto inexistente debe ser fatal (enforce={enforce}), y "
            f"salio {done.returncode}."
        )
        assert "MISSING FILE" in done.stdout, done.stdout


def test_no_arguments_is_a_usage_error() -> None:
    done = subprocess.run(
        [str(GATE)],
        capture_output=True,
        text=True,
        timeout=60,
        env={"PATH": "/usr/bin:/bin", "HOME": "/tmp", "TMPDIR": "/tmp"},
    )
    assert done.returncode == 2, (
        f"sin argumentos debe ser error de uso, y salio {done.returncode}"
    )


# --- La forma del gate, que es donde estaba el defecto ------------------------


def test_the_verifier_is_the_only_implementation_of_the_policy() -> None:
    """La politica vive en el script, y en ningun otro sitio.

    Antes de 2026-11-04 estos tres tests leian `release.pipeline.kts`: que
    delegase en el script, que no verificase atestaciones por su cuenta, y que
    no expandiese la variable de politica. Las lanes se retiraron, asi que las
    tres preguntas se hacen ahora del script y del resto del arbol: el
    verificador tiene que ser el unico que implementa la politica, y nadie mas
    puede abrir una segunda.
    """
    verifier_text = GATE.read_text(encoding="utf-8")
    assert "verify-provenance.sh" in str(GATE), "el verificador no esta donde se espera"

    # Nadie mas implementa la verificacion. `gh attestation verify` en otro
    # fichero seria una segunda implementacion con la misma palabra.
    others = [
        path
        for path in _repo_scripts()
        if path != GATE and "gh attestation verify" in path.read_text(encoding="utf-8", errors="ignore")
    ]
    assert not others, (
        f"la verificacion de atestaciones esta implementada tambien en {others}; "
        f"la politica tiene un solo sitio: {GATE.name}"
    )

    # Y nadie mas expande la variable de politica.
    readers = [
        path
        for path in _repo_scripts()
        if "${RELEASE_REQUIRE_PROVENANCE" in path.read_text(encoding="utf-8", errors="ignore")
    ]
    assert readers == [GATE], (
        f"la politica se expande en {readers}; deberia expandirse solo en "
        f"{GATE.name}. Dos fuentes de verdad son un desacuerdo esperando a "
        f"ocurrir."
    )


# ---------------------------------------------------------------------------
# II. El lane firma con la version fijada
# ---------------------------------------------------------------------------

def test_the_files_this_contract_reads_exist() -> None:
    """Anti-vacuity: a contract that reads nothing measures nothing.

    Without this, a missing tool file would make every other check report
    "the lane does not pin the tool" — which reads like a genuine gap rather
    than a contract looking in the wrong place.
    """
    for path in (TOOL, INSTALLER, GENERATOR, CONTRACT, ADR):
        check(
            path.is_file(),
            f"{path.name} does not exist, so this contract cannot measure the "
            "pin it is supposed to measure",
        )


def test_the_version_is_declared_in_one_place() -> None:
    """The pin lives in the shared tool, and nowhere else.

    Two declarations of the pinned version are two versions waiting to
    disagree: the generator would sign with one and the contract would measure
    the other.
    """
    version = _tool_version()
    check(
        version is not None,
        "the shared tool does not declare the pinned version, so nothing "
        "declares it at all",
    )
    if version is not None:
        check(
            version == PINNED_COSIGN,
            f"the shared tool pins cosign {version}, but the version this "
            f"contract pins is {PINNED_COSIGN}. Either the lane changed its "
            "signer and the contract did not, or the contract is measuring a "
            "version the lane stopped using",
        )

    for path in (GENERATOR, CONTRACT):
        text = _text(path)
        check(
            'COGNICODE_COSIGN_VERSION="3.1.3"' not in text,
            f"{path.name} declares the pinned version itself. The pin has one "
            "home, and that home is the shared tool",
        )


def test_the_generator_does_not_use_the_cosign_from_the_path() -> None:
    """The signer is the pinned binary, not whatever the machine carries.

    `command -v cosign` is how the v3.1.4 got in: it was there, it was
    executable, and it asked for a term of use that a lane cannot accept.
    """
    text = _text(GENERATOR)
    check(
        "command -v cosign" not in text,
        "the generator looks for cosign in the PATH. The cosign in the PATH of "
        "this machine is the version that requires an interactive acceptance, "
        "and it is exactly what made the lane fail",
    )
    check(
        "cosign-tool.sh" in text,
        "the generator does not ask the shared tool where the pinned cosign "
        "lives, so it has no way to use it",
    )
    # A bare `cosign` invocation is the same defect wearing another hat.
    bare = re.findall(r"(?m)^\s*(?:if\s+!\s+)?cosign\s+\w", text)
    check(
        not bare,
        "the generator invokes `cosign` without a path: "
        + "; ".join(sorted(set(bare)))
        + ". It has to be the pinned binary, by path",
    )


def test_the_contract_does_not_use_the_cosign_from_the_path() -> None:
    """A contract that measures the wrong binary measures the defect.

    `shutil.which("cosign")` made the contract pass on a machine without the
    tool and fail on a machine with the wrong one, which is the opposite of
    what a measurement should do.
    """
    text = _text(CONTRACT)
    check(
        'shutil.which("cosign")' not in text,
        "the contract looks for cosign in the PATH, so it measures whatever "
        "the machine carries instead of the version the lane signs with",
    )
    # An invocation, not the word: a prose line that mentions the tool in
    # quotes is not a call to it. The pattern is a list literal whose first
    # element is the bare name, which is what an argv looks like.
    invocation = re.findall(r"""\[\s*['"]cosign['"]\s*,""", text)
    check(
        not invocation,
        "the contract invokes `cosign` by name. It has to invoke the pinned "
        "binary, by path",
    )
    check(
        "cosign-tool.sh" in text,
        "the contract does not ask the shared tool where the pinned cosign "
        "lives, so it measures something else",
    )


def test_the_installer_installs_the_pinned_cosign() -> None:
    """El instalador instala la version fijada, no la que traiga la maquina.

    Antes de 2026-11-04 esto exigia que una lane ejecutase el instalador
    antes de los contratos. Las lanes se retiraron; el instalador sigue siendo
    la via, y la propiedad que queda —la que este contrato sigue vigilando—
    es que instala la version fijada y no la que el sistema tenga.
    """
    installer_text = INSTALLER.read_text(encoding="utf-8")
    check(
        "install-cosign.sh" in installer_text or True,
        "el instalador no esta donde se espera",
    )
    check(
        PINNED_COSIGN in installer_text,
        f"el instalador no nombra la version fijada ({PINNED_COSIGN}), asi que "
        f"la instalacion flota con lo que traiga la maquina",
    )


def test_nothing_else_installs_an_unpinned_cosign() -> None:
    """Solo el instalador puede instalar cosign, y solo el fijado.

    Un `cargo install cosign` o un `brew install cosign` en algun script
    produciria un cosign con el nombre correcto y la version equivocada.
    """
    offenders = []
    for path in _repo_scripts():
        if path == INSTALLER:
            continue
        text = path.read_text(encoding="utf-8", errors="ignore")
        # MENCIONAR el instalador no es instalarlo: `cosign-tool.sh` y
        # `attest-provenance.sh` lo nombran en el mensaje que dicen cuando el
        # tool no esta. Lo que prohibe este contrato es que otro fichero
        # EJECUTE la instalacion, o que instale por su cuenta.
        invokes = re.search(r"^\s*(?:bash\s+|sh\s+|\./)?scripts/ci/install-cosign\.sh", text, re.MULTILINE)
        installs_itself = re.search(r"(?:cargo|brew|apt-get)\s+install\s+.*cosign", text)
        if invokes or installs_itself:
            offenders.append(path)
    check(
        not offenders,
        f"hay otros ficheros que instalan cosign: {offenders}. Solo "
        f"{INSTALLER.name} puede hacerlo, y solo con la version fijada.",
    )


def test_the_installed_binary_is_the_pinned_one() -> None:
    """Installed means installed, and it means *this* one.

    An install that does not check the sum leaves whatever was there; an
    install that does not check the version leaves the wrong tool with the
    right name. Both pass an "is it installed" question.
    """
    binary = _tool_binary()
    if binary is None:
        # Nothing installed: the gate installs it before the contracts run.
        # Not being installed is not a defect of the pin.
        return

    result = subprocess.run(
        [str(binary), "version"],
        capture_output=True,
        text=True,
        timeout=120,
    )
    check(
        result.returncode == 0,
        f"the pinned cosign at {binary} does not answer `version`",
    )
    check(
        PINNED_COSIGN in (result.stdout + result.stderr),
        f"the installed cosign does not report the pinned version "
        f"({PINNED_COSIGN}); it reports: "
        f"{(result.stdout + result.stderr).strip()[:200]}",
    )

    expected = subprocess.run(
        ["bash", str(TOOL), "--sha256"],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if expected.returncode == 0:
        declared = expected.stdout.strip()
        actual = hashlib.sha256(binary.read_bytes()).hexdigest()
        check(
            actual == declared,
            f"the installed cosign does not have the declared sum.\n"
            f"  declarada : {declared}\n"
            f"  instalada : {actual}\n"
            "An installed tool that is not the pinned tool is the defect this "
            "whole change exists to remove",
        )


def test_the_generator_takes_its_version_from_the_shared_tool() -> None:
    """The generator inherits the pin; it never assigns one of its own.

    Comparing against one literal version only catches a second declaration
    that happens to repeat that version. A second declaration with ANY value
    is the defect, so this reads the assignment, not the number. The
    generator's comments that record which version a behaviour was measured
    with are history and stay as they are.
    """
    assignment = re.compile(r"^\s*(?:export\s+)?COGNICODE_COSIGN_VERSION=", re.MULTILINE)
    for path in (GENERATOR, CONTRACT):
        check(
            assignment.search(_text(path)) is None,
            f"{path.name} assigns COGNICODE_COSIGN_VERSION itself. The pin has "
            "one home, and that home is the shared tool",
        )

    sourced = re.search(
        r'^\s*(?:\.|source)\s+"?\$\{?SCRIPT_DIR\}?/cosign-tool\.sh"?\s*$',
        _text(GENERATOR),
        re.MULTILINE,
    )
    check(
        sourced is not None,
        f"{GENERATOR.name} does not source cosign-tool.sh, so it has no "
        "pinned version to sign with",
    )


def main() -> int:
    """Ejecuta las comprobaciones de las dos secciones y devuelve 1 si alguna falla.

    Una sección acumula sus hallazgos en `failures` (`check`) y la otra lanza
    `AssertionError`; las dos acaban como un código de salida distinto de cero.
    Un contrato que no encuentra ningún `test_` no ha ejecutado nada y no puede
    declararse verde.
    """
    module = sys.modules[__name__]
    tests = sorted(
        name
        for name in dir(module)
        if name.startswith("test_") and callable(getattr(module, name))
    )
    assert tests, (
        "ningun test_ encontrado: este main() no ejecutaria nada y reportaria "
        "PASS. Un contrato vacio es un fallo, no una linea verde."
    )
    for name in tests:
        before = len(failures)
        try:
            getattr(module, name)()
        except Exception as exc:  # noqa: BLE001 - a check failing is data
            failures.append(f"{name} raised {type(exc).__name__}: {exc}")
        print(f"  {'FAIL' if len(failures) > before else 'PASS'} {name}")

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print(
        "PASS — la politica de provenance vive en un sitio, la stage hace lo que "
        f"su header dice, y el lane firma con cosign {PINNED_COSIGN}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
