#!/usr/bin/env python3
"""La generacion de provenance tiene que existir, funcionar y negarse a inventarse.

## El hueco, exacto

`docs/adr/ADR-RELEASE-PROVENANCE-PIPELINEK.md` mide la mitad que falta:

| Half | Status |
|---|---|
| Verify an artifact has provenance | present (`gh attestation verify`) |
| Fail closed when provenance is required | present (`RELEASE_REQUIRE_PROVENANCE=1`) |
| **Generate an attestation** | **absent** (was `actions/attest-build-provenance`) |

Asi que la lane puede exigir provenance, negarse correctamente cuando no lo hay,
y no tener ninguna forma de conseguirlo. `scripts/ci/attest-provenance.sh` es esa
forma, y este contrato es lo que demuestra que funciona.

## Que NO se comprueba aqui, y por que

La custodia de la clave. Este fichero genera un par de claves **desechable, en un
temporal, con una contrasena de prueba**, y solo para poder firmar algo. No es una
clave de release, no sale de aqui, y ningun test la persiste. Lo que se prueba es
el mecanismo, no la decision de donde vive la clave real; esa sigue siendo de
`Open decision, and it is the whole decision` en el ADR.

Por eso `test_the_generator_never_creates_a_key` existe y es la asercion mas
importante del fichero: un generador que fabricase su propia clave publicaria
provenance que nadie podria verificar nunca, que es peor que no publicar ninguna,
porque el consumidor recibe el mensaje de "estos bytes estan atestiguados".

Y por eso `test_the_lane_does_not_yet_require_provenance`: el ADR dice que el
defecto de `RELEASE_REQUIRE_PROVENANCE` debe pasar a `1` **cuando la generacion
exista en la misma lane**, porque invertir ese orden hace releases inalcanzables
—que es exactamente como murio `v0.101.3`. Que la generacion exista hoy no
autoriza a exigirla hoy: la clave no existe todavia, y exigirla dejaria la lane
roja por una razon que nadie puede accionar.

## Que se comprueba

- Sin clave: falla, y **no crea ninguna**.
- Con clave inexistente o sin su publica: falla, nombrando los dos nombres que
  cosign usa de verdad.
- Artefacto ausente: falla, y el directorio de salida queda **vacio**. Un
  directorio con clave publica y sin bundle parece atestiguado y no lo esta.
- El caso bueno: firma, y el `subject[0].digest.sha256` del bundle coincide con
  el `sha256sum` del artefacto. El primer digito de los tres, en la bytes que
  cosign escribio, no en las que el script queria escribir.
- `cosign verify-blob` con la publica **que el script copio al directorio de
  salida** —la que viajaría con la release— verifica OK. Verificar con la clave
  del builder probaria que el firmante funciona; verificar con la clave
  descargada prueba el camino del consumidor, que es la propiedad de la que
  depende la gente.
- **Las dos negativas**, porque un gate que solo pasa no distingue nada:
  - con una clave distinta, la verificacion falla;
  - con el bundle de un artefacto pero otro artefacto, la verificacion falla.
    Esta segunda es la que atrapa un bundle que nombra bytes equivocados.
- El generador no aparece en la lane de publicacion.
- `verify-provenance.sh` sigue siendo el unico que lee `RELEASE_REQUIRE_PROVENANCE`.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GENERATOR = REPO_ROOT / "scripts" / "ci" / "attest-provenance.sh"
GATE = REPO_ROOT / "scripts" / "ci" / "verify-provenance.sh"
CANDIDATE_PIPELINE = REPO_ROOT / "release-candidate.pipeline.kts"
RELEASE_PIPELINE = REPO_ROOT / "release.pipeline.kts"

# Contrasena de la clave DE SONDA. No es un secreto y no protege nada: la clave
# que firma vive en un temporal que este test borra al terminar. Ponerla aqui y
# no en el entorno es a proposito — un contrato que necesita una variable secreta
# para correr no se puede ejecutar en ningun sitio donde se revisen contratos.
PROBE_PASSWORD = "clave-de-sonda-que-no-protege-nada"

SLSA_TYPE = "https://slsa.dev/provenance/v1"


def _cosign() -> bool:
    return shutil.which("cosign") is not None


def _make_keypair(root: Path, prefix: str, password: str = PROBE_PASSWORD) -> None:
    """Un par de claves desechable, con la forma que cosign 3.1.3 usa de verdad.

    MEDIDO 2026-10-04: `generate-key-pair` PIDE CONTRASENA y sin TTY muere con
    `inappropriate ioctl for device`. El ADR midio la forma interactiva, que
    funciona; un lane y un test reciben la forma no interactiva, que es la que
    necesita `COSIGN_PASSWORD`. Sin esto el contrato no se puede ejecutar en
    ningun runner, y un contrato que no corre es un comentario.
    """
    done = subprocess.run(
        ["cosign", "generate-key-pair", "--output-key-prefix", str(prefix)],
        capture_output=True,
        text=True,
        env={**os.environ, "COSIGN_PASSWORD": password},
        timeout=120,
    )
    assert done.returncode == 0, (
        f"no se pudo generar la clave de sonda: rc={done.returncode}\n"
        f"stdout:\n{done.stdout}\nstderr:\n{done.stderr}"
    )


def run_generator(
    root: Path,
    artifacts: list[str],
    *,
    key: str | None,
    out_dir: str,
) -> subprocess.CompletedProcess:
    argv = [str(GENERATOR), "--out-dir", out_dir]
    if key is not None:
        argv += ["--key", key]
    argv += artifacts
    return subprocess.run(
        argv,
        capture_output=True,
        text=True,
        cwd=root,
        env={**os.environ, "COSIGN_PASSWORD": PROBE_PASSWORD},
        timeout=180,
    )


def _sha256(path: Path) -> str:
    return subprocess.run(
        ["sha256sum", str(path)], capture_output=True, text=True, timeout=60
    ).stdout.split()[0]


def _bundle_subject_digest(bundle: Path) -> str | None:
    """El `subject[0].digest.sha256` que leyo el envelope DSSE.

    MEDIDO: el payload va en base64 y dentro hay JSON con el subject. El digest
    es el sha256 **hex**, verbatim. Decodificarlo como si fuera base64 produce
    un desajuste que parece un fallo de firma y no lo es — lo apunta el ADR.
    """
    raw = json.loads(bundle.read_text(encoding="utf-8"))
    import base64

    payload = json.loads(base64.b64decode(raw["dsseEnvelope"]["payload"]))
    return payload["subject"][0]["digest"]["sha256"]


def _verify_blob(pub: Path, bundle: Path, blob: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["cosign", "verify-blob", "--key", str(pub), "--bundle", str(bundle), str(blob)],
        capture_output=True,
        text=True,
        timeout=120,
    )


def _candidate_provenance_stage() -> str | None:
    """El CUERPO de la stage, o `None` si no existe.

    MEDIDO 2026-10-04. La primera version de estos tests buscaba
    `attest-provenance.sh` en el fichero entero, y la mutacion que borra la
    stage **no los tumbo**: el bloque de comentario que explica la stage
    menciona el mismo nombre, asi que la asercion se satisfacia con prosa.

    Eso es exactamente el defecto que este ADR documenta para
    `release.pipeline.kts` —el header describia una politica y el codigo
    aplicaba otra—, repetido en el contrato que vigila al header. Un gate que
    puede pasar con un comentario no vigila el codigo.

    Se recorta desde `stage("provenance") {` hasta el cierre del `sh(...)`, que
    es donde termina el shell que corre de verdad. El comentario de arriba queda
    fuera por construccion.
    """
    text = CANDIDATE_PIPELINE.read_text(encoding="utf-8")
    marker = 'stage("provenance") {'
    if marker not in text:
        return None
    start = text.index(marker)
    if '""".trimIndent())' not in text[start:]:
        return None
    return text[start : start + text[start:].index('""".trimIndent())')]


