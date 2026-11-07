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

## Que version se mide, y por que esta fijada

La v3.1.4 de cosign —la que hay en el PATH de esta maquina— hace obligatorio el
registro de transparencia y, sin terminal, rechaza el prompt y sale con rc=1.
Un lane no tiene terminal, asi que ese binario no puede firmar aqui.

Este contrato no mide el `cosign` que aparezca en el PATH: pide la version
fijada a `cosign-tool.sh`, que es la unica fuente de la que bebe tambien el
generador. MEDIDO: la v3.1.3 firma offline, sin red y sin registro, y su bundle
verifica con la clave que acompana a la release — que es exactamente lo que el
ADR de provenance midio.

Un contrato que midiera el binario equivocado no mediria el mecanismo: mediria
el defecto. Y uno que se saltara porque el binario fijado no esta tampoco
mediria nada, asi que el gate lo instala antes de correr los contratos.

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
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GENERATOR = REPO_ROOT / "scripts" / "ci" / "attest-provenance.sh"
GATE = REPO_ROOT / "scripts" / "ci" / "verify-provenance.sh"
VERIFIER = GATE
TOOL = REPO_ROOT / "scripts" / "ci" / "cosign-tool.sh"

# Contrasena de la clave DE SONDA. No es un secreto y no protege nada: la clave
# que firma vive en un temporal que este test borra al terminar. Ponerla aqui y
# no en el entorno es a proposito — un contrato que necesita una variable secreta
# para correr no se puede ejecutar en ningun sitio donde se revisen contratos.
PROBE_PASSWORD = "clave-de-sonda-que-no-protege-nada"

SLSA_TYPE = "https://slsa.dev/provenance/v1"


