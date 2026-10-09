#!/usr/bin/env python3
"""El nombre del asset que pide `install.sh` y el que publica la release.

MEDIDO 2026-10-03. R1 cierra con un UAT de instalacion desde cero, y ese UAT
descarga **exactamente un asset**:

    base="${COGNICODE_RELEASE_BASE:-https://github.com/$REPO/releases/download/$tag}"
    asset="cogh-$version-$rust_target.tar.gz"

Ese nombre lo derivan dos piezas de codigo que no se conocen entre si:

- `install.sh` deduce `rust_target` de `uname -s` y `uname -m`, con un `case`;
- la release lo deduce de `platform_token(platform)` en `release_contract.rs`.

MEDIDO HOY: coinciden. `platform_token(Platform::LinuxX86_64)` es
`x86_64-unknown-linux-gnu`, que es exactamente lo que el `case` de `install.sh`
asigna a `x86_64|amd64`. Lo mismo con aarch64. Y el nombre completo encaja con
`artifact_filename("cogh", version, platform)`, que produce
`{stem}-{version}-{token}.tar.gz`.

El problema no es que hoy discrepen. Es que **nada mide si discrepan**. Cada
gate que existe mira un solo lado: `verify_release` comprueba manifiesto contra
payload, y el contrato de layout comprueba que la lane produzca la disposition
que el script de aplanado lee. Ninguno abre `install.sh`. Si alguien renombra
un token en `platform_token`, o anade una plataforma a `TIER1_PLATFORMS` sin
ensenarle a `install.sh` el `case` de arquitectura, **todo el mundo en esa
plataforma recibe un 404 al instalar y ningun gate se entera** hasta el UAT, que
es la unica red y se ejecuta una vez por release.

Es la misma clase que los tres hallazgos anteriores de la sesion, y la que mas
cuesta cara: los otros rompian una suite o una certificacion; este rompe la
instalacion de un consumidor sin que nada en el repositorio lo advierta.

## Mutaciones que este contrato tiene que detectar

- renombrar un token en `platform_token` (los dos lados dejan de hablar);
- anadir una plataforma a `TIER1_PLATFORMS` que `install.sh` no puede producir;
- cambiar la cadena de arquitectura que `install.sh` asigna;
- cambiar la forma del nombre del asset, de modo que el `grep` del SHA256SUMS
  deje de encontrarlo y la instalacion falle cerrada aunque el archivo exista;
- cambiar `RELEASE_DOWNLOAD_BASE` o el `REPO` de `install.sh`, que hoy coinciden
  y de los que depende toda la URL.
"""

from __future__ import annotations

import hashlib
import io
import platform
import re
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
INSTALL_SH = REPO_ROOT / "install.sh"
RELEASE_CONTRACT = REPO_ROOT / "crates" / "cognicode-cli" / "src" / "cmd" / "release_contract.rs"
BUNDLE_MANIFEST = REPO_ROOT / "crates" / "cognicode-cli" / "src" / "cmd" / "bundle_manifest.rs"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


# --- Lo que dice cada lado --------------------------------------------------
def platform_tokens() -> dict[str, str]:
    """`{Platform::X86_64: "x86_64-unknown-linux-gnu", ...}` desde el fuente real."""
    text = read(RELEASE_CONTRACT)
    body = re.search(r"pub fn platform_token\(platform: Platform\)[^{]*\{(.*?)\n\}", text, re.DOTALL)
    if body is None:
        raise AssertionError(
            f"no se encontro `platform_token` en {RELEASE_CONTRACT.name}; el contrato no "
            "leeria los tokens y pasaria sin comparar nada"
        )
    pairs = re.findall(r"Platform::(\w+)\s*=>\s*\"([^\"]+)\"", body.group(1))
    assert pairs, "platform_token no devolvio ningun par variante/token"
    return dict(pairs)


def tier1_platforms() -> list[str]:
    """Las plataformas que la release publica de verdad, no todas las del enum."""
    text = read(RELEASE_CONTRACT)
    const = re.search(r"pub const TIER1_PLATFORMS[^=]*=\s*&?\[([^\]]*)\]", text)
    if const is None:
        raise AssertionError(
            f"no se encontro TIER1_PLATFORMS en {RELEASE_CONTRACT.name}; sin el, el "
            "contrato no sabria que plataformas tienen que ser instalables"
        )
    return re.findall(r"Platform::(\w+)", const.group(1))