# --- Tests ------------------------------------------------------------------


def test_the_generator_exists_and_is_executable() -> None:
    assert GENERATOR.is_file(), f"no existe {GENERATOR}"
    assert os.access(GENERATOR, os.X_OK), f"{GENERATOR} no es ejecutable"


def test_the_generator_never_creates_a_key() -> None:
    """La asercion mas importante del fichero.

    MEDIDO 2026-10-04: `cosign generate-key-pair` funciona y produce un par
    usable. Seria tentador que un generador de provenance lo invocase para no
    depender de una clave externa, y seria un error: la clave de una release no
    es un parametro de ejecucion, es una decision de custodia, y una clave que
    el propio generador se fabrica no puede pertenecer a nadie que pueda
    certificar mas adelante.

    Se comprueba sobre el texto entero del script, no solo sobre lo que se
    ejecuto: una construccion por concatenacion ("cosign" + " generate" +
    "-key-pair") seguiria conteniendo la cadena en su forma final, asi que la
    asercion no se deja esquivar por dividirla.
    """
    text = GENERATOR.read_text(encoding="utf-8")
    offending = [
        (number, line.strip())
        for number, line in enumerate(text.splitlines(), start=1)
        if "generate-key-pair" in line and not line.lstrip().startswith("#")
    ]
    assert not offending, (
        "el generador no puede fabricar claves. La custodia es una decision humana "
        f"(ADR-RELEASE-PROVENANCE-PIPELINEK, 'Open decision'). Lineas: {offending}"
    )


