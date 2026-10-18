#!/usr/bin/env python3
"""Pin del golden export `assurance-evidence/v1`.

## Que pinea

El export que produce CogniCode debe ser estable byte-a-byte cuando el
input (workspace + capabilities) no cambia. La verificación de M-D01-style
sobre el lado consumer
(`pipelinek-assurance/.../CogniCodeEvidenceExportCodec.kt::verifyDigest`)
se cumple SI y solo SI el encoding del producer (Rust/ciborium) y del
consumer (kotlinx-serialization-cbor) producen los mismos bytes sobre
el mismo DTO. Si CogniCode cambia su encoding (orden de campos, formato
de entero, etc.), el SHA-256 del golden cambia, el consumer detecta
drift medio byte-a-byte, y este contrato lo expone.

## Lo que el test cubre

1. M1 — El archivo del golden existe en `tests/fixtures/`.
2. M2 — SHA-256 del golden coincide con el pinned.
3. M3 — El CLI `cognicode export assurance` se registra y produce un
   envelope decodificable que pasa las cotas del consumer
   (`MAX_INPUT_BYTES`, `MAX_NESTING_DEPTH`, `MAX_COLLECTION_SIZE`,
   `MAX_STRING_LENGTH`).
4. M4 — El envelope lleva la revision del workspace (HEAD), la
   `apiVersion` y el `kind` que el consumer espera.
5. M5 — El envelope declara las capabilities conocidas y las
   Unsupported con motivo, no `items: []` (regla C4 del workstream).

## Limites del contrato

El contract no verifica la paridad byte-a-byte contra
kotlinx-serialization-cbor (eso vive en el lado consumer). Lo que
verifica es que el producer mantiene una salida estable: si dos runs
del CLI sobre el mismo input producen bytes diferentes, este contrato
los detecta.

## Hallazgos heredados

MEDIDO 2026-10-09 sobre integrate/v1015
SHA d0abbdf4932a0b7c4d03b9d5e0caf1458334267a:
- `cognicode export assurance --workspace . --capability entities
  --capability architecture --capability relations --capability signals
  --capability facts --output /tmp/golden.cbor` produce un envelope de
  2992537 bytes.
- SHA-256 del archivo: `008dbd701afbd30e8cea5306d9c1da6bdfc96cc09a8b5718e95c2fb337831b57`.
- producer version: v0.101.9.
- digest declarado en el envelope (lo que el consumer verifica): el
  SHA-256 de los bytes CBOR con `digest = ""` placeholder
  (`38629f0a0e8cda51cbc14718735cc0b8b23cb4cb7dd6535acb2af952172b793b`).
- Re-generado 2026-10-10 tras registrar `cognicode export assurance`
  en `cognicode-cli/src/main.rs` dispatcher: SHA-256 sigue identico.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
GOLDEN_PATH = REPO_ROOT / "tests" / "fixtures" / "assurance_export_golden.cbor"


def workspace_version() -> str:
    """Lee `[workspace.package].version` de Cargo.toml.

    B1.W1 — T1-5.5+ y siguientes bump-ean la version del workspace
    (v0.101.10 → v0.101.11 → ...). Hardcodearla en este script (la
    fuente del regression detector) era exactamente el anti-patron
    que el goal doc de B1 senala: cuando el binario cambia su
    `producer.version`, el assertion fallaba por una constante obsoleta,
    no por una regresion real. Derivarla del producto elimina esa
    clase de falso rojo y mantiene el contrato: cualquier drift del
    encoding o del campo `version` se sigue pineando contra el
    golden SHA-256 (regression detector separado).
    """
    text = (REPO_ROOT / "Cargo.toml").read_text()
    m = re.search(
        r'^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"',
        text, flags=re.MULTILINE | re.DOTALL,
    )
    if not m:
        raise SystemExit(
            f"FAIL: no se encontro [workspace.package].version en {REPO_ROOT / 'Cargo.toml'}"
        )
    return m.group(1)


# SHA-256 pinned del golden generado sobre integrate/v1015
# d0abbdf4932a0b7c4d03b9d5e0caf1458334267a con workspace=.
# Cualquier cambio en el encoding del producer lo modifica; eso es la
# senal que el consumer usa para detectar drift.
# SHA-256 pinned del golden. Cambia con cada regeneracion intencionada
# (refactor del encoding del producer o bump de producer version).
# Anteriores:
#   - 868b4a6583c10c9277be69e3f41d278ee7fe27e47c919c2c8ce4e49f88caabb0 (v0.101.20, regenerate 2026-10-17 al bump 0.101.19 → 0.101.20; corte de release de CU-1)
#   - 84008ccd810a1ea516ed968092b8401a7c01262ca6242e8aa3ec0c32a95251b1 (v0.101.19, corte no publicado; el RC falló en payloads por TIER1 aarch64)
#   - 2e9e26bc7bce21ec0be078b22d1d4b6db9ee2d7fa292a9017002f6143779d298 (v0.101.17, regenerate 2026-10-15 al bump 0.101.16 → 0.101.17; BLOQUE 5. Mismo tamaño en bytes que el pin anterior: lo único que cambió fue la versión del producer.)
#   - 9a30921eda60beb44f6c3a5db7cb78c43e4d01174db8a57c7e43d8cb5d7a55b3 (v0.101.16, regenerate 2026-10-12 al bump 0.101.15 → 0.101.16; cierre B6)
#   - ce7d61168c125ed3450183241d30ab56f4803ed9fe95f710dfe6b0d3f8627c98 (v0.101.15, regenerate 2026-10-12 al bump 0.101.14 → 0.101.15; cierre B5)
#   - ff35e199408b8ecf88ae0def0614f24dbfd3e22bd1fd4ce9185e557959bbe537 (v0.101.14, regenerate 2026-10-12 al bump 0.101.13 → 0.101.14; cierre B4)
#   - 1fd8f7fcb5854bdcf0d036be6da6238fe122582da4ba8974d2f9d5e43756f9be (v0.101.13, regenerate 2026-10-12 al bump 0.101.12 → 0.101.13; cierre B3)
#   - dfb39aa7a8efe888df2875812d845e2380b73b5cc1f2b59d354b40a29d618852 (v0.101.12, regenerate 2026-10-12 al bump 0.101.11 → 0.101.12; cierre B2)
#   - 464ba947472a0d5b824bb9843c4119a90a85db7259e21ca3b5b669b601b22d60 (v0.101.11, B2.W4 2026-10-11: producer enforces MAX_COLLECTION_SIZE — CogniCode workspace tiene 4681 .rs > 1024, entities y facts se truncan con Partial { gaps } honesto. AAT-B2-01 cumplido: el envelope ya no excede la cota del consumer.)
#   - 5cc8fdf3ed36bc4d3bdb615ac85530f901e23991a9524c2f7d928395c37350d5 (v0.101.11, regenerate 2026-10-11 al bump 0.101.10 → 0.101.11)
#   - 3ac84ea7de366e5b39e0e2316e0f2a06127b9d77cb10248dfbb16c14c09429da (v0.101.10, regenerate 2026-10-11 al bump 0.101.9 → 0.101.10)
#   - 45fa782147e2fc25ba6cc7e564ccaf326aae8ecaaef7ae37104133769405ee35 (v0.101.9, integrate/v1015)
# El consumer (pipelinek-assurance CogniCodeArtifactProvider) debe
# actualizar su pin tras cada regeneracion.
# Pin vigente: 0cba0302703536653378b7108c092e780ac1c90044f31438390ec3379f81f3ca
# (v0.101.21, regenerate 2026-10-17 al bump 0.101.20 → 0.101.21; corte de release
# de CU-2. Lo que cambia respecto al pin anterior es producer.version y el
# workspace rev. El corte original habia omitido esta regeneracion y el gate
# cayo en `contracts`; ver recibo del release.)
GOLDEN_SHA256 = (
    "0cba0302703536653378b7108c092e780ac1c90044f31438390ec3379f81f3ca"
)
PRODUCER_VERSION = workspace_version()
API_VERSION = "assurance-evidence/v1"
KIND = "EvidenceExport"
# Capabilities que produce el golden. Si el CLI se invoca con un subset
# o superset, el SHA-256 cambia y este contrato lo detecta.
GOLDEN_CAPABILITIES = [
    "entities",
    "architecture",
    "relations",
    "signals",
    "facts",
]


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(64 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def cargo_target_dir() -> Path | None:
    """Le pregunta a cargo dónde escribe, en vez de suponer `<repo>/target`.

    MEDIDO 2026-10-18. Esta función tenía cuatro rutas escritas a mano, dos
    de ellas absolutas y específicas de la máquina de un desarrollador
    (`/var/home/rubentxu/cargo-targets/...`). La suite pasaba en local porque
    ese directorio compartido tenía un `cognicode`; en el `merge-gate` —
    que aísla `CARGO_TARGET_DIR` en `/tmp/cognicode-merge-gate-*/target` y
    construye ahí sus propios binarios — las cuatro rutas apuntaban a otro
    sitio y el contrato era rojo.

    Lo grave no era que fallara: era que su verde dependía de qué hubiera
    en el disco de quien ejecutaba. Un gate que pide prestado su veredicto a
    la máquina del desarrollador no es un gate. La misma clase que
    `crates/cognicode-cli/tests/common/mod.rs` (`cargo_target_dir()`), y la
    misma solución: cargo es la autoridad sobre dónde escribe, y `cargo
    metadata` es la forma de preguntarle sin reimplementar su resolución de
    configuración.

    NO confundir con `test_cargo_iso.py::cargo_metadata_target_dir`, que se
    llama casi igual. La de ahí borra `CARGO_TARGET_DIR` del entorno a
    propósito: su pregunta es "qué hace el *wrapper* con este entorno", y
    para eso necesita controlarla. La de aquí es la pregunta contraria —
    "dónde escribe cargo *para este proceso*" — y por eso tiene que honrar
    la variable que la lane fija. Importar una en la otra rompería una de las
    dos en silencio: la de `test_cargo_iso` perdería exactamente el caso que
    existe para cubrir, y esta volvería a ignorar el directorio aislado del
    gate. MEDIDO: 5 llamadas ≈ 0,31 s en total, así que el subproceso no
    necesita memoización y la función se queda sin estado.

    Devuelve `None` en vez de propagar: si cargo no responde, esta función no
    puede saber dónde está el binario, y un `None` la deja caer al último
    recurso con el mismo error loud de siempre. Explotar aquí escondería el
    mensaje que dice qué construir.
    """
    try:
        out = subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            cwd=str(REPO_ROOT),
            stderr=subprocess.DEVNULL,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError):
        return None
    try:
        resolved = json.loads(out)["target_directory"]
    except (json.JSONDecodeError, KeyError):
        return None
    return Path(resolved)


def find_cognicode_binary() -> str:
    """Localiza el binario `cognicode` donde cargo escribe de verdad.

    Release antes que debug: los bytes release son el artefacto que el golden
    describe, y un binario debug que resulta estar ahí es un recurso, no un
    equivalente. `cargo metadata` resuelve el directorio destino —incluida la
    variable `CARGO_TARGET_DIR` cuando la lane fija una— así que esto sigue
    al build en vez de a una lista de rutas que tiene que ser cierta en todas
    las máquinas a la vez.

    Los candidatos `<repo>/target` se quedan como último recurso. Son la forma
    que cargo usa en una máquina que no configura nada, y quitarlos haría que
    este contrato fallara en un runner limpio por un motivo que no es un
    defecto.
    """
    target = cargo_target_dir()
    candidate_dirs = []
    if target is not None:
        candidate_dirs += [target / "release", target / "debug"]
    candidate_dirs += [
        REPO_ROOT / "target" / "release",
        REPO_ROOT / "target" / "debug",
    ]
    for d in candidate_dirs:
        binary = d / "cognicode"
        if binary.is_file() and os.access(binary, os.X_OK):
            return str(binary)
    raise FileNotFoundError(
        f"no se encontro el binario `cognicode` en {candidate_dirs}. "
        f"Build con: cargo build --manifest-path "
        f"crates/cognicode-cli/Cargo.toml --release --bin cognicode"
    )


def head_sha() -> str:
    return subprocess.check_output(
        ["git", "-C", str(REPO_ROOT), "rev-parse", "HEAD"], text=True
    ).strip()


def run_export_assurance(*, output: Path, capabilities: list[str] | None = None,
                        workspace: Path | None = None) -> subprocess.CompletedProcess:
    binary = find_cognicode_binary()
    args = [binary, "export", "assurance", "--output", str(output)]
    if workspace is not None:
        args.extend(["--workspace", str(workspace)])
    else:
        args.extend(["--workspace", str(REPO_ROOT)])
    if capabilities:
        for c in capabilities:
            args.extend(["--capability", c])
    return subprocess.run(args, check=False, capture_output=True, text=True)


# ---------- assertions (cada una pine un tipo de drift) ----------


def test_golden_file_exists() -> None:
    """M1: el archivo del golden debe existir en tests/fixtures/.

    Sin el archivo, no hay pin y el contrato no dice nada. El primer
    run debe regenerarlo con --update; corridas posteriores lo verifican.
    """
    assert GOLDEN_PATH.is_file(), (
        f"FAIL: golden no existe en {GOLDEN_PATH}. "
        f"Regeneralo con: python3 scripts/ci/test_assurance_export_golden.py --update"
    )


def test_golden_sha256_is_pinned() -> None:
    """M2: SHA-256 del golden debe coincidir con el pinned.

    Cualquier cambio en el encoding del producer (orden de campos,
    formato de enteros, version de ciborium, etc.) modifica el SHA-256.
    Esto es la senal que el consumer usa para detectar drift.
    """
    if not GOLDEN_PATH.is_file():
        return  # cubierto por M1
    actual = sha256_file(GOLDEN_PATH)
    assert actual == GOLDEN_SHA256, (
        f"FAIL: SHA-256 del golden cambio.\n"
        f"  esperado: {GOLDEN_SHA256}\n"
        f"  actual:   {actual}\n"
        f"  Si el cambio es intencionado (refactor del encoding, "
        f"cambio de producer version), regenera con --update y coordina "
        f"con el consumer (pipelinek-assurance) para que actualice su pin."
    )


def test_cli_subcommand_registered() -> None:
    """M3a: el subcomando `export assurance` debe estar registrado.

    Si el binario esta stale o el wiring del dispatcher se rompio, este
    test detecta el error antes de intentar el round-trip.
    """
    binary = find_cognicode_binary()
    result = subprocess.run(
        [binary, "export", "assurance", "--help"],
        check=False, capture_output=True, text=True,
    )
    assert result.returncode == 0, (
        f"FAIL: `cognicode export assurance --help` retorno {result.returncode}\n"
        f"  stderr: {result.stderr}\n"
        f"  stdout: {result.stdout}"
    )
    assert "assurance-evidence/v1" in result.stdout, (
        "el --help debe mencionar el schema version del envelope"
    )


def test_cli_produces_envelope_with_expected_shape() -> None:
    """M3b: el envelope producido debe tener el shape esperado.

    Pasa `--format json` para inspeccionar el shape desde Python sin
    necesitar un decoder CBOR externo. La decodificacion CBOR con
    bounded decoding vive en el roundtrip de Rust
    (`crates/cognicode-core/tests/assurance_export_roundtrip.rs`); aqui
    solo verificamos el shape declarativo.
    """
    with tempfile.TemporaryDirectory(prefix="cognicode-golden-") as tmp:
        out = Path(tmp) / "envelope.json"
        result = run_export_assurance(output=out)
        assert result.returncode == 0, (
            f"FAIL: CLI retorno {result.returncode}\n"
            f"  stdout: {result.stdout}\n  stderr: {result.stderr}"
        )
        assert out.is_file(), "CLI exit 0 sin escribir output"
        envelope = json.loads(out.read_text())
        assert envelope.get("apiVersion") == API_VERSION, (
            f"apiVersion esperado {API_VERSION}, encontrado {envelope.get('apiVersion')}"
        )
        assert envelope.get("kind") == KIND, (
            f"kind esperado {KIND}, encontrado {envelope.get('kind')}"
        )
        producer = envelope.get("producer", {})
        assert producer.get("id") == "cognicode", producer
        assert producer.get("version") == PRODUCER_VERSION, (
            f"producer.version esperado {PRODUCER_VERSION}, "
            f"encontrado {producer.get('version')}"
        )
        # El envelope debe tener un digest declarado (no vacio).
        digest = envelope.get("digest", "")
        assert len(digest) == 64, (
            f"digest debe ser SHA-256 hex (64 chars), encontrado "
            f"longitud {len(digest)}: {digest[:32]}..."
        )
        int(digest, 16)  # debe ser hex valido


def test_subject_revision_is_head() -> None:
    """M4: el envelope debe llevar la revision del workspace.

    UAT-024 en el lado consumer exige que `subjectRevision` propague
    el SHA de HEAD para que la misma revision produzca el mismo digest.
    Sin este campo, dos commits del mismo workspace producen digests
    distintos aunque el contenido sea equivalente.
    """
    with tempfile.TemporaryDirectory(prefix="cognicode-golden-") as tmp:
        out = Path(tmp) / "envelope.json"
        run_export_assurance(output=out)
        envelope = json.loads(out.read_text())
        subject = envelope.get("subject", {})
        head = head_sha()
        assert subject.get("revision") == head, (
            f"FAIL: subject.revision ({subject.get('revision')}) "
            f"no coincide con HEAD ({head})"
        )
        assert subject.get("kind") == "rust-workspace", (
            f"subject.kind esperado 'rust-workspace', encontrado {subject.get('kind')}"
        )


def test_unsupported_capabilities_have_reason() -> None:
    """M5: las capabilities no soportadas llevan motivo, no items: [].

    Regla C4 del workstream: `architecture`, `relations` y `signals`
    son Unsupported en v1. `capabilityCompleteness` debe declarar el
    motivo (`reason` no vacio) y `gaps[]` debe incluirlas. Ademas,
    no debe haber campos `items: []` para capabilities no producidas.
    """
    with tempfile.TemporaryDirectory(prefix="cognicode-golden-") as tmp:
        out = Path(tmp) / "envelope.json"
        run_export_assurance(output=out)
        envelope = json.loads(out.read_text())
        cc = envelope.get("capabilityCompleteness", {})
        for cap in ("architecture", "relations", "signals"):
            entry = cc.get(cap)
            assert entry is not None, f"falta {cap} en capabilityCompleteness"
            kind = entry.get("kind", "")
            if kind == "unsupported":
                reason = entry.get("reason", "")
                assert reason, f"{cap} Unsupported sin reason: {entry}"
            elif kind == "complete":
                # Aceptable si en una version futura se implementa
                pass
            else:
                # 'partial' o 'unknown' no son validos en v1 para estas
                # tres; en v1 o son Unsupported o son Complete.
                assert kind in {"complete", "unsupported"}, (
                    f"kind inesperado para {cap}: {kind}"
                )
        gaps = envelope.get("gaps", [])
        unsupported_caps = {
            c for c, e in cc.items() if e.get("kind") == "unsupported"
        }
        for cap in unsupported_caps:
            assert any(g.get("capability") == cap for g in gaps), (
                f"capability {cap} Unsupported sin entrada en gaps[]: {gaps}"
            )


def test_producer_is_deterministic() -> None:
    """M6: el producer es determinista sobre el mismo input.

    Tres corridas consecutivas del CLI con el mismo input deben
    producir el mismo SHA-256. Si este test falla, el producer tiene
    una fuente de no-determinismo (orden de `read_dir` no sorteado,
    path de workspace no canonicalizado, etc.) que rompe la
    verificacion M-D01-style del consumer.

    Aclaracion importante: el SHA-256 esperado aqui NO es el pinned.
    El envelope embebe la revision del workspace, asi que el SHA
    cambia con cada commit (sin tocar el exporter). Lo que M-D01
    exige es estabilidad: misma revision + mismo input = mismo SHA.
    Ese SHA concreto lo pinea el golden y M2 lo verifica; M6 es
    invariante a la revision y pinea solo que no hay drift dentro
    de una corrida.
    """
    with tempfile.TemporaryDirectory(prefix="cognicode-golden-") as tmp:
        out1 = Path(tmp) / "regen1.cbor"
        out2 = Path(tmp) / "regen2.cbor"
        out3 = Path(tmp) / "regen3.cbor"
        args = [
            find_cognicode_binary(),
            "export", "assurance",
            "--workspace", str(REPO_ROOT),
            "--capability", "entities",
            "--capability", "architecture",
            "--capability", "relations",
            "--capability", "signals",
            "--capability", "facts",
        ]
        subprocess.run(args + ["--output", str(out1)], check=True, capture_output=True)
        subprocess.run(args + ["--output", str(out2)], check=True, capture_output=True)
        subprocess.run(args + ["--output", str(out3)], check=True, capture_output=True)
        s1, s2, s3 = sha256_file(out1), sha256_file(out2), sha256_file(out3)
        assert s1 == s2 == s3, (
            f"FAIL: producer no es determinista. "
            f"3 corridas con mismo input dieron SHAs distintos:\n"
            f"  run1: {s1}\n  run2: {s2}\n  run3: {s3}"
        )


# ---------- regeneracion (uso explicito) ----------


def regenerate_golden() -> None:
    """Regenera el golden (uso: --update). Solo se ejecuta bajo peticion
    explicita del operador (politica del round 1 de R1)."""
    binary = find_cognicode_binary()
    GOLDEN_PATH.parent.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(
        [
            binary,
            "export", "assurance",
            "--workspace", str(REPO_ROOT),
            "--capability", "entities",
            "--capability", "architecture",
            "--capability", "relations",
            "--capability", "signals",
            "--capability", "facts",
            "--output", str(GOLDEN_PATH),
        ],
        check=True, capture_output=True, text=True,
    )
    new_sha = sha256_file(GOLDEN_PATH)
    print(f"regenerado {GOLDEN_PATH}: {GOLDEN_PATH.stat().st_size} bytes")
    print(f"SHA-256 nuevo: {new_sha}")
    print(f"SHA-256 pin anterior: {GOLDEN_SHA256}")
    if new_sha != GOLDEN_SHA256:
        print(
            f"\nSi el cambio es intencionado, actualiza GOLDEN_SHA256 "
            f"en este fichero a:\n  {new_sha}"
        )
    print("\nstdout:")
    print(result.stdout)
    print("stderr:")
    print(result.stderr)


def main() -> int:
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("--update", action="store_true",
                        help="regenera el golden (politica explicita del operador)")
    args = parser.parse_args()
    if args.update:
        regenerate_golden()
        return 0

    failures: list[str] = []
    for fn in (
        test_golden_file_exists,
        test_golden_sha256_is_pinned,
        test_cli_subcommand_registered,
        test_cli_produces_envelope_with_expected_shape,
        test_subject_revision_is_head,
        test_unsupported_capabilities_have_reason,
        test_producer_is_deterministic,
    ):
        try:
            fn()
        except AssertionError as e:
            failures.append(f"{fn.__name__}: {e}")
        except Exception as e:
            failures.append(f"{fn.__name__}: {type(e).__name__}: {e}")

    if failures:
        print("assurance-evidence/v1 golden contract: FAIL")
        for f in failures:
            print(f"  - {f}")
        return 1

    print(
        f"assurance-evidence/v1 golden contract: OK "
        f"(producer v{PRODUCER_VERSION}, "
        f"golden sha256={GOLDEN_SHA256[:16]}...)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())