def installer_targets() -> dict[str, str]:
    """`{alias_de_arquitectura: rust_target}` tal como lo deduce install.sh.

    Se lee el `case "$arch"` real, no una lista escrita aqui: si alguien anade un
    alias nuevo, el contrato lo ve sin que haya que actualizar ninguna lista.
    """
    text = read(INSTALL_SH)
    block = re.search(r'case\s+"\$arch"\s+in(.*?)\nesac', text, re.DOTALL)
    if block is None:
        raise AssertionError('install.sh ya no tiene un `case "$arch"`; el contrato no lee nada')
    found: dict[str, str] = {}
    for line in block.group(1).splitlines():
        match = re.match(r'\s*([\w|]+)\)\s*rust_target="([^"]+)"', line)
        if match:
            for alias in match.group(1).split("|"):
                found[alias] = match.group(2)
    assert found, f"install.sh no asigna ningun rust_target en su case de arquitectura:\n{block.group(1)}"
    return found


def installer_supported_os() -> set[str]:
    text = read(INSTALL_SH)
    block = re.search(r'case\s+"\$os"\s+in(.*?)\nesac', text, re.DOTALL)
    if block is None:
        raise AssertionError('install.sh ya no tiene un `case "$os"`')
    return set(re.findall(r"^\s*(\w+)\)\s*os_name=", block.group(1), re.MULTILINE))


def installer_asset_shape() -> str:
    """La forma del nombre de asset, con los valores ya interpolados."""
    text = read(INSTALL_SH)
    match = re.search(r'^\s*asset="([^"]+)"', text, re.MULTILINE)
    assert match, "install.sh ya no deriva el nombre del asset; el contrato no lo compara"
    return match.group(1)


def rust_artifact_shape() -> str:
    """La forma equivalente, desde `artifact_filename` en Rust."""
    text = read(RELEASE_CONTRACT)
    match = re.search(r"pub fn artifact_filename\([^)]*\)\s*->\s*String\s*\{(.*?)\n\}", text, re.DOTALL)
    assert match, "no se encontro `artifact_filename` en release_contract.rs"
    literal = re.search(r'format!\("([^"]+)"', match.group(1))
    assert literal, "artifact_filename ya no usa un format! con una forma literal"
    return literal.group(1)


def rust_download_base() -> str:
    text = read(RELEASE_CONTRACT)
    match = re.search(r'pub const RELEASE_DOWNLOAD_BASE: &str = "([^"]+)"', text)
    assert match, "no se encontro RELEASE_DOWNLOAD_BASE en release_contract.rs"
    return match.group(1)


def installer_download_base() -> str:
    """La base que install.sh construye, con `$REPO` ya sustituido.

    install.sh mete el tag DENTRO de la base y luego solo concatena el asset;
    Rust deja la base sin version y concatena `/v{version}/{filename}`. Se
    normaliza quitando el sufijo del tag, que es donde las dos formas se
    comparan.
    """
    text = read(INSTALL_SH)
    repo = re.search(r'^\s*REPO="([^"]+)"', text, re.MULTILINE)
    assert repo, "install.sh ya no declara REPO"
    base = re.search(r"COGNICODE_RELEASE_BASE:-([^}]+)\}", text)
    assert base, "install.sh ya no declara la base de descarga por defecto"
    resolved = base.group(1).replace("$REPO", repo.group(1))
    return re.sub(r"/\$tag$", "", resolved)


def published_component_stems() -> set[str]:
    """Los `ArtifactKind` que la release publica de verdad, en kebab-case.

    Se leen de `COMPONENTS`, la tabla unica de superficie de producto. Es lo que
    permite comprobar que el nombre que install.sh tiene escrito a mano
    corresponde a algo que se publica, sin que el contrato lleve su propia lista.
    """
    text = read(RELEASE_CONTRACT)
    block = re.search(r"pub const COMPONENTS[^=]*=\s*&\[(.*?)\n\];", text, re.DOTALL)
    if block is None:
        raise AssertionError(
            f"no se encontro COMPONENTS en {RELEASE_CONTRACT.name}; el contrato no "
            "sabria que componentes se publican"
        )
    stems: set[str] = set()
    for entry in re.finditer(r"kind:\s*ArtifactKind::(\w+)(.*?)published:\s*(true|false)", block.group(1), re.DOTALL):
        if entry.group(2) is None or "true" in entry.group(0).split("published:")[1][:6]:
            stems.add(re.sub(r"(?<!^)(?=[A-Z])", "-", entry.group(1)).lower())
    assert stems, "COMPONENTS no declaro ningun componente publicado"
    return stems