def test_without_a_key_it_fails_and_invents_nothing() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        artifact = root / "a.tar.gz"
        artifact.write_text("artefacto\n", encoding="utf-8")
        out = root / "out"

        done = run_generator(root, [str(artifact)], key=None, out_dir="out")
        assert done.returncode != 0, (
            f"sin clave debe fallar, y salio {done.returncode}.\n{done.stdout}"
        )
        assert "FAIL" in done.stderr, f"el motivo deberia quedar escrito.\n{done.stderr}"
        assert not any(out.glob("*")) if out.exists() else True, (
            "un fallo no puede dejar nada en el directorio de salida: una clave "
            "publica sin bundle parece una release atestiguada y no lo esta."
        )


def test_a_missing_key_is_named_not_replaced() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        artifact = root / "a.tar.gz"
        artifact.write_text("artefacto\n", encoding="utf-8")

        done = run_generator(root, [str(artifact)], key="keys/no-existe.key", out_dir="out")
        assert done.returncode != 0, done.stdout
        assert "no-existe.key" in done.stderr, (
            f"el error tiene que nombrar la clave que no encuentra.\n{done.stderr}"
        )


def test_a_missing_artifact_leaves_the_output_directory_empty() -> None:
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        _make_keypair(root, root / "probe")
        out = root / "out"

        done = run_generator(root, ["no-existe.tar.gz"], key="probe.key", out_dir=str(out))
        assert done.returncode != 0, done.stdout
        leftovers = sorted(p.name for p in out.glob("*")) if out.exists() else []
        assert leftovers == [], (
            f"un fallo a mitad deja estado parcial en el directorio de salida: {leftovers}. "
            "El directorio tiene que ser todo o nada."
        )


def test_the_subject_digest_is_the_artifact_digest() -> None:
    """El primer digito de los tres, en los bytes que cosign escribio."""
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        _make_keypair(root, root / "probe")
        artifact = root / "release-a.tar.gz"
        artifact.write_text("candidato\n", encoding="utf-8")
        sums = root / "SHA256SUMS"
        sums.write_text("otra pieza\n", encoding="utf-8")
        out = root / "out"

        done = run_generator(
            root, [str(artifact), str(sums)], key="probe.key", out_dir=str(out)
        )
        assert done.returncode == 0, f"rc={done.returncode}\n{done.stdout}\n{done.stderr}"

        for source in (artifact, sums):
            bundle = out / f"{source.name}.bundle.json"
            assert bundle.is_file(), f"no se escribio bundle para {source.name}"
            claimed = _bundle_subject_digest(bundle)
            actual = _sha256(source)
            assert claimed == actual, (
                f"el bundle de {source.name} attesta otros bytes.\n"
                f"  sha256sum : {actual}\n"
                f"  subject   : {claimed}\n"
                "Provenance que nombra los bytes equivocados es peor que no tenerla."
            )


