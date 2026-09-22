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
sus artefactos y perfiles. Linux x86_64/aarch64 GNU es la matriz publicada.

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
cogh uninstall mcp-server --version 0.97.3 --ide opencode
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
