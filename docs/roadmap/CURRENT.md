# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda fuente
> de estado: donde este documento y el ROADMAP diverjan, manda el ROADMAP y
> este snapshot se considera obsoleto hasta su próxima regeneración.
>
> **Snapshot**: 2026-10-22, sobre `46915ce564469779108a190b2401c0cd03731f00` (`origin/main`), rama
> `fix/every-test-surface-has-a-lane`, `origin/main` = `46915ce564469779108a190b2401c0cd03731f00`.
> Versión del workspace: `0.101.26`. Última release publicada: `v0.101.23`
> (2026-10-18, 8 assets). `main` va **47** commits por delante de ese tag.
>
> **La historia vive en el Journal**; aquí sólo vive el estado.

## Release: tres cortes con tag y ninguno publicado

| Tag | Estado | Por qué |
|---|---|---|
| `v0.101.23` | **publicada** (2026-10-18) | la última que un usuario puede instalar |
| `v0.101.24` | tag, sin release | su gate de candidato encontró un defecto real del índice de símbolos |
| `v0.101.25` | tag, sin release | su gate de candidato murió en la última etapa por un comando de shell mal escrito |
| `v0.101.26` | tag, sin release | su gate de candidato murió en `clean-clone` por un defecto real del filtro de recorrido |

Un tag que no llegó a publicarse no se mueve. El siguiente corte publicable
es **`v0.101.27`**, y ya tiene resuelta la primera de las dos condiciones: el
filtro de recorrido que mataba el clon está arreglado y fusionado. Lo que
queda es su gate de candidato, que tiene que correr sobre un `main` que lo
incluya.

## La regla que este ciclo destapó

El gate de merge falla por dos razones que no son del código, y confundirlas
cuesta un ciclo entero:

1. **Cuota de disco.** `/tmp` es un tmpfs con cuota de usuario. Cuando se
   llena, `cargo test` devuelve `os error 122` y **379 pruebas fallan a la vez**
   con un error que no menciona la cuota. El arreglo es mover los temporales a
   un volumen con espacio: `TMPDIR=/var/home/rubentxu/cognicode-tmp`. El
   volumen de compilación ya se movió; los temporales de las pruebas, no.
2. **Defecto real.** El filtro de recorrido excluía cualquier ruta que tuviera
   un directorio llamado `.cache` en cualquiera de sus componentes. El gate de
   candidato clona en `~/.cache/cognicode/gate/`, así que el clon se ciega a
   sí mismo y el proyecto sale vacío. **Arreglado y fusionado**: el filtro ya
   no ciega al clon.
3. **Carriles que se invalidan entre sí.** Dos gates de merge corriendo a la
   vez sobre el mismo repositorio comparten el puntero de la última corrida, y
   se invalidan mutuamente. El síntoma —«cookie file not created» con la
   etapaunderlying en verde— no dice nada del código. Los gates pesados van
   **en serie**, uno detrás de otro.

Un gate que muere por cuota **no** dice nada del código que quiere probar. Se
comprueba el motivo antes de tocar una línea.

## Unidades abiertas

| Unidad | Qué es | Detalle |
|---|---|---|
| **C14** | publicar una release con las estabilizaciones posteriores a `v0.101.23` | `ROADMAP.md` §11 |
| **C32** | el gate tiene que dar el mismo resultado en una máquina limpia | `ROADMAP.md` §11 |
| **B3** (`C19`, `C30`, `C9`, `C16`, `C10`, `C26`, `C15`, `C27`, `C22`, `C23`) | ontología y arquitectura simplificadas | `ROADMAP.md` §11 |
| **H5** (`C11`) | `SnapshotKey` e incrementalidad honesta | `consumer-contracts/specs/SPEC-CU-005-SNAPSHOT-INCREMENTAL.md` |
| **H3** (`C12`), **H4.4** (`C13`) | navegación Kotlin sin JVM y sus relaciones con Java | `ROADMAP.md` §11 |

