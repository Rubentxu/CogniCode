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

Los bundles de skills de esta release son únicamente los dos que `cogh`
instala (`cognicode` y `cognicode-mcp`). No existe una skill de auditoría
externa publicada por separado: no hay nada más que instalar con `npx`.

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
cogh uninstall mcp-server --version v0.103.0 --ide opencode
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

The four crates are published on crates.io at version `0.103.0`, and a clean
consumer project that depends on those exact versions builds and runs. You can
repeat that check with
`bash scripts/distribution/verify-sdk-consumer.sh --confirm-published`.

Add the crates in publication order, dependencies first
(`cognicode-core` depends on `cognicode-graph-algos` and `cognicode-macros`;
`cognicode-graph-wasm` depends on `cognicode-graph-algos`):

```bash
cargo add cognicode-graph-algos@=0.103.0
cargo add cognicode-macros@=0.103.0
cargo add cognicode-core@=0.103.0
cargo add cognicode-graph-wasm@=0.103.0
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

Limitation: release `v0.102.1` is published (`gh release view` reports it as
not a draft, with eight assets), so you can install the server with `cogh`
instead of building it; the commands are in the installation section above.
On November 9, 2026 that sequence was run once against the release, on
`x86_64-unknown-linux-gnu`, with the `cogh` home set to a new, empty temporary
directory: `cogh init`, then the `reviewer` install for OpenCode, which
reported version 0.102.1. The `cognicode` and `cognicode-mcp` shims printed
0.102.1; `cogh current`, `cogh where cognicode-mcp` and `cogh doctor`
(`overall: healthy`) passed; and a dry run of `cogh update` resolved the bundle of
`v0.102.1` without downloading it. The hashes of the two component tarballs it
cached matched the entries of the release `SHA256SUMS`. Two behaviors of that
run matter to you: installing a plugin fails without a prior `cogh init`, and
it downloads and writes under the `cogh` home even without the apply option
(which only governs the self-install of `cogh`), so rehearse it in a throwaway
home. Not run: the remote `install.sh`, an install into the real home, an IDE
other than OpenCode, and a handshake of the installed server from a real MCP
client; none of those is confirmed here. If `cogh` is unavailable, you can
still compile the server from source (see the README) and register its
absolute path as `command`.

## Release assets

This section is written in English; the rest of this guide is in Spanish.

A release `v0.103.0` publishes the files below, named by the functions of
`crates/cognicode-cli/src/cmd/release_contract.rs`. Only the certified platform
`x86_64-unknown-linux-gnu` is published; `product/platforms.json` marks
`aarch64` and `musl` as experimental and macOS and Windows as unsupported.

Binaries, one tarball per component and platform:

- `cogh-0.103.0-x86_64-unknown-linux-gnu.tar.gz`
- `cognicode-0.103.0-x86_64-unknown-linux-gnu.tar.gz`
- `cognicode-mcp-0.103.0-x86_64-unknown-linux-gnu.tar.gz`

Skill bundles, platform-independent (they are skills, not executables):

- `cognicode-0.103.0.tar.gz`
- `cognicode-mcp-0.103.0.tar.gz`

Manifests and checksums:

- `bundle-0.103.0-x86_64-unknown-linux-gnu.yaml`: the platform manifest that
  `cogh` resolves.
- `release-inventory-0.103.0.json`: the inventory of the release.
- `SHA256SUMS`: the checksum of every file above.

`explorer-api` and `explorer-mcp` are not published, and neither is the
`cognicode-developer` skill bundle.

Verify integrity after downloading every file you need into one empty
directory (the URL base is
`https://github.com/Rubentxu/CogniCode/releases/download/v0.103.0/`):

```bash
sha256sum --check --ignore-missing SHA256SUMS
```

`--ignore-missing` lets you check only the files you downloaded; a file that is
present and does not match still fails the command.

Limitation: these names are derived from the release contract in this tree and
were compared, on November 9, 2026, with the asset list that
`gh release view v0.103.0` reports: all eight match and the release is not a
draft. On the same day the x86_64 Linux assets were downloaded to a fresh
directory and `sha256sum --check SHA256SUMS` matched every entry; the
`cognicode`, `cognicode-mcp` and `cogh` binaries extracted from them ran
`--version` and reported 0.103.0. The remote `install.sh` installed `cogh`
0.103.0 into a temporary directory. Only that one platform was exercised, and
a full MCP start was not. The `mise` and `asdf` routes are described in the
next section, with what was and was not measured. Provenance is out of scope here and is absent from
v0.103.0: no attestation is published, and signatures and SBOMs are not asset
kinds in `release_contract.rs`. How provenance is produced is an open key
custody decision (see `scripts/ci/attest-provenance.sh`).

## mise and asdf

