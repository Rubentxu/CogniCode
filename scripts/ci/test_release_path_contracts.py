#!/usr/bin/env python3
"""Contratos de la ruta de release: la guarda de toolchain cruzado y el acuerdo del instalador.

Tres contratos del mismo carril (el binario que sale de la release, el que
`install.sh` descarga y el conjunto de assets del release-candidate),
fusionados en uno (TASK-74, TASK-146) conservando todas las comprobaciones y
sus fallos cerrados.

# I. Un target instalado no es un target que compile

Un target instalado no es un target que compile.

MEDIDO 2026-10-03. La stage `toolchain-for-$target` de
`release-candidate.pipeline.kts` comprobaba una sola cosa —que el target
estuviera en `rustup target list --installed`— mientras su propio comentario
decia que un target cuyas binaries no pueden enlazar "produces a candidate that
looks built and is not". El comentario describia una garantia que la stage no
hacia.

La lane de `v0.101.4` loENSEÑO de la peor manera: la stage pasó, y
`binaries-aarch64` fallo veinte minutos despues con tres errores distintos y
ninguno de ellos diciendo "a este target le falta el compilador de C":

    error occurred in cc-rs: failed to find tool "aarch64-linux-gnu-g++"
        -> CXX_aarch64_unknown_linux_gnu no estaba definido
    error occurred in cc-rs: command did not execute successfully
        ... zig-cc-aarch64 ... "--target=aarch64-unknown-linux-gnu"
        -> unable to parse target query 'aarch64-unknown-linux-gnu':
           UnknownOperatingSystem

Los dos ultimos son la misma causa: `cc-rs` anade `--target=<triple de Rust>` y
un wrapper que solo hornea su propio `-target` queda **pisado** por ese
argumento. El wrapper tiene que traducir el triple, no solo lurking.

Es la misma forma que los otros dos hallazgos de esta sesion: una guarda cuya
condicion de disparo se cumple en el caso normal, y cuyo camino de error nadie
recorre porque nunca se ha ejecutado.

## Que se comprueba

Este contrato ejecuta `scripts/ci/check-cross-toolchain.sh` de verdad, con
compiladores de mentira en el PATH y en el entorno:

- **Target nativo**: no exige nada; el toolchain del build es el suyo.
- **Sin compilador**: falla, y el motivo dice que variable definir.
- **Compilador que existe pero rechaza el triple**: falla tambien. Este es el
  caso que la stage anterior no podia ver: un compilador instalado que no
  compila para el target se comporta, desde fuera, igual que uno que no existe.
- **Compilador que funciona**: pasa, y dice cual fue.
- Un compilador de C++ configurado que no funciona tambien es fallo, porque el
  build lo encontraria igual; pero la ausencia de C++ no se exige, porque no
  todos los toolchain lo traen y no todos los proyectos lo necesitan.

Y la stage del pipeline llama al script, que es lo que hace que la guarda exista
en el sitio donde se anuncia.

# II. El asset que pide `install.sh` y el que publica la release

El nombre del asset que pide `install.sh` y el que publica la release.

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

Run:
    python3 scripts/ci/test_release_path_contracts.py
"""

from __future__ import annotations

import hashlib
import io
import json
import os
import platform
import re
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
GUARD = REPO_ROOT / "scripts" / "ci" / "check-cross-toolchain.sh"

# Compilador de mentira que ACCEPTa el triple: escribe un objeto y sale 0.
CC_OK = """#!/usr/bin/env bash
set -euo pipefail
out=""
prev=""
for a in "$@"; do
  if [ "$prev" = "-o" ]; then out="$a"; fi
  prev="$a"
done
[ -n "$out" ] && : > "$out"
exit 0
"""

# Compilador de mentira que RECHAZA el triple, como hacia zig sin traduccion.
CC_REJECTS = """#!/usr/bin/env bash
set -euo pipefail
for a in "$@"; do
  case "$a" in
    --target=aarch64-unknown-linux-gnu)
      echo "error: unable to parse target query 'aarch64-unknown-linux-gnu': UnknownOperatingSystem" >&2
      exit 1 ;;
  esac
done
exit 0
"""