**C6, C31, C5, C21, C8 y C7 están cerrados.** El lazo de feedback ejecutaba la
biblioteca del crate y nunca el test de integración que se acababa de
escribir: ahora pide el binario que produce ese fichero —y sólo ese— sin
convertir el modo rápido en la batería completa. La autoridad de una
herramienta dejó de derivarse del catálogo del proyecto analizado. La línea
base vacía dejó de obligar a reejecutar la suite entera. La configuración
ejecutable dejó de apuntar a documentos que no existen, el guard de
procedencia pasó a gobernar el contrato del consumidor, y cada superficie de
prueba tiene ahora un carril declarado o la razón escrita de por qué se
queda sin él.

## La misión

Misión `cognicode` declarada (`goal-compass.json`), con candidatos que llevan
evidencia —una ruta que existe— y un criterio de cierre observable. La cola
está escrita en el orden que fija la auditoría del 2026-10-21 (sus bloques B1
a B6), de mayor a menor prioridad.

El primero que encadenaba era **C31**, y está cerrado: la autoridad con la que
una herramienta MCP se ejecutaba la leía del catálogo que había en el disco
del proyecto analizado. Un proyecto que declaraba `write_file` como
herramienta de solo lectura conseguía que el modo de solo lectura la
aceptase. La autoridad de una herramienta es del producto, y el producto no
es el proyecto que se analiza. Detrás de él vinieron **C5**, **C21**, **C8**
y **C7**, también cerrados.

**C25, C20, C24, C18, C28 y C29 están cerrados.** C17 salió en la PR #404.
C2, C3 y C4 salieron en las PR #400, #401 y #402.

## Gates

| Gate | Comando | Cuándo |
|---|---|---|
| fast | `bash scripts/test-fast.sh` | durante el evolutivo, con `--filter` mientras la unidad sea una |
| full | `bash scripts/test-full.sh` | antes de PR, antes de tag, antes de bump de SemVer |
| merge-gate | `bash scripts/ci/publish-merge-gate.sh` | publica el veredicto que `main` exige |

`full` incluye el gate de contratos. `merge-gate` corre sobre el árbol de
trabajo, así que **no se edita código mientras corre**: el resultado
correspondería a otro árbol. Y corre en un volumen con espacio: si los
temporales van a `/tmp`, la cuota puede matar la batería entera.

## Bloqueos abiertos

- **Publicación**: `main` lleva 47 commits sin publicar y las tres releases
  con tag siguen sin existir para quien las quiera instalar. El corte
  `v0.101.27` está preparado y esperando su gate de candidato.
- **C8-R (CR-01)**: la firma *contractual* del operador sobre un clon limpio.
  La firma operativa está hecha; la contractual no. Las citas al dossier del
  programa cerrado (`docs/prf/`) son históricas por contrato y no se
  reescriben, pero ese dossier ya no está en el árbol: la re-verificación
  tiene que apoyarse en la certificación vigente y en el Journal.
- **SDDK storage**: `~/.local/share/sddk/data/ledger.sqlite` está vacío y el
  ciclo no arranca. Es operator-gated, no un bug de CogniCode.
- **Binarios release**: algunas suites de CLI requieren
  `$CARGO_TARGET_DIR/release`. Si el binario falta, el fallo es de
  infraestructura local, no una regresión: `cargo build --release --bin cogh`.

## Dónde está cada cosa

| Qué | Dónde |
|---|---|
| Agenda (la única autoridad) | `docs/roadmap/ROADMAP.md`, §11 |
| Recibo de cada unidad, append-only | `docs/roadmap/JOURNAL.md` |
| Decisiones | `docs/adr/` y `docs/roadmap/consumer-contracts/**/adr/` |
| Contratos publicados del consumidor | `docs/roadmap/consumer-contracts/specs/` |
| Certificaciones | `docs/roadmap/certifications/` |
| Guía de distribución | `docs/distribution/INSTALL.md` |
| Identidad de acciones de la comunidad | `docs/cognicode-community-productization/` |
| Backlog de la misión | `goal-compass.json` |
| Cómo se decide qué se toca | `AGENTS.md` |
