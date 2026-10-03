#!/usr/bin/env python3
"""La stage de provenance no puede ser mas estricta que la politica que documenta.

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
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GATE = REPO_ROOT / "scripts" / "ci" / "verify-provenance.sh"
PIPELINE = REPO_ROOT / "release.pipeline.kts"

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


def test_the_pipeline_calls_the_gate() -> None:
    text = PIPELINE.read_text(encoding="utf-8")
    assert "verify-provenance.sh" in text, (
        "release.pipeline.kts debe delegar la politica en el script; la stage ya "
        "no la implementa por su cuenta"
    )


def test_the_pipeline_has_no_unconditional_verification() -> None:
    """La forma exacta del defecto: verificar sin switch."""
    text = PIPELINE.read_text(encoding="utf-8")
    assert "gh attestation verify" not in text, (
        "release.pipeline.kts no debe verificar atestaciones directamente: esa "
        "era la stage que fallaba siempre y que el header describia como "
        "condicional"
    )


def test_the_policy_lives_in_exactly_one_place() -> None:
    """Dos etapas leyendo la variable es como empezo la discrepancia.

    Lo prohibido es *evaluar* la variable, no *documentarla*: el header debe
    poder decir que `RELEASE_REQUIRE_PROVENANCE=1` endurece el gate, porque eso
    es la politica que el script implementa. Lo que no puede volver a aparecer
    es una expansion de shell dentro de la lane.
    """
    text = PIPELINE.read_text(encoding="utf-8")
    assert "${RELEASE_REQUIRE_PROVENANCE" not in text, (
        "la politica debe vivir solo en verify-provenance.sh; si la lane vuelve "
        "a expandir la variable, hay dos fuentes de verdad"
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
        "PASS - la politica de provenance vive en un sitio, y la stage hace lo "
        "que su header dice en vez de lo contrario."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