# Un `rustc` que no responde. Deja un rastro para que el test vea que se le
# pregunto, y sale sin decir que host es: es el caso de un toolchain roto, que
# antes se traducía en un veredicto sobre el target que no era sobre el host.
RUSTC_SILENT = """#!/usr/bin/env bash
: > "${RUSTC_MARKER:?the marker has to be reachable}"
exit 1
"""

# Un `rustc` que responde con el host y no instala nada.
RUSTC_HOSTS = """#!/usr/bin/env bash
echo "host: x86_64-unknown-linux-gnu"
"""


def host_target() -> str:
    """El triple del host, resuelto aqui y con el entorno real de este proceso.

    MEDIDO 2026-10-25. Resolverlo DENTRO de la guarda, con el HOME aislado que
    hace el aislamiento, hace que rustup intente instalar la toolchain desde la
    red: 117,7 s por llamada y `TimeoutExpired` a los 120 s. Resolverlo aqui,
    una vez, con el entorno real, cuesta 9 ms.

    Si `rustc -vV` no dice cual es el host, esto falla y lo dice. Adivinar un
    triple es la forma de que un veredicto de la guarda dependa de una suposicion
    en vez de de una medida.
    """
    out = subprocess.run(
        ["rustc", "-vV"], capture_output=True, text=True, timeout=60
    )
    for line in out.stdout.splitlines():
        if line.startswith("host:"):
            return line.split(":", 1)[1].strip()
    raise AssertionError(
        "'rustc -vV' no devolvio el triple del host:\n" + out.stdout + out.stderr
    )


def run_guard(
    target: str,
    *,
    host: str | None = None,
    resolve_host: bool = False,
    bin_dir: str | None = None,
    **env_extra: str,
) -> subprocess.CompletedProcess:
    """Ejecuta la guarda con un entorno controlado y devuelve su estado real.

    El host hay que darlo. La guarda lo resuelve sola con `rustc -vV`, y esa
    ruta, con el HOME aislado que hace este aislamiento, hace que rustup
    descargue la toolchain: 117,7 s por llamada, y `TimeoutExpired` a los 120 s.
    Para ejercitar esa ruta a proposito, `resolve_host=True`.

    `bin_dir` antepone un directorio al PATH. Lo usa el test que pone un `rustc`
    de mentira: sin el, la guarda usaria el rustc de verdad, que no es lo que
    el test quiere medir.
    """
    if resolve_host:
        if host is not None:
            raise ValueError(
                "resolve_host=True ya dice que la guarda resuelve el host: "
                "darlo ademas hace que el test no mida lo que dice medir"
            )
    elif host is None:
        raise ValueError(
            "el host hay que darlo: la guarda lo resuelve con `rustc -vV`, y con "
            "el HOME aislado eso hace que rustup descargue la toolchain. Si lo que "
            "se quiere es ejercitar esa ruta, pide resolve_host=True."
        )
    argv = [str(GUARD), target]
    if host is not None:
        argv.append(host)
    with tempfile.TemporaryDirectory() as raw:
        # El PATH se antepone el dir temporal pero conserva el real: la guarda
        # llama a `rustc -vV` para resolver el host, y con un PATH minimo el
        # target nativo parecia uno cruzado y se rechazaba a si mismo.
        prefix = f"{bin_dir}:" if bin_dir else ""
        env = {
            "PATH": f"{prefix}{raw}:{os.environ.get('PATH', '/usr/bin:/bin')}",
            "HOME": raw,
            "TMPDIR": raw,
        }
        for name in list(env_extra):
            if env_extra[name] == "":
                env.pop(name, None)
        env.update({k: v for k, v in env_extra.items() if v != ""})
        # Se llama por ruta absoluta: el PATH de la guarda es deliberadamente
        # minimo y no debe decides si el propio script se encuentra.
        return subprocess.run(
            argv,
            capture_output=True,
            text=True,
            timeout=120,
            env=env,
        )