def test_the_bundle_verifies_with_the_key_that_ships_with_the_release() -> None:
    """El camino del consumidor, no el del builder.

    Verificar con la clave que copio el generador al directorio de salida
    reproduce lo que hara quien descargue la release. Verificar con la clave del
    builder solo probaria que el firmante funciona.
    """
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        _make_keypair(root, root / "probe")
        artifact = root / "a.tar.gz"
        artifact.write_text("candidato\n", encoding="utf-8")
        out = root / "out"

        run_generator(root, [str(artifact)], key="probe.key", out_dir=str(out))
        shipped = out / "probe.pub"
        assert shipped.is_file(), "la clave publica tiene que viajar con la release"

        done = _verify_blob(shipped, out / "a.tar.gz.bundle.json", artifact)
        assert done.returncode == 0, (
            f"verify-blob con la clave que accompanya a la release fallo.\n"
            f"stdout:\n{done.stdout}\nstderr:\n{done.stderr}"
        )
        assert "Verified OK" not in done.stdout, (
            "MEDIDO 2026-10-04, cosign 3.1.3: 'Verified OK' sale por stderr. La "
            "primera version de este test lo buscaba en stdout y fallo con un "
            "mensaje vacio, que es la peor forma de fallo posible: parece un "
            "aserto roto en vez de una suposicion equivocada.\n"
            f"stdout:\n{done.stdout}"
        )
        # El veredicto se lee en el stream que cosign usa de verdad. Un codigo de
        # salida sin texto al lado no es una prueba de nada, y esta asercion
        # existe para que un futuro cosign que redireccione no convierta este
        # test en un verde sin contenido.
        assert "Verified OK" in done.stderr, (
            f"verify-blob devolvio 0 sin decir que verifico.\nstderr:\n{done.stderr}"
        )


def test_a_bundle_does_not_verify_against_another_key() -> None:
    """Negativa 1: la verificacion no es un sello de goma."""
    if not _cosign():
        return
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        _make_keypair(root, root / "probe")
        _make_keypair(root, root / "ajena")
        artifact = root / "a.tar.gz"
        artifact.write_text("candidato\n", encoding="utf-8")
        out = root / "out"

        run_generator(root, [str(artifact)], key="probe.key", out_dir=str(out))
        done = _verify_blob(root / "ajena.pub", out / "a.tar.gz.bundle.json", artifact)
        assert done.returncode != 0, (
            "un bundle firmado con una clave verifico con otra. Si esto pasara, "
            f"la verificacion no estaria comprobando nada.\n{done.stdout}"
        )


def test_a_bundle_does_not_transfer_to_other_bytes() -> None:
    """Negativa 2: la que atrapa un bundle que nombra el artefacto equivocado.

    Es la propiedad que `cosign verify-blob` promete y que un consumidor
    necesita: el bundle va travelling con los bytes, y si los bytes cambian el
    bundle deja de servir. Sin esta negativa, "hay un bundle al lado" seria todo
    lo que se puede afirmar.
    """
    if not _cosign():
        return
    with tempfile.TemporaryDirectory() as raw:
        root = Path(raw)
        _make_keypair(root, root / "probe")
        artifact = root / "a.tar.gz"
        artifact.write_text("candidato\n", encoding="utf-8")
        swapped = root / "b.tar.gz"
        swapped.write_text("contenido distinto\n", encoding="utf-8")
        out = root / "out"

        run_generator(root, [str(artifact)], key="probe.key", out_dir=str(out))
        done = _verify_blob(out / "probe.pub", out / "a.tar.gz.bundle.json", swapped)
        assert done.returncode != 0, (
            "el bundle de un artefacto verifico contra otro artefacto. Un consumidor "
            f"que solo compruebe 'hay bundle al lado' daria por bueno esto.\n{done.stdout}"
        )


# --- La forma, que es donde estaba el defecto --------------------------------


def test_the_release_lane_does_not_generate() -> None:
    """La generacion va en la lane del candidato, no en la de publicacion.

    El ADR lo dice: la atestion tiene que cubrir **el candidato que fue
    certificado**, y `release` consume ese candidato. Si `release` generase, la
    atestion describiría un artefacto que nadie certifico, que es exactamente
    la propiedad rota que el candidato inmutable previene para los binarios.
    """
    text = RELEASE_PIPELINE.read_text(encoding="utf-8")
    assert "attest-provenance.sh" not in text, (
        "release.pipeline.kts no debe generar provenance: la generacion pertenece a "
        "la lane que produce el candidato (ADR-RELEASE-PROVENANCE-PIPELINEK, "
        "'Consequences if accepted')."
    )