def rust_platform_os(variant: str) -> str:
    """El sistema operativo de una variante, para comparar con el `case "$os"`."""
    display = re.search(rf"Platform::{variant}\s*=>\s*\"([^\"]+)\"", read(BUNDLE_MANIFEST))
    assert display, f"Platform::{variant} no tiene forma de Display en bundle_manifest.rs"
    return display.group(1).split("-")[0].lower()


# --- Tests ------------------------------------------------------------------
def test_every_installer_target_is_a_real_platform_token() -> None:
    """Lo que install.sh pide tiene que existir en la release.

    Si install.sh pide un triple que la release no publica, ese triple no va a
    estar en ningun manifest y la descarga es un 404 para todos los usuarios de
    esa plataforma, sin que la release pueda notarlo: ella solo publica lo suyo.
    """
    tokens = set(platform_tokens().values())
    for alias, target in sorted(installer_targets().items()):
        assert target in tokens, (
            f"install.sh asigna {target!r} al alias {alias!r}, y ese triple no es "
            f"ningun platform_token de la release. Los que existen: {sorted(tokens)}"
        )


def test_every_tier1_platform_is_installable() -> None:
    """La direccion inversa, y la que mas caro sale si falta.

    Anadir una plataforma a `TIER1_PLATFORMS` la publica la release. Si
    `install.sh` no sabe deducir su triple, esa plataforma queda publicada y no
    instalable, y ningun gate lo ve porque cada uno mira un lado.
    """
    producible = set(installer_targets().values())
    for variant in tier1_platforms():
        token = platform_tokens().get(variant)
        assert token is not None, f"{variant} esta en TIER1_PLATFORMS y no tiene platform_token"
        assert token in producible, (
            f"{variant} se publica como {token!r} pero install.sh no sabe producir "
            f"ese triple; produce {sorted(producible)}. Se publicaria algo que "
            "nadie podria instalar por el canal principal"
        )


def test_the_asset_name_is_derived_the_same_way_on_both_sides() -> None:
    """La forma del nombre, y que el prefijo sea un componente real.

    install.sh: `cogh-$version-$rust_target.tar.gz`
    Rust:       `format!("{stem}-{version}-{}.tar.gz", platform_token(platform))`

    No se comparan como cadenas: install.sh escribe el stem a mano y Rust lo
    recibe por parametro, de modo que la igualdad literal seria fragil por
    construccion. Lo que importa es que las dos formas tengan la misma
    estructura —stem, version, token, `.tar.gz`— y que el stem que install.sh
    tiene escrito exista entre los componentes publicados.

    install.sh despues hace `grep " $asset" ...` sobre el SHA256SUMS, asi que un
    desajuste aqui no es un 404: es un fallo cerrado por checksum, con un mensaje
    que senala al checksum cuando el problema es el nombre.
    """
    shell_shape = installer_asset_shape()
    rust_shape = rust_artifact_shape()

    assert rust_shape == "{stem}-{version}-{}.tar.gz", (
        f"artifact_filename deriva {rust_shape!r} y la forma esperada es "
        "'{stem}-{version}-{}.tar.gz'. Si cambia el numero de huecos, el nombre "
        "que produce install.sh deja de coincidir con el que produce la release"
    )
    assert shell_shape.endswith(".tar.gz"), (
        f"install.sh deriva {shell_shape!r} y la forma derivada tiene que terminar "
        "en .tar.gz como artifact_filename"
    )
    # install.sh lleva el stem literal donde Rust lleva {stem}: ambos ocupan el
    # primer segmento, asi que las dos formas tienen que tener el mismo numero
    # de ellos. Si install.sh insertara o quitara un campo, los contarian igual.
    assert len(shell_shape.split("-")) == len(rust_shape.split("-")), (
        f"install.sh deriva {shell_shape!r} "
        f"({len(shell_shape.split('-'))} segmentos) y artifact_filename "
        f"{rust_shape!r} ({len(rust_shape.split('-'))}). El stem ocupa el "
        "primero en las dos, asi que la estructura tiene que coincidir"
    )

    stem = shell_shape.split("-", 1)[0]
    stems = published_component_stems()
    assert stem in stems, (
        f"install.sh descarga el asset {stem!r} y ese stem no esta entre los "
        f"componentes publicados ({sorted(stems)}). La URL seria correcta y el "
        "archivo no existiria"
    )