def make_cc(directory: str, body: str, name: str = "fake-cc") -> str:
    """Un compilador de mentira, en SU PROPIO fichero.

    Dos llamadas al mismo nombre se pisan: la segunda reescribe la primera y
    ambos tests acaban usando el mismo compilador. Por eso el nombre es un
    parametro y no una constante.
    """
    path = Path(directory) / name
    path.write_text(body, encoding="utf-8")
    path.chmod(0o755)
    return str(path)


def test_the_guard_exists_and_is_executable() -> None:
    assert GUARD.is_file(), f"no existe {GUARD}"
    assert os.access(GUARD, os.X_OK), f"{GUARD} no es ejecutable"


def test_a_native_target_is_not_cross_checked() -> None:
    """Preguntar por el toolchain nativo solo añadiria ruido."""
    done = run_guard(host_target(), host=host_target())
    assert done.returncode == 0, (
        f"el target nativo debe pasar sin comprobacion cruzada.\n{done.stdout}"
    )
    assert "host" in done.stdout.lower(), done.stdout


def test_a_target_with_no_compiler_fails_and_says_which_variable() -> None:
    target = "aarch64-unknown-linux-gnu"
    done = run_guard(
        target,
        host=host_target(),
        CC_aarch64_unknown_linux_gnu="",
        CXX_aarch64_unknown_linux_gnu="",
    )
    assert done.returncode != 0, (
        f"sin compilador de C el target no compila y la guarda debe decirlo.\n"
        f"{done.stdout}"
    )
    assert "CC_aarch64_unknown_linux_gnu" in done.stdout, (
        f"el motivo tiene que decir que variable definir, no solo que algo "
        f"falta.\n{done.stdout}"
    )


def test_a_compiler_that_rejects_the_triple_fails() -> None:
    """El caso que la stage anterior no podia ver.

    Un compilador instalado que no parsea el triple se comporta, desde fuera,
    igual que uno que no existe — salvo que la stage de rustup lo daba por
    bueno. Este es exactamente el fallo de v0.101.4.
    """
    target = "aarch64-unknown-linux-gnu"
    with tempfile.TemporaryDirectory() as raw:
        cc = make_cc(raw, CC_REJECTS)
        done = run_guard(target, host=host_target(), CC_aarch64_unknown_linux_gnu=cc)
    assert done.returncode != 0, (
        f"un compilador que rechaza el triple debe fallar la guarda.\n{done.stdout}"
    )
    assert "rejects the target triple" in done.stdout, (
        f"el motivo tiene que distinguir 'no hay compilador' de 'el "
        f"compilador no compila para este target'.\n{done.stdout}"
    )
    # Y debe mostrar el error del propio compilador: sin el, quien depure esto
    # tiene que adivinar que paso.
    assert "UnknownOperatingSystem" in done.stdout, (
        f"la salida del compilador es la unica evidencia del por que.\n{done.stdout}"
    )


def test_a_working_compiler_passes_and_names_itself() -> None:
    """Un C y un C++ que compilan el probe: la guarda pasa y dice cuales uso.

    El C++ tambien se configura porque para un target cruzado es
    precondicion, no un extra: ver
    `test_a_target_with_no_cxx_compiler_fails_and_says_which_variable`. Con las
    dos mitas en su sitio, la guarda tiene que nombrar las dos, para que un
    fallo posterior se pueda atribuir a un compilador y no al otro.
    """
    target = "aarch64-unknown-linux-gnu"
    with tempfile.TemporaryDirectory() as raw:
        cc = make_cc(raw, CC_OK, "fake-cc-ok")
        cxx = make_cc(raw, CC_OK, "fake-cxx-ok")
        done = run_guard(
            target,
            host=host_target(),
            CC_aarch64_unknown_linux_gnu=cc,
            CXX_aarch64_unknown_linux_gnu=cxx,
        )
    assert done.returncode == 0, (
        f"unos compiladores que compilan el probe deben pasar.\nstdout:\n{done.stdout}\n"
        f"stderr:\n{done.stderr}"
    )
    assert "fake-cc-ok" in done.stdout, (
        f"la guarda debe decir que compilador de C uso, para que un fallo "
        f"posterior se pueda atribuir.\n{done.stdout}"
    )
    assert "fake-cxx-ok" in done.stdout, (
        f"la guarda debe decir que compilador de C++ uso.\n{done.stdout}"
    )


