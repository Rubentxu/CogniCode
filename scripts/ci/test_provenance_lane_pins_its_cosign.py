#!/usr/bin/env python3
"""El lane de provenance tiene que firmar con la version que dice, no con la que aparece.

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
    python3 scripts/ci/test_provenance_lane_pins_its_cosign.py
"""

from __future__ import annotations

import hashlib
import re
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import pipeline_authority as authority  # noqa: E402

REPO_ROOT = Path(__file__).resolve().parents[2]
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


def test_the_merge_authority_installs_the_pinned_cosign() -> None:
    """A merge cannot pass without the tool the contracts need.

    The contracts stage runs before anything can fail closed on it, so the
    tool has to be there already. An install that happens after the contracts
    would leave the gate measuring nothing.
    """
    body = "\n".join(authority.pipeline_steps(authority.MERGE_AUTHORITY))
    check(
        "install-cosign.sh" in body,
        f"{authority.MERGE_AUTHORITY} never installs the pinned cosign, so the "
        "contracts stage measures with whatever the machine carries —or skips "
        "the measurement entirely",
    )
    check(
        PINNED_COSIGN in body,
        f"{authority.MERGE_AUTHORITY} installs the cosign without naming the "
        f"pinned version ({PINNED_COSIGN}), so the install floats with the "
        "machine's package manager",
    )


def test_the_lane_that_signs_installs_the_pinned_cosign() -> None:
    """The release lane signs; the release lane needs the tool.

    The generation half lives in `release-candidate.pipeline.kts`. If that lane
    reaches for a cosign that is not there, the release ships without an
    attestation and says nothing about why.
    """
    hosts = authority.invoked_by("install-cosign.sh")
    check(
        "release-candidate.pipeline.kts" in hosts,
        "the lane that generates the attestation never installs the pinned "
        "cosign: "
        + ", ".join(f"{name} ({stage})" for name, stage in sorted(hosts.items()))
        + ". It would sign with whatever the machine carries, or not sign",
    )


def test_the_install_happens_before_the_contracts_run() -> None:
    """Order is part of the property.

    An install after the contracts is an install that arrives too late: the
    measurement already ran, and it ran with nothing.
    """
    path = REPO_ROOT / authority.MERGE_AUTHORITY
    bodies = authority.pipeline_steps(authority.MERGE_AUTHORITY)
    install_at = next(
        (index for index, body in enumerate(bodies) if "install-cosign.sh" in body),
        None,
    )
    contracts_at = next(
        (index for index, body in enumerate(bodies) if "run-all-contracts.sh" in body),
        None,
    )
    check(
        path.is_file(),
        f"the declared merge authority does not exist: {path}",
    )
    check(
        install_at is not None,
        f"{authority.MERGE_AUTHORITY} does not install the pinned cosign at all",
    )
    check(
        contracts_at is not None,
        f"{authority.MERGE_AUTHORITY} does not run the contracts, so there is "
        "nothing to order the install against",
    )
    if install_at is not None and contracts_at is not None:
        check(
            install_at < contracts_at,
            f"the pinned cosign is installed at step {install_at} and the "
            f"contracts run at step {contracts_at}: the measurement runs "
            "before the tool is there",
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


def main() -> int:
    for name, func in sorted(globals().items()):
        if name.startswith("test_") and callable(func):
            try:
                func()
            except Exception as exc:  # noqa: BLE001 - a check failing is data
                failures.append(f"{name} raised {type(exc).__name__}: {exc}")

    if failures:
        print(f"FAIL — {len(failures)} problem(s):")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print(
        f"PASS — el lane firma con cosign {PINNED_COSIGN}, el generador y el "
        "contrato lo piden por su nombre, y las lanes que lo necesitan lo "
        "instalan antes de usarlo"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