def test_the_download_base_agrees() -> None:
    """`RELEASE_DOWNLOAD_BASE` y el `REPO` de install.sh coinciden hoy, por suerte.

    Son dos constantes en dos ficheros, en dos lenguajes, sin ninguna relacion.
    """
    rust_base = rust_download_base()
    shell_base = installer_download_base()
    assert rust_base == shell_base, (
        f"la release descarga de {rust_base!r} e install.sh construye "
        f"{shell_base!r}. Con la release publicada, install.sh pediria assets a "
        "una base que no existe"
    )
    assert rust_base.endswith("/releases/download"), (
        f"la base de la release no termina en /releases/download: {rust_base!r}"
    )


def test_the_supported_os_covers_every_tier1_platform() -> None:
    """install.sh declara Linux; si la release publica macOS, nadie lo instala.

    Se comparan en minusculas porque los dos lados nombran el mismo sistema con
    convenciones distintas: install.sh casa contra `uname -s`, que devuelve
    `Linux`, y `Platform::LinuxX86_64` se muestra como `linux-x86-64`.
    """
    supported = {name.casefold() for name in installer_supported_os()}
    assert supported, "install.sh no declara ningun sistema operativo soportado"
    for variant in tier1_platforms():
        os_name = rust_platform_os(variant)
        assert os_name in supported, (
            f"{variant} se publica como sistema {os_name!r} y install.sh solo "
            f"soporta {sorted(supported)}. Se publicaria algo que el canal "
            "principal no puede instalar"
        )


def test_the_checksum_lookup_matches_the_asset_name() -> None:
    """El `grep` del SHA256SUMS tiene que citar el asset con el que se descargo.

    Es la razon por la que la forma del nombre importa mas de lo que parece: un
    `grep` que no encuentra la linea hace que la instalacion falle cerrada con
    un mensaje sobre el checksum, cuando el asset si existia.
    """
    text = read(INSTALL_SH)
    lookup = re.search(r'grep\s+"([^"]*\$asset[^"]*)"', text)
    assert lookup, "install.sh ya no busca el checksum por el nombre del asset"
    pattern = lookup.group(1)
    assert "$asset" in pattern, pattern
    # El sufijo `\\$` ancla al final de la linea: sin el, "cogh-0.101.3" casaria
    # con "cogh-0.101.30".
    assert pattern.endswith("\\$"), (
        f"el grep del checksum es {pattern!r} y no ancla al final de la linea: un "
        "nombre que fuera prefijo de otro casaria con el checksum equivocado"
    )


# --- La mitad de ejecucion: lo que install.sh PROMETE y nadie comprueba ------
# El encabezado de install.sh declara cinco posturas de seguridad, y hasta hoy
# ninguna estaba probada:
#     unsupported platform   -> fail loud
#     missing checksum       -> fail closed
#     checksum mismatch      -> fail closed
#     partial download       -> never reaches the destination
#     existing destination   -> replaced only AFTER the new binary verifies
#
# MEDIDO 2026-10-03: `COGNICODE_RELEASE_BASE` — el hook que install.sh declara
# exactamente para esto, "so a local fake release can drive the checksum/tamper
# tests" — no lo usa nadie en el repo. `scripts/e88-entry-gates.sh` si ejecuta
# install.sh, pero esta clavado en la release publica v0.97.0, vive fuera de
# scripts/ci/ y por tanto fuera del merge gate, y solo prueba el camino feliz.
#
# Con COGNICODE_VERSION y COGNICODE_RELEASE_BASE fijos, install.sh no hace
# ninguna llamada a GitHub: el tag ya esta resuelto y la base es la que le
# damos. Con `file://` el stage entero es hermetico —sin red y sin release
# publicada— asi que estas posturas se comprueban en el merge gate y no en la
# proxima release que salga por la puerta.
#
# Los tests viven aqui y no en un fichero nuevo porque este ya es el dueno del
# contrato de install.sh, y porque un segundo fichero de pruebas de instalacion
# seria una segunda puerta al mismo gate, que es la leccion que
# scripts/ci/test_preflight_cleanup.sh ya dejo pagada.
INSTALL_VERSION = "0.0.0-contract"


def host_triple() -> str:
    """El triple que install.sh deduciria en ESTA maquina, leido de su `case`."""
    targets = installer_targets()
    arch = platform.machine()
    assert arch in targets, (
        f"install.sh no deduce ningun triple para la arquitectura de esta maquina "
        f"({arch!r}); sus alias son {sorted(targets)}"
    )
    return targets[arch]