def test_a_broken_cxx_compiler_fails() -> None:
    """Un C++ configurado y roto es fallo, y tiene que decirlo.

    Si el operador ha configurado uno, el build lo usara, asi que un C++ roto
    tiene que decirselo ahora y no veinte minutos despues.
    """
    target = "aarch64-unknown-linux-gnu"
    with tempfile.TemporaryDirectory() as raw:
        cc_ok = make_cc(raw, CC_OK, "fake-cc-ok")
        cxx_bad = make_cc(raw, CC_REJECTS, "fake-cxx-bad")
        bad = run_guard(
            target,
            host=host_target(),
            CC_aarch64_unknown_linux_gnu=cc_ok,
            CXX_aarch64_unknown_linux_gnu=cxx_bad,
        )
    assert bad.returncode != 0, f"un C++ roto debe fallar.\n{bad.stdout}"
    assert "C++" in bad.stdout, bad.stdout


def test_a_target_with_no_cxx_compiler_fails_and_says_which_variable() -> None:
    """Sin C++ esta lane no compila. Medido, no supuesto.

    MEDIDO 2026-10-03. La lane de v0.101.4 cayo en `binaries-aarch64` con

        error occurred in cc-rs: failed to find tool "aarch64-linux-gnu-g++"

    y este test, hasta ahora, PINABA LO CONTRARIO: afirmaba que un target sin
    C++ configurado debe pasar, con el motivo de que "no todos los toolchain lo
    traen". Ese motivo es cierto en general y falso aqui. `cc-rs` no eligio
    g++ por capricho: un build script del arbol lo pidio, asi que la ausencia
    de C++ no es una hipotesis sobre el toolchain, es un fallo conocido de
    este repositorio.

    El efecto medido del comportamiento anterior era una guarda que pasaba
    exactamente con el entorno que hacia fracasar el build veinte minutos mas
    tarde. Una guarda que no puede fallar sobre la condicion que dice cubrir no
    es una guarda: es una linea de log.

    Asi que la ausencia de C++ para un target cruzado es fallo, y el mensaje
    tiene que nombrar la variable que falta. Un fallo que no dice cual
    configurar es un fallo que se repite.
    """
    target = "aarch64-unknown-linux-gnu"
    with tempfile.TemporaryDirectory() as raw:
        cc_ok = make_cc(raw, CC_OK, "fake-cc-ok")
        absent = run_guard(
            target, host=host_target(), CC_aarch64_unknown_linux_gnu=cc_ok
        )
    assert absent.returncode != 0, (
        f"sin C++ esta lane no compila, y la guarda no puede decir que si.\n"
        f"{absent.stdout}"
    )
    assert "CXX_aarch64_unknown_linux_gnu" in absent.stdout, (
        f"el fallo tiene que nombrar la variable que falta, no solo que falta algo.\n"
        f"{absent.stdout}"
    )


def test_the_guard_does_not_ask_rustc_when_the_host_is_given() -> None:
    """Con el host dado, la guarda no necesita preguntar a rustc.

    MEDIDO 2026-10-25. La guarda resolvia el host con `rustc -vV` DENTRO de un
    HOME aislado, y ahi rustup no encuentra ninguna toolchain: lee el
    `rust-toolchain.toml` del repositorio e intenta instalar la suya desde la
    red. Medido: 117,7 s por llamada, y el `timeout=120` del contrato se
    agotaba. Con la red un poco mas lenta, el contrato fallaba y la etapa
    `contracts` del gate caia -- sin que el codigo del arbol tuviera nada que
    ver. Con el host dado, la guarda no toca rustc y no puede depender de la red.
    """
    with tempfile.TemporaryDirectory() as raw:
        marker = Path(raw) / "rustc-was-called"
        make_cc(raw, RUSTC_SILENT, "rustc")
        done = run_guard(
            "x86_64-unknown-linux-gnu",
            host="x86_64-unknown-linux-gnu",
            bin_dir=raw,
            RUSTC_MARKER=str(marker),
        )
        called = marker.exists()
    assert done.returncode == 0, (
        f"con el host dado, la guarda tiene que pasar sin preguntar a rustc.\n"
        f"{done.stdout}{done.stderr}"
    )
    assert not called, (
        "la guarda no puede preguntar a rustc cuando el host ya se le dio: esa "
        "pregunta es la que hace que rustup descargue la toolchain"
    )


