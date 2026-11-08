# Distribución e instalación de CogniCode

> Fuente de verdad operativa para los artefactos publicados. Una versión en
> `target/release` es un resultado local de compilación: **nunca** se utiliza
> como ruta persistente del servidor MCP ni como fuente de una instalación.

## Superficie de distribución

La release estable entrega `cogh` (gestor, capa 0), `cognicode` (CLI),
`cognicode-mcp` (servidor stdio), los manifiestos por plataforma y
`SHA256SUMS`. Los paquetes portables `cognicode` y `cognicode-mcp`
son **skills**, no ejecutables con esos nombres. `cogh` los coloca por versión
bajo `versions/<v>/skills/<bundle-id>` y los integra sin sobrescribir otros
bundles de OpenCode. Los binarios de `explorer-api`/`explorer-mcp` y las
plataformas Windows/macOS **no** forman parte de esta superficie estable:
no anunciarlos como instalables mediante `cogh` hasta publicar y certificar
sus artefactos y perfiles. Linux x86_64 GNU es la matriz publicada.
Las variantes `aarch64` y `musl` son experimentales y no se publican (`product/platforms.json`).

La skill de auditoría externa
[`cognicode-quality-investigator`](https://github.com/Rubentxu/agent-skill/tree/main/skills/cognicode-quality-investigator)
se instala **independientemente** desde `Rubentxu/agent-skill`; no debe
confundirse con los bundles de skills internos de CogniCode.

## Instalación desde cero en Linux

```sh
# Bootstrap: instala solamente cogh; la release se selecciona por la API oficial.
curl -fsSL https://raw.githubusercontent.com/Rubentxu/CogniCode/main/install.sh -o /tmp/cognicode-install.sh
sh /tmp/cognicode-install.sh

# Si cogh no está en PATH; ante una instalación previa por Cargo/mise,
# comprueba con type -a cogh QUÉ ejecutable se invoca realmente.
export PATH="$HOME/.cognicode/bin:$HOME/.cognicode/shims:$PATH"
type -a cogh

cogh init
# reviewer instala CLI, MCP y sus dos bundles de skills.
cogh install mcp-server --version latest --profile reviewer --ide opencode
cogh current
cogh doctor
cogh where cognicode
cogh where cognicode-mcp
cognicode --version
cognicode-mcp --version
```

`cogh install mcp-server --ide opencode` selecciona el perfil `reviewer`
cuando se omite `--profile`. `cogh install mcp-server` sin IDE mantiene el
perfil ligero `core`. `core` **no** incluye el daemon MCP y es incorrecto
usarlo para configurar una conexión MCP. El selector `--version latest`
se resuelve a la versión publicada antes de registrar el servidor en el IDE:
no deben aparecer directorios literales `versions/latest` ni rutas de build.

La instalación de la skill externa es independiente:

```sh
npx skills add Rubentxu/agent-skill --skill cognicode-quality-investigator --agent opencode --global
```

## Actualizar, reparar, volver atrás y desinstalar

```sh
cogh latest
cogh update --dry-run
cogh update                   # preserva el perfil activo: reviewer sigue con MCP
cogh doctor
cogh reshim                   # reconstruye enlaces desde el manifiesto instalado
cogh rollback                 # revierte la última transición registrada, si existe

# Selección deliberada de otra composición en la MISMA versión:
cogh install mcp-server --version latest --profile reviewer --ide opencode

# Retira una versión instalada y su integración IDE de forma explícita:
cogh uninstall mcp-server --version v0.102.1 --ide opencode
```

`cogh update --profile core` es un cambio deliberado de perfil y retirará
la capacidad MCP de la instalación activa; no utilizarlo como actualización
ordinaria de una instalación reviewer. Si `cogh` procede de mise, actualizar
su binario con mise; si procede del bootstrap directo, repetir `install.sh`.
`cogh update` administra únicamente los componentes de runtime.

Los shims pertenecen a `~/.cognicode/shims` (o
`$COGNICODE_HOME/shims`): apuntan a `versions/<versión>/<componente>/bin/`.
No crear enlaces persistentes a `target/release`, `/tmp` ni directorios de
tests. La configuración MCP del IDE debe referirse al shim **válido** que
apunta a la versión activa. `cogh doctor` y `cogh where cognicode-mcp`
son comprobaciones iniciales, pero conviene verificar también el arranque real
del servidor desde OpenCode.

La opción global `cogh --home /ruta/alternativa` cambia la propiedad de
manifiestos, caché, lock, versiones y shims. El directorio de configuración
del IDE es independiente del runtime y requiere consentimiento para
modificarlo. No reutilizar `HOME` global del proceso para escribir artefactos
en un `--home` diferente.

## Contrato de integridad y admisión de release

`cogh` resuelve el manifiesto publicado por versión y plataforma, verifica
SHA256 de cada componente y contrasta el SHA256 de cada bundle de skills con
`SHA256SUMS` de **esa misma release** antes de extraer. Los nombres de skills
no son intercambiables con los de los binarios, aunque coincidan: sus cachés
deben estar separadas. El comando de IDE no debe instalar una configuración
que señale a un binario ausente o a un shim dirigido a otra versión.

La publicación de una versión debe superar: instalación limpia de `reviewer`
con ambas skills, consulta CLI, handshake MCP real, actualización core→reviewer
en la misma versión, update que preserve reviewer, reshim de un enlace roto,
rollback y desinstalación sin afectar versiones ajenas ni archivos de usuario.
Probar esos recorridos desde los **tarballs generados**, con HOME/XDG limpio
y con `--home` distinto de `HOME`; ejecutar los tests completos exigidos por
la política de release antes de publicar. Un test ejecutado contra
`target/release` no sustituye la comprobación de los artefactos de distribución.

## Instalación directa de emergencia, no gestionada por cogh

En caso de fallo del gestor, el binario MCP puede extraerse del tarball
`cognicode-mcp-<versión>-<target>.tar.gz` publicado, después de verificar
el archivo con el `SHA256SUMS` de su release, en un directorio **propio y
versionado** como `~/.local/opt/cognicode-mcp/<versión>/bin/`. La configuración
de OpenCode puede apuntar temporalmente a ese binario; el estado de
`cogh doctor` no cambia por instalar una copia externa. No confundir este
procedimiento de recuperación con una reparación de la instalación gestionada.

## Consuming the SDK

This section is written in English; the rest of this guide is in Spanish.

The four `library` crates declared in
[`SDK_SURFACES.json`](SDK_SURFACES.json) are the SDK. Their layout is described
in [`SDK_ARCHITECTURE.md`](SDK_ARCHITECTURE.md) and the release procedure in
[`SDK_RELEASE.md`](SDK_RELEASE.md).

**Limitation: the SDK is not yet published on crates.io.** Publishing needs a
registry token and an explicit operator confirmation, and it is tracked by
TASK-178 (acceptance criterion 4). Until then the commands below do not
resolve from the registry and will fail. They document the intended usage at
the workspace version `0.102.1`.

Once published, add the crates in publication order, dependencies first
(`cognicode-core` depends on `cognicode-graph-algos` and `cognicode-macros`;
`cognicode-graph-wasm` depends on `cognicode-graph-algos`):

```bash
cargo add cognicode-graph-algos@=0.102.1
cargo add cognicode-macros@=0.102.1
cargo add cognicode-core@=0.102.1
cargo add cognicode-graph-wasm@=0.102.1
```

Add only the crates your project uses; the order matters for publishing, not
for consuming. Pin every crate to the same version (`=`), because they are
released together. Feature combinations and the supported toolchain are listed
in `SDK_RELEASE.md`.

## Registering cognicode-mcp in an MCP client

This section is written in English; the rest of this guide is in Spanish.

`cognicode-mcp` is a stdio MCP server: the client starts it as a child process
and talks to it over its standard streams. Register it in the client's MCP
configuration (Claude Desktop, Cursor and Windsurf share this shape):

```json
{
  "mcpServers": {
    "cognicode": {
      "command": "cognicode-mcp",
      "args": ["--cwd", "/path/to/your/project", "--profile", "default"]
    }
  }
}
```

Use the path that `cogh where cognicode-mcp` prints as `command` when the
client does not inherit the shell `PATH`; that path is the managed shim, which
points to the active version.

Arguments, as declared by the clap definition in `crates/cognicode-mcp/src/main.rs`:

- `--cwd <dir>` (short `-c`): the project root to analyse. The default is the
  current directory of the client process, which is rarely the project, so set
  it. The server exits with an error if the directory does not exist.
- `--read-only`: hide the mutating tools from `tools/list` and reject them.
- `--profile <name>`: the surface announced in `tools/list`. The accepted
  values are `default`, `full` and `legacy`; any other value makes the server
  exit with status 2. This is not the install profile of `cogh install`.

Environment:

- `COGNICODE_TELEMETRY=1` enables metric export; it is off by default and no
  source code is ever sent.
- `RUST_LOG` sets the log filter (the default level is `info`).

Streams: JSON-RPC messages travel on stdout and nothing else may be written
there; logs and diagnostics go to stderr. A client that shows a parse error on
startup is reading something that is not JSON-RPC from stdout.

Limitation: `cogh install` and the remote `install.sh` fetch a published
release, and they were not run to write this section. Until a release carries
this version, build the binary from source as the README describes and
register its absolute path as `command`.

## Release assets

This section is written in English; the rest of this guide is in Spanish.

A release `v0.102.1` publishes the files below, named by the functions of
`crates/cognicode-cli/src/cmd/release_contract.rs`. Only the certified platform
`x86_64-unknown-linux-gnu` is published; `product/platforms.json` marks
`aarch64` and `musl` as experimental and macOS and Windows as unsupported.

Binaries, one tarball per component and platform:

- `cogh-0.102.1-x86_64-unknown-linux-gnu.tar.gz`
- `cognicode-0.102.1-x86_64-unknown-linux-gnu.tar.gz`
- `cognicode-mcp-0.102.1-x86_64-unknown-linux-gnu.tar.gz`

Skill bundles, platform-independent (they are skills, not executables):

- `cognicode-0.102.1.tar.gz`
- `cognicode-mcp-0.102.1.tar.gz`

Manifests and checksums:

- `bundle-0.102.1-x86_64-unknown-linux-gnu.yaml`: the platform manifest that
  `cogh` resolves.
- `release-inventory-0.102.1.json`: the inventory of the release.
- `SHA256SUMS`: the checksum of every file above.

`explorer-api` and `explorer-mcp` are not published, and neither is the
`cognicode-developer` skill bundle.

Verify integrity after downloading every file you need into one empty
directory (the URL base is
`https://github.com/Rubentxu/CogniCode/releases/download/v0.102.1/`):

```bash
sha256sum --check --ignore-missing SHA256SUMS
```

`--ignore-missing` lets you check only the files you downloaded; a file that is
present and does not match still fails the command.

Limitation: these names are derived from the release contract in this tree.
No published release was downloaded to write this section, so the names and
the checksums are not confirmed against a real release. Provenance is also
out of scope here: signatures and SBOMs are not asset kinds in
`release_contract.rs`, and this guide does not describe how to verify them.