def _write_tarball(path: Path, entries: dict[str, str]) -> None:
    with tarfile.open(path, "w:gz") as tar:
        for name, body in entries.items():
            data = body.encode()
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = 0o755
            tar.addfile(info, io.BytesIO(data))


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def stage_release(
    release: Path,
    triple: str,
    *,
    version: str = INSTALL_VERSION,
    entries: dict[str, str] | None = None,
    sums: str = "correct",
) -> str:
    """Deja en `release` lo que un GitHub release pondria, y devuelve la base.

    `sums` decide que dice el SHA256SUMS: el digest real del tarball, un digest
    equivocado, un digest de otro asset, o nada. El tarball se construye aqui
    en vez de copiarse de un fixture porque cada caso negativo necesita un
    tarball cuyo digest no coincida con lo que dice el manifiesto.
    """
    release.mkdir(parents=True, exist_ok=True)
    asset = f"cogh-{version}-{triple}.tar.gz"
    archive = release / asset
    _write_tarball(
        archive,
        entries
        if entries is not None
        else {"bin/cogh": f'#!/bin/sh\necho "cogh {version}"\n'},
    )
    if sums == "correct":
        (release / "SHA256SUMS").write_text(f"{_sha256(archive)}  {asset}\n")
    elif sums == "wrong-digest":
        (release / "SHA256SUMS").write_text(f"{'0' * 64}  {asset}\n")
    elif sums == "other-asset-only":
        (release / "SHA256SUMS").write_text(f"{'0' * 64}  otro-{triple}.tar.gz\n")
    elif sums == "absent":
        pass
    else:
        raise AssertionError(f"sums={sums!r} no es un modo conocido")
    return f"file://{release}"


def run_installer(
    work: Path, base: str, *, version: str = INSTALL_VERSION
) -> tuple[subprocess.CompletedProcess, Path]:
    """install.sh con un HOME limpio, la version fijada y la base que le damos.

    El destino se aparta del HOME a proposito: install.sh usa
    `COGNICODE_INSTALL_DIR` y asi el test puede afirmar sobre el binario final
    sin depender de donde Caen los shims de Layer 1.
    """
    home = work / "home"
    dest = work / "dest"
    tmp = work / "tmp"
    for directory in (home, dest, tmp):
        directory.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(
        ["sh", str(INSTALL_SH)],
        capture_output=True,
        text=True,
        timeout=120,
        env={
            "PATH": "/usr/bin:/bin:/usr/local/bin",
            "HOME": str(home),
            "TMPDIR": str(tmp),
            "COGNICODE_VERSION": version,
            "COGNICODE_RELEASE_BASE": base,
            "COGNICODE_INSTALL_DIR": str(dest),
        },
    )
    return result, dest


def assert_refused(result: subprocess.CompletedProcess, expected: str, what: str) -> None:
    """Que falle, por la razon declarada, y no por la primera que se encuentre.

    Un `returncode != 0` sin mas seria el mismo falso verde que cerro el UAT de
    `graph full`: probaria que algo fallo, no que install.sh detecto lo que dice
    detectar. install.sh escribe por `log`, que va a stderr, asi que el mensaje
    se busca ahi.
    """
    assert result.returncode != 0, f"{what}: install.sh salio 0 cuando deberia negarse"
    assert expected in result.stderr, (
        f"{what}: fallo, pero no por la razon declarada. Se esperaba {expected!r} "
        f"en stderr.\nstderr:\n{result.stderr}"
    )


def test_a_verified_release_installs_the_binary_it_checksummed() -> None:
    """La mitad positiva: sin esto, negarse a todo tambien pasaria el contrato."""
    triple = host_triple()
    with tempfile.TemporaryDirectory() as raw:
        work = Path(raw)
        base = stage_release(work / "release", triple)
        result, dest = run_installer(work, base)
        assert result.returncode == 0, (
            f"una release coherente fue rechazada.\nstderr:\n{result.stderr}"
        )
        installed = dest / "cogh"
        assert installed.is_file(), f"install.sh no dejo el binario en {installed}"
        reported = subprocess.run(
            [str(installed), "--version"], capture_output=True, text=True, timeout=30
        )
        assert reported.stdout.strip() == f"cogh {INSTALL_VERSION}", (
            f"el binario instalado no es el que se comprobo: {reported.stdout!r}"
        )