def test_a_guard_that_cannot_resolve_the_host_refuses_to_give_a_verdict() -> None:
    """Sin host, y con un rustc que no responde, la guarda no puede veredicto.

    Antes, un `rustc -vV` que fallaba dejaba el host vacio, y el target nativo
    --el que se estaba comprobando-- se rechazaba como si fuera cruzado. Ese
    veredicto hablaba del compilador que no se llego a mirar y del toolchain que
    estaba roto, y decia "falta el compilador de C". Un fallo que no dice su
    causa es un fallo que se repite.
    """
    with tempfile.TemporaryDirectory() as raw:
        make_cc(raw, RUSTC_SILENT, "rustc")
        done = run_guard(
            "x86_64-unknown-linux-gnu",
            resolve_host=True,
            bin_dir=raw,
            RUSTC_MARKER=str(Path(raw) / "rustc-was-called"),
        )
    assert done.returncode != 0, (
        "sin host la guarda no puede dar un veredicto: no sabe si el target es "
        "el nativo\n" + done.stdout + done.stderr
    )
    combined = done.stdout + done.stderr
    assert "host" in combined.lower(), (
        f"el motivo tiene que decir que no se pudo resolver el host.\n{combined}"
    )
    assert "no C compiler" not in combined, (
        f"el motivo no puede ser que falte el compilador: el compilador no llego "
        f"a preguntarse.\n{combined}"
    )


def test_the_guard_resolves_the_host_when_it_is_not_given() -> None:
    """La ruta de resolver el host dentro de la guarda sigue viva.

    El contrato ya no la ejercita de forma implicita --le da el host--, asi que
    esta comprobacion la mantiene a proposito: un `rustc` de mentira responde
    con el host y la guarda lo usa.
    """
    with tempfile.TemporaryDirectory() as raw:
        make_cc(raw, RUSTC_HOSTS, "rustc")
        done = run_guard(
            "x86_64-unknown-linux-gnu", resolve_host=True, bin_dir=raw
        )
    assert done.returncode == 0, (
        f"un rustc que responde con el host tiene que bastarle a la guarda.\n"
        f"{done.stdout}{done.stderr}"
    )
    assert "host target" in done.stdout.lower(), (
        f"la guarda tiene que decir que el target es el host.\n{done.stdout}"
    )


def test_the_guard_is_where_it_is_announced() -> None:
    """La guarda tiene que existir y ser ejecutable, o no es una guarda.

    Antes de 2026-11-04 esto exigia que una stage de la lane del candidato la
    invocara. Las lanes se retiraron; la guarda se queda, porque es lo que
    sabe decir si un target cruzado compila codigo nativo o solo esta instalado.

    Sin lane que la invoque, la guarda se usa a mano antes de una
    compilacion cruzada, y lo que este contrato sigue exigiendo es que exista,
    sea ejecutable, y no haya sido degradada a un aviso.
    """
    guard = REPO_ROOT / "scripts" / "ci" / "check-cross-toolchain.sh"
    assert guard.is_file(), f"no existe {guard}"
    assert os.access(guard, os.X_OK), f"{guard} no es ejecutable"
    text = guard.read_text(encoding="utf-8")
    assert "rustup target list --installed" in text, (
        "la comprobacion de rustup se queda: el target instalado por rustup no "
        "demuestra que el target compile codigo nativo, y quitarla deja la "
        "guarda sin la mitad que si es la importante"
    )


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
# scripts/ci/ y por tanto fuera del gate, y solo prueba el camino feliz.
#
# Con COGNICODE_VERSION y COGNICODE_RELEASE_BASE fijos, install.sh no hace
# ninguna llamada a GitHub: el tag ya esta resuelto y la base es la que le
# damos. Con `file://` el stage entero es hermetico —sin red y sin release
# publicada— asi que estas posturas se comprueban en el gate y no en la
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