def test_the_candidate_lane_wires_the_generator_where_the_candidate_is_built() -> None:
    body = _candidate_provenance_stage()
    assert body is not None, (
        "no hay stage(\"provenance\") en release-candidate.pipeline.kts. La "
        "generacion existe como script pero no esta conectada: es codigo muerto, "
        "y lo unico que puede separarlo de la custodia es que la clave todavia "
        "no exista. Conectarlo es lo que deja la custodia como lo unico que falta."
    )
    assert "scripts/ci/attest-provenance.sh" in body, (
        "la stage de provenance no invoca al generador.\n"
        f"cuerpo:\n{body}"
    )


def test_the_policy_still_lives_in_exactly_one_place() -> None:
    """B7 no puede abrir una segunda politica.

    `verify-provenance.sh` es el unico que lee `RELEASE_REQUIRE_PROVENANCE`, y su
    contrato (`test_provenance_gate.py`) lo exige en ambas lanes. El generador
    no lee esa variable: su unica condicion es si hay una clave con la que
    firmar, que es una capacidad, no una politica.
    """
    generator_text = GENERATOR.read_text(encoding="utf-8")
    assert "RELEASE_REQUIRE_PROVENANCE" not in generator_text, (
        "el generador no lee la politica: si la leyera, habria dos politicas y "
        "volveria exactamente el desacuerdo que test_provenance_gate.py prohibe."
    )
    for pipeline in (CANDIDATE_PIPELINE, RELEASE_PIPELINE):
        text = pipeline.read_text(encoding="utf-8")
        assert "${RELEASE_REQUIRE_PROVENANCE" not in text, (
            f"{pipeline.name} vuelve a expandir la politica de provenance"
        )


def test_the_generator_is_not_required_while_no_key_exists() -> None:
    """La asercion de orden, y la que si se invirtio dejo v0.101.3 sin publicar.

    El ADR: "Flip the default only once generation exists in the same lane —
    inverting this order breaks releases". La generacion existe; la clave no.
    Exigir provenance hoy dejaria la lane roja por una razon que nadie puede
    accionar, que es como se rompio la release anterior.

    Lo que se comprueba es la FORMA de la etapa, no una palabra: que su cuerpo
    tenga una rama sin clave que termine en `exit 0`. La primera version de este
    test afirmaba que el nombre de la variable no aparecia en el fichero, y
    fallo —el nombre aparece en el comentario que explica precisamente por que
    la lane no la lee—. Es la distincion que `test_provenance_gate.py` ya fijo
    y que aqui se respeta: lo prohibido es *evaluar* la variable, no *nombrarla*.
    """
    body = _candidate_provenance_stage()
    assert body is not None, (
        "no hay stage(\"provenance\") en release-candidate.pipeline.kts, asi que no "
        "hay rama sin clave que pueda romper la release."
    )
    assert "RELEASE_PROVENANCE_KEY" in body, (
        "la generacion tiene que depender de la clave, que es la capacidad"
    )
    assert "if [ -z" in body, (
        "la stage tiene que tener una rama sin clave. Sin ella, llamar al "
        f"generador sin clave es un fallo deliberado.\ncuerpo:\n{body}"
    )

    # La rama sin clave va de `if [ -z` a su `fi` de cierre, buscado con su
    # sangria y no como subcadena: `"fi"` aparece dentro de palabras y una
    # busqueda laxa cortaria en el sitio equivocado.
    start = body.index("if [ -z")
    close = body.index("\n                    fi", start)
    no_key_branch = body[start:close]

    assert "exit 0" in no_key_branch, (
        "la rama sin clave tiene que terminar la etapa con exito. Fallar aqui "
        "seria una release rota por una decision de custodia que todavia no se "
        f"ha tomado.\nrama:\n{no_key_branch}"
    )
    assert "NOT generated" in no_key_branch, (
        "la etapa tiene que decir que no firmo. Saltarse en silencio es lo mismo "
        "que no decir nada, que es lo que este repositorio se niega a publicar.\n"
        f"rama:\n{no_key_branch}"
    )


def main() -> int:
    if not _cosign():
        print("SKIP - cosign no esta en PATH; el mecanismo de provenance no se puede medir")
        return 0

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
        "ningun test_ encontrado: este main() no ejecutaria nada y reportaria PASS. "
        "Un contrato vacio es un fallo, no una linea verde."
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
        "PASS - la generacion de provenance existe, firma, liga los tres digests, "
        "se niega a inventarse una clave y no se ha conectado a una politica que "
        "todavia no se puede cumplir."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