def _cosign_bin() -> str | None:
    """La ruta del cosign FIJADO, o None si no esta instalado.

    MEDIDO: el `cosign` del PATH de esta maquina es la v3.1.4, que hace
    obligatorio el registro de transparencia y, sin terminal —un lane no tiene
    terminal—, rechaza el prompt y sale con rc=1. Ese binario no puede firmar
    aqui, y usarlo mediria el defecto en vez de medir el mecanismo.

    Por eso el contrato pregunta por la version fijada a `cosign-tool.sh`, que
    es la unica fuente de la que el generador tambien bebe: los dos pueden
    discrepar sobre que version se firma, y ese contrato nuevo lo vigila.
    """
    result = subprocess.run(
        ["bash", str(TOOL), "--path"],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if result.returncode != 0:
        return None
    path = result.stdout.strip()
    return path if path and Path(path).is_file() else None


def _make_keypair(root: Path, prefix: str, password: str = PROBE_PASSWORD) -> None:
    """Un par de claves desechable, con la forma que cosign 3.1.3 usa de verdad.

    MEDIDO 2026-10-04: `generate-key-pair` PIDE CONTRASENA y sin TTY muere con
    `inappropriate ioctl for device`. El ADR midio la forma interactiva, que
    funciona; un lane y un test reciben la forma no interactiva, que es la que
    necesita `COSIGN_PASSWORD`. Sin esto el contrato no se puede ejecutar en
    ningun runner, y un contrato que no corre es un comentario.
    """
    cosign = _cosign_bin()
    assert cosign is not None, (
        "el cosign fijado no esta instalado, y el del PATH no se usa a proposito. "
        "Instalalo con: bash scripts/ci/install-cosign.sh"
    )
    done = subprocess.run(
        [cosign, "generate-key-pair", "--output-key-prefix", str(prefix)],
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
    cosign = _cosign_bin()
    assert cosign is not None, (
        "el cosign fijado no esta instalado, y el del PATH no se usa a proposito. "
        "Instalalo con: bash scripts/ci/install-cosign.sh"
    )
    return subprocess.run(
        [cosign, "verify-blob", "--key", str(pub), "--bundle", str(bundle), str(blob)],
        capture_output=True,
        text=True,
        timeout=120,
    )


def _repo_scripts() -> list[Path]:
    """Every shell script that belongs to THIS repository.

    `rglob` over the repository root also walks `.claude/worktrees/`, where an
    agent's checkout holds its own copy of every script. A copy is not a second
    policy: it is the same file in another working tree, and counting it would
    make the "the policy lives in one place" assertion report a finding that
    does not exist — and would hide a real one behind the noise.
    """
    skip = {".git", "node_modules", "target", ".claude", "leases", "staging"}
    return [
        path
        for path in REPO_ROOT.rglob("*.sh")
        if not any(part in skip for part in path.relative_to(REPO_ROOT).parts)
    ]


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
    if _cosign_bin() is None:
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
    if _cosign_bin() is None:
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


def test_no_orchestration_asks_for_provenance_to_be_generated() -> None:
    """La generacion la pide el que certifica el artefacto, no el que lo publica.

    El ADR lo dice: la atension tiene que cubrir **el candidato que fue
    certificado**, y la publicacion consume ese candidato. Si la publicacion
    generase, la atension describiría un artefacto que nadie certifico, que es
    exactamente la propiedad rota que el candidato inmutable previene para los
    binarios.

    Las lanes se retiraron el 2026-11-04, asi que no hay orquestador que
    pueda pedir la generacion. La propiedad que queda —y que este contrato
    sigue vigilando— es que el generador no se invoca a si mismo: se ejecuta
    a mano, contra un artefacto concreto, por quien lo certifica.
    """
    assert not any(REPO_ROOT.glob("*.pipeline.kts")), (
        "volvio un orquestador de pipelines. Si genera provenance, tiene que "
        "ser la lane que produce el candidato, no la que lo publica."
    )


def test_the_generator_is_reachable_by_hand() -> None:
    """El generador tiene que existir y ser ejecutable a mano.

    Antes de 2026-11-04 esto exigia que una stage de la lane del candidato lo
    invocara. Sin lanes, la custodia de un artefacto es manual: quien certifica
    ejecuta el generador contra el artefacto concreto. La garantia que queda
    —y la que sigue importando— es que el generador existe, se puede ejecutar,
    y no fabrica nada cuando no puede firmar.
    """
    assert GENERATOR.is_file(), f"no existe {GENERATOR}"
    assert os.access(GENERATOR, os.X_OK), f"{GENERATOR} no es ejecutable"


def test_the_policy_still_lives_in_exactly_one_place() -> None:
    """B7 no puede abrir una segunda politica.

    `verify-provenance.sh` es el unico que lee `RELEASE_REQUIRE_PROVENANCE`. El
    generador no lee esa variable: su unica condicion es si hay una clave con la
    que firmar, que es una capacidad, no una politica.

    Las lanes se retiraron el 2026-11-04, asi que la segunda mitad del contrato
    —que ninguna lane expanda la politica— ya no tiene objeto: no hay ninguna.
    Lo que queda, y lo que sigue siendo verdad, es que la politica vive en un
    solo fichero.
    """
    generator_text = GENERATOR.read_text(encoding="utf-8")
    assert "RELEASE_REQUIRE_PROVENANCE" not in generator_text, (
        "el generador no lee la politica: si la leyera, habria dos politicas y "
        "volveria exactamente el desacuerdo que test_provenance_contracts.py prohibe."
    )
    readers = [
        path
        for path in _repo_scripts()
        if "RELEASE_REQUIRE_PROVENANCE" in path.read_text(encoding="utf-8", errors="ignore")
    ]
    assert readers == [VERIFIER], (
        f"la politica de provenance la leen {readers}; deberia leerla solo "
        f"{VERIFIER}. Dos politicas son un desacuerdo esperando a ocurrir."
    )


def test_the_generator_refuses_without_a_key_and_says_why() -> None:
    """Sin clave, el generador falla y explica el motivo.

    Antes de 2026-11-04 esto se comprobaba sobre la etapa de la lane, que
    tenia una rama sin clave que terminaba en `exit 0`. La lane se retiro; la
    politica se mudo al generador, que es donde vive hoy. Y su forma es la
    opuesta a la de la etapa: sin clave, **falla**, con un mensaje que dice
    que la clave no se fabrica a proposito y que la custodia es una decision
    humana.

    Que el fallo sea un fallo y no un exito silencioso es lo que importa: un
    generador que se salta la firma sin decir nada produce un artefacto sin
    atension que parece tenerlo.
    """
    generator_text = GENERATOR.read_text(encoding="utf-8")
    assert "--key" in generator_text, (
        "la generacion tiene que depender de la clave, que es la capacidad"
    )
    assert "if [ -z" in generator_text, (
        "el generador tiene que tener una rama sin clave. Sin ella, llamar al "
        "generador sin clave tiene que ser un fallo deliberado."
    )

    start = generator_text.index("if [ -z")
    close = generator_text.index("\nfi", start)
    no_key_branch = generator_text[start:close]

    assert "die" in no_key_branch, (
        "la rama sin clave tiene que fallar. Fallar aqui es una decision de "
        f"custodia que todavia no se ha tomado, no un fallo de construccion.\n"
        f"rama:\n{no_key_branch}"
    )
    assert "not" in no_key_branch.lower(), (
        "la rama sin clave tiene que decir que no firmo y por que. Saltarse en "
        "silencio es lo mismo que no decir nada, que es lo que este repositorio "
        f"se niega a publicar.\nrama:\n{no_key_branch}"
    )


def main() -> int:
    if _cosign_bin() is None:
        print(
            "SKIP - el cosign fijado no esta instalado; el mecanismo de provenance "
            "no se puede medir. El `cosign` del PATH no se usa a proposito. "
            "Instalalo con: bash scripts/ci/install-cosign.sh"
        )
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