This section is written in English. It records what was run on November 9,
2026 with mise 2026.8.6 and asdf 0.19.0 on x86_64 Linux, each in a fresh data
directory.

`mise` installs from the GitHub release with its `github:` backend, with no
plugin. A bare `github:Rubentxu/CogniCode@0.103.0` picks the first matching
asset, which is the `cognicode` tarball. To get another binary, name its asset
in a `mise.toml`:

```toml
[tools."github:Rubentxu/CogniCode"]
version = "0.103.0"
asset_pattern = "cogh-*-x86_64-unknown-linux-gnu.tar.gz"
bin = "cogh"
```

With `cognicode-mcp-*-x86_64-unknown-linux-gnu.tar.gz` and `bin = "cognicode-mcp"`
the same recipe installed `cognicode-mcp`. In each case the binary in the
mise install directory, run by its full path, reported 0.103.0.

Limitations, all measured:

- One repository key gives one binary. Two `[tools]` entries for the same
  repository, even with different options in the key, collapsed into one and
  installed only `cognicode`. To have the three binaries you need three
  separate configurations (for example one directory per binary), or the
  direct download above.
- Check the path, not the name. `mise exec ... -- cogh --version` printed
  `0.102.1` because an older `cogh` was first on `PATH`; it was not the one mise
  had installed. Compare `command -v` with the mise install directory.
- Only x86_64 Linux was exercised. mise reported that it verified artifact
  attestations and SLSA provenance, but none are published for v0.103.0, so
  that step proves nothing about provenance.

`asdf` has no published route yet. asdf 0.19.0 has no built-in GitHub backend
and installs only through a plugin, and no plugin is published for CogniCode.
The repository carries a working one in `scripts/distribution/asdf-cogh/`
(`list-all`, `download` with SHA256SUMS check, `install`). Measured with asdf
0.19.0 on x86_64 Linux in a fresh data directory, with the plugin copied to
`plugins/cogh`: `asdf install cogh 0.103.0` verified the checksum and
`asdf which cogh` pointed to the installed `cogh 0.103.0`; version 9.9.9 failed
with a 404. It installs `cogh` only; `cogh` installs the rest. `asdf plugin add`
needs the plugin in its own git repository, so publishing it is a decision for
the maintainer; until then use `mise` or the direct download.

## CLI workflows

This section is written in English; the rest of this guide is in Spanish.

Four recipes for the `cognicode` CLI, run from the root of the project
you want to analyse. Every subcommand and flag below is declared by the clap
definitions in `crates/cognicode-cli/src/cli/commands.rs` or, for the goal
commands, generated from the goal registry in
`crates/cognicode-core/src/interface/goals.rs`; the values of
`--strategy`, `--operation` and `--format` are the closed sets those sources
accept. Run `cognicode <command> --help` for the full list of options.

Map a project: index it, build the call graph, find the most called functions
and draw the graph.

```bash
cognicode index build . --strategy full
cognicode graph full . --rebuild
cognicode graph hot-paths . -n 5 --min-fan-in 2
cognicode graph mermaid . --format svg
```

`--strategy` accepts `lightweight` (the default), `full` and `on_demand`.
`hot-paths` lists at most `-n` functions (default 10) that have at least
`--min-fan-in` callers (default 1). `graph mermaid --format` accepts `code`,
`png`, `svg` and `txt`.

Preview a refactor. The operation is chosen with `--operation` (`rename`,
`extract`, `inline` or `move`), and the default is `rename`:

```bash
cognicode refactor old_name new_name --operation rename
cognicode refactor old_name new_name --operation rename --format json
```

Without `--apply` the command is a preview and writes nothing. `--apply` is a
separate authorization and also needs `--authorization-token`; it is not part of
these recipes.

Ask about one symbol with the goal commands. `status` reports the version and
the phases the CLI offers; `find` locates a symbol by name; `trace` follows its
callers and callees (`--direction` accepts `callers`, `callees` or `both`, the
default, and `--depth` is the number of hops, default 1); `impact` lists what a
change to it would reach.

```bash
cognicode status
cognicode find --symbol helper
cognicode trace --symbol helper --direction callers --depth 2
cognicode impact --symbol helper --format json
```

Each of these accepts `--format text` (the default) or `--format json`, and
works on the current workspace without the map recipe above. `status` also
reports how many source files could not be read, so a partial answer is not
mistaken for a complete one.

Check the environment: `doctor` reports which language servers are available
for the workspace.

```bash
cognicode doctor
cognicode doctor --format json --cwd .
```

Limitation: the flags and values are checked against the clap sources and the goal
registry of this tree by `scripts/ci/test_readme_contract.py`. The goal commands
were also run once against a two-function scratch project with the installed
0.102.1 CLI; none of the recipes was executed against a published release or
the remote installer. Their output is not
described here because it was not recorded.