# III. El conjunto de assets del release-candidate es completo y atribuible
#
# Antes vivia en `test_release_candidate_asset_set.py`: un script con `--version`
# y `--platform` obligatorios, sin una sola funcion `test_*`, que
# `run-all-contracts.sh` excluia del glob y que ninguna lane invocaba. Un
# contrato que nadie ejecuta no vigila nada. Aqui lo ejercita la suite, contra
# un directorio de mentira y un repositorio git de fixture, sin artefactos
# reales y sin argumentos. Cada prueba rompe UNA cosa del fixture sano y exige
# que el defecto salga nombrado; la prueba sana exige cero defectos, para que
# las demas no puedan pasar por una funcion que siempre protesta.

CANDIDATE_VERSION = "0.101.17"
CANDIDATE_PLATFORM = "x86_64-unknown-linux-gnu"


def _isolated_git_env(home: Path) -> dict[str, str]:
    """Entorno de git que no lee la configuracion de la maquina.

    Un repositorio de fixture con la config global del operador heredaria sus
    hooks, su firma y su identidad, y el contrato dependeria de quien lo corre.
    """
    return {
        "PATH": os.environ.get("PATH", ""),
        "HOME": str(home),
        "GIT_CONFIG_GLOBAL": os.devnull,
        "GIT_CONFIG_SYSTEM": os.devnull,
        "GIT_CONFIG_NOSYSTEM": "1",
    }


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 16), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _parse_sha256sums(path: Path) -> dict[str, str]:
    """`<sha256>  <filename>` (dos espacios) o `<sha256> *<filename>`."""
    entries: dict[str, str] = {}
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "  " not in line:
            continue
        digest, name = line.split("  ", 1)
        entries[name.lstrip("*").strip()] = digest.strip()
    return entries


def _git_commit_exists(repo: Path, sha: str) -> bool:
    done = subprocess.run(
        ["git", "rev-parse", "--verify", sha + "^{commit}"],
        capture_output=True, text=True, cwd=repo, timeout=10,
    )
    return done.returncode == 0


def candidate_defects(root: Path, version: str, platform_token: str, repo: Path) -> list[str]:
    """Defectos de un conjunto de assets local; vacio si es completo y atribuible.

    Cada fichero que el inventario nombra existe, coincide con su sha256 y con
    la entrada de `SHA256SUMS`; el `source_commit` existe en `repo`; el nombre
    lleva el token de plataforma; y los ficheros auxiliares de la lane estan.
    """
    inventory_path = root / f"release-inventory-{version}.json"
    sums_path = root / "SHA256SUMS"
    if not inventory_path.exists():
        return [f"inventory missing: {inventory_path.name}"]
    if not sums_path.exists():
        return ["SHA256SUMS missing"]

    defects: list[str] = []
    inventory = json.loads(inventory_path.read_text())
    if inventory.get("version") != version:
        defects.append(
            f"inventory version {inventory.get('version')!r} != {version!r}"
        )
    commit = inventory.get("source_commit")
    if not commit:
        defects.append("inventory missing source_commit")
    elif not _git_commit_exists(repo, commit):
        defects.append(f"inventory source_commit {commit!r} not reachable from HEAD")

    artifacts = inventory.get("artifacts", [])
    if not artifacts:
        defects.append("inventory has no artifacts")

    sums = _parse_sha256sums(sums_path)
    for artifact in artifacts:
        name, declared = artifact.get("filename"), artifact.get("sha256")
        if not name or not declared:
            defects.append(f"artifact missing filename/sha256: {artifact}")
            continue
        path = root / name
        if not path.exists():
            defects.append(f"missing file on disk: {name}")
            continue
        actual = _sha256_file(path)
        if actual != declared:
            defects.append(
                f"sha256 mismatch for {name}: declared={declared}, actual={actual}"
            )
        listed = sums.get(name)
        if listed is None:
            defects.append(f"file {name} not in SHA256SUMS")
        elif listed != declared:
            defects.append(
                f"SHA256SUMS hash for {name} ({listed}) != inventory ({declared})"
            )
        if platform_token not in name:
            defects.append(
                f"artifact {name} does not carry platform token {platform_token}"
            )

    for name in (
        f"bundle-{version}-{platform_token}.yaml",
        f"cognicode-{version}.tar.gz",
        f"cognicode-mcp-{version}.tar.gz",
    ):
        if not (root / name).exists():
            defects.append(f"missing auxiliary file: {name}")
    return defects