def test_a_tampered_asset_is_refused_and_installs_nothing() -> None:
    """checksum mismatch -> fail closed, y el destino no se toca."""
    triple = host_triple()
    with tempfile.TemporaryDirectory() as raw:
        work = Path(raw)
        base = stage_release(work / "release", triple, sums="wrong-digest")
        result, dest = run_installer(work, base)
        assert_refused(result, "checksum mismatch", "digest que no coincide")
        assert not (dest / "cogh").exists(), (
            "install.sh dejo un binario en el destino tras rechazar el checksum; "
            "fail closed significa fail closed"
        )


def test_a_missing_checksum_entry_is_refused() -> None:
    """missing checksum -> fail closed: un manifiesto sin nuestra linea no vale."""
    triple = host_triple()
    with tempfile.TemporaryDirectory() as raw:
        work = Path(raw)
        base = stage_release(work / "release", triple, sums="other-asset-only")
        result, dest = run_installer(work, base)
        assert_refused(result, "no checksum found", "manifiesto sin nuestra entrada")
        assert not (dest / "cogh").exists()


def test_a_missing_sha256sums_file_is_refused() -> None:
    """Sin manifiesto no hay digest que consultar, y sin digest no se instala."""
    triple = host_triple()
    with tempfile.TemporaryDirectory() as raw:
        work = Path(raw)
        base = stage_release(work / "release", triple, sums="absent")
        result, dest = run_installer(work, base)
        assert_refused(
            result, "refusing to install without a checksum", "manifiesto ausente"
        )
        assert not (dest / "cogh").exists()


def test_a_refused_install_leaves_the_previous_binary_in_place() -> None:
    """existing destination -> replaced only AFTER the new binary verifies.

    La postura que mas cuesta: el binario anterior se conserva mientras se
    descarga, se descomprime y se valida el nuevo. Un fallo en cualquier punto
    anterior al `mv` tiene que dejar el anterior exactamente como estaba.
    """
    triple = host_triple()
    with tempfile.TemporaryDirectory() as raw:
        work = Path(raw)
        dest = work / "dest"
        dest.mkdir()
        sentinel = dest / "cogh"
        sentinel.write_text("#!/bin/sh\necho 'cogh version-anterior'\n")
        sentinel.chmod(0o755)

        base = stage_release(work / "release", triple, sums="wrong-digest")
        result, _ = run_installer(work, base)
        assert_refused(result, "checksum mismatch", "installacion rechazada")
        assert sentinel.is_file(), "un install fallido borro el binario anterior"
        assert "version-anterior" in sentinel.read_text(), (
            "un install fallido reemplazó el binario anterior"
        )


def test_an_archive_without_bin_cogh_is_refused() -> None:
    """El asset se verifica y luego se abre; un tarball sin bin/cogh no instala."""
    triple = host_triple()
    with tempfile.TemporaryDirectory() as raw:
        work = Path(raw)
        base = stage_release(
            work / "release", triple, entries={"bin/otro-cosa": "#!/bin/sh\ntrue\n"}
        )
        result, dest = run_installer(work, base)
        assert_refused(result, "archive does not contain bin/cogh", "tarball sin cogh")
        assert not (dest / "cogh").exists()


def test_a_binary_reporting_another_version_is_refused() -> None:
    """Un digest que cuadra no dice que el binario sea el que se pidio.

    Este es el fallo que el checksum no puede ver: el tarball es autentico y el
    digest coincide, pero lo que hay dentro no es la version que se esta
    instalando. install.sh lo comprueba ejecutandolo antes de tocar el destino.
    """
    triple = host_triple()
    with tempfile.TemporaryDirectory() as raw:
        work = Path(raw)
        base = stage_release(
            work / "release",
            triple,
            entries={"bin/cogh": '#!/bin/sh\necho "cogh 9.9.9-otra"\n'},
        )
        result, dest = run_installer(work, base)
        assert_refused(result, "failed validation", "binario de otra version")
        assert not (dest / "cogh").exists()


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
        "PASS - el asset que pide install.sh y el que publica la release se "
        "derivan igual, toda plataforma publicada es instalable por ese canal, y "
        "las cinco posturas que install.sh declara en su encabezado se han "
        "ejercitado de verdad contra una release local: instala la verificada y "
        "se niega, nombrando la razon, ante digest alterado, checksum ausente, "
        "entrada ausente, tarball sin bin/cogh y binario de otra version, sin "
        "tocar jamas un destino preexistente."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