def _candidate_fixture(base: Path) -> tuple[Path, Path, str]:
    """Un release local sano: (directorio de assets, repo git, source_commit)."""
    repo = base / "repo"
    repo.mkdir()
    env = _isolated_git_env(base)

    def git(*args: str, stdin: str | None = None) -> str:
        done = subprocess.run(
            ["git", *args], cwd=repo, env=env, input=stdin,
            capture_output=True, text=True, timeout=30,
        )
        assert done.returncode == 0, f"fixture git {args[:2]}: {done.stderr}"
        return done.stdout.strip()

    git("init", "-q")
    # El commit se escribe como objeto, sin `git commit`: el `git` de una
    # maquina con politica de identidad rechaza el autor de un fixture, y este
    # contrato solo necesita que el objeto exista en el repo.
    tree = git("hash-object", "-w", "-t", "tree", "--stdin", stdin="")
    commit = git(
        "hash-object", "-w", "-t", "commit", "--stdin",
        stdin=(
            f"tree {tree}\n"
            "author fixture <fixture@example.invalid> 0 +0000\n"
            "committer fixture <fixture@example.invalid> 0 +0000\n\nfixture\n"
        ),
    )

    root = base / "release"
    root.mkdir()
    artifact = f"cognicode-{CANDIDATE_VERSION}-{CANDIDATE_PLATFORM}.tar.gz"
    payload = b"binary-bytes-of-the-fixture"
    (root / artifact).write_bytes(payload)
    digest = hashlib.sha256(payload).hexdigest()
    (root / f"release-inventory-{CANDIDATE_VERSION}.json").write_text(
        json.dumps(
            {
                "version": CANDIDATE_VERSION,
                "source_commit": commit,
                "artifacts": [{"filename": artifact, "sha256": digest}],
            }
        )
    )
    (root / "SHA256SUMS").write_text(f"{digest}  {artifact}\n")
    for aux in (
        f"bundle-{CANDIDATE_VERSION}-{CANDIDATE_PLATFORM}.yaml",
        f"cognicode-{CANDIDATE_VERSION}.tar.gz",
        f"cognicode-mcp-{CANDIDATE_VERSION}.tar.gz",
    ):
        (root / aux).write_bytes(b"aux")
    return root, repo, commit


def _candidate_defects_of(mutate) -> list[str]:
    """Defectos que el contrato halla tras aplicar `mutate(root, repo, commit)`."""
    with tempfile.TemporaryDirectory() as tmp:
        root, repo, commit = _candidate_fixture(Path(tmp))
        mutate(root, repo, commit)
        return candidate_defects(root, CANDIDATE_VERSION, CANDIDATE_PLATFORM, repo)


def _rewrite_inventory(root: Path, change) -> None:
    path = root / f"release-inventory-{CANDIDATE_VERSION}.json"
    inventory = json.loads(path.read_text())
    change(inventory)
    path.write_text(json.dumps(inventory))


def _assert_names(defects: list[str], needle: str) -> None:
    assert any(needle in d for d in defects), (
        f"se esperaba un defecto que contuviera {needle!r}; el contrato dijo: {defects}"
    )


def test_a_sound_release_candidate_has_no_defects() -> None:
    assert _candidate_defects_of(lambda *_: None) == []


def test_a_tampered_artifact_is_a_sha256_mismatch() -> None:
    def tamper(root: Path, *_: object) -> None:
        (root / f"cognicode-{CANDIDATE_VERSION}-{CANDIDATE_PLATFORM}.tar.gz").write_bytes(b"other")

    _assert_names(_candidate_defects_of(tamper), "sha256 mismatch")


def test_an_inventory_hash_that_disagrees_with_sha256sums_is_named() -> None:
    def disagree(root: Path, *_: object) -> None:
        _rewrite_inventory(root, lambda inv: inv["artifacts"][0].update(sha256="0" * 64))

    defects = _candidate_defects_of(disagree)
    _assert_names(defects, "sha256 mismatch")
    _assert_names(defects, "SHA256SUMS hash for")


def test_an_artifact_missing_from_sha256sums_is_named() -> None:
    def drop(root: Path, *_: object) -> None:
        (root / "SHA256SUMS").write_text("")

    _assert_names(_candidate_defects_of(drop), "not in SHA256SUMS")


def test_an_artifact_missing_on_disk_is_named() -> None:
    def remove(root: Path, *_: object) -> None:
        (root / f"cognicode-{CANDIDATE_VERSION}-{CANDIDATE_PLATFORM}.tar.gz").unlink()

    _assert_names(_candidate_defects_of(remove), "missing file on disk")


def test_a_source_commit_that_git_does_not_know_is_unreachable() -> None:
    def forge(root: Path, *_: object) -> None:
        _rewrite_inventory(root, lambda inv: inv.update(source_commit="1" * 40))

    _assert_names(_candidate_defects_of(forge), "not reachable")


def test_an_inventory_without_source_commit_or_artifacts_is_named() -> None:
    def strip(root: Path, *_: object) -> None:
        _rewrite_inventory(root, lambda inv: (inv.pop("source_commit"), inv.update(artifacts=[])))

    defects = _candidate_defects_of(strip)
    _assert_names(defects, "missing source_commit")
    _assert_names(defects, "no artifacts")


def test_a_wrong_version_or_platform_token_is_named() -> None:
    def other_version(root: Path, *_: object) -> None:
        _rewrite_inventory(root, lambda inv: inv.update(version="0.0.1"))

    _assert_names(_candidate_defects_of(other_version), "inventory version")

    with tempfile.TemporaryDirectory() as tmp:
        root, repo, _ = _candidate_fixture(Path(tmp))
        defects = candidate_defects(root, CANDIDATE_VERSION, "aarch64-apple-darwin", repo)
    _assert_names(defects, "does not carry platform token")


def test_a_missing_auxiliary_file_is_named() -> None:
    def remove(root: Path, *_: object) -> None:
        (root / f"cognicode-mcp-{CANDIDATE_VERSION}.tar.gz").unlink()

    _assert_names(_candidate_defects_of(remove), "missing auxiliary file")


def test_a_missing_inventory_or_sha256sums_is_a_defect_not_a_crash() -> None:
    def no_sums(root: Path, *_: object) -> None:
        (root / "SHA256SUMS").unlink()

    _assert_names(_candidate_defects_of(no_sums), "SHA256SUMS missing")

    def no_inventory(root: Path, *_: object) -> None:
        (root / f"release-inventory-{CANDIDATE_VERSION}.json").unlink()

    _assert_names(_candidate_defects_of(no_inventory), "inventory missing")


def main() -> int:
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
        "PASS - un target instalado que no compila codigo nativo no se cuela "
        "como un target listo, y el asset que pide install.sh y el que publica "
        "la release se derivan igual, con las cinco posturas del instalador "
        "ejercitadas contra una release local, y un conjunto de assets del "
        "release-candidate solo es completo si inventario, SHA256SUMS, ficheros "
        "y commit de origen coinciden."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
