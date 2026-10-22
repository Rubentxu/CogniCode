# CURRENT — Snapshot operativo (deriva de `docs/roadmap/ROADMAP.md`)

> **Autoridad**: `docs/roadmap/ROADMAP.md` es la **única autoridad de agenda**
> de CogniCode. Este fichero es un **snapshot puntual**, no una segunda fuente
> de estado: donde este documento y el ROADMAP diverjan, manda el ROADMAP y
> este snapshot se considera obsoleto hasta su próxima regeneración.
>
> **Snapshot**: 2026-10-23, sobre `0b096482fd941e213896136e47e323ccac82db59` (`origin/main` al
> cerrar este ciclo), rama `main`.
> Versión del workspace: `0.101.29`. Última release publicada: `v0.101.29`
> (2026-10-22, 8 assets).
>
> **La historia vive en el Journal**; aquí sólo vive el estado.

## Release: la última publicada es la v0.101.29

| Tag | Estado | Por qué |
|---|---|---|
| `v0.101.23` | publicada (2026-10-18) | la anterior |
| `v0.101.24` | tag, sin release | su gate de candidato encontró un defecto real del índice de símbolos |
| `v0.101.25` | tag, sin release | su gate de candidato murió en la última etapa por un comando de shell mal escrito |
| `v0.101.26` | tag, sin release | su gate de candidato murió en `clean-clone` por un defecto real del filtro de recorrido |
| `v0.101.27` | publicada (2026-10-22, 8 assets) | lleva los tres defectos que los gates anteriores encontraron |
| `v0.101.28` | publicada (2026-10-22, 8 assets) | lleva el orden del grafo filtrado y la lane de publicación reparada |
| `v0.101.29` | **publicada** (2026-10-22, 8 assets) | lleva las tres unidades nuevas del bloque B1 |

Un tag que no llegó a publicarse no se mueve. `v0.101.27` sí se publicó, tras
pasar su gate de candidato sobre el clon limpio —**6507 pruebas, 0 en rojo**—
y la lane de publicación, que tuvo que arreglar tres checks que no podían
pasar antes de llegar a publicar. Lo que se puede instalar es, por fin, el
corte que lleva el filtro de recorrido arreglado.

**`v0.101.29` es la vigente**, y no movió `v0.101.28`: un tag publicado no
se mueve. Lleva las tres unidades nuevas del bloque B1: el instalador que
ya no termina el proceso cuando la descarga se rompe, la señal de material
Rust que mira donde mira el grafo, y el grafo vacío que ya no se presenta
como un análisis completo.

## La regla que este ciclo destapó

Un gate que muere por cuota de disco **no** dice nada del código que quiere
probar, y confundirse cuesta un ciclo entero:

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
4. **La máquina decidía el veredicto.** Un repositorio de laboratorio que una
   prueba construye hereda la configuración de git de quien la ejecuta. En una
   máquina con una política de identidad, los commits del laboratorio se
   deniegan, el repositorio se queda sin historia, y las pruebas leen «0
   commits» donde había dos. Affected: nueve pruebas de Rust y cuatro
   contratos. Ninguna fallaba por el producto.
5. **`crates.io` retiró `tokio 1.54.0`** después de la última vez que `main`
   pasó su propio gate, así que `main` llevaba su comprobación en rojo sin que
   nadie lo mirase.

Y tres que encontró la propia lane de publicación, al publicar de verdad:

6. **Un gate que exige un directorio que nadie escribe.** La lane pedía un
   `dist/` que el candidato dejó de escribir en octubre. Pasaba en los
   worktrees viejos, porque el directorio viejo seguía ahí, y rechazaba
   cualquier árbol limpio con un candidato completo. Medía el worktree.
7. **El momento de la comparación es parte del contrato.** La comprobación del
   conjunto publicado corría *antes* de que se subieran los manifiestos, y
   comparaba contra un conjunto que los incluía. No era una comprobación más
   estricta: era otra comprobación, y siempre fallaba.
8. **Un 404 justo después de escribir suele ser tu propia escritura.** La
   prueba de consumidor descargaba los assets segundos después de sustituirlos;
   GitHub da id nuevo a cada asset re-subido, y el id que pedía la descarga
   ya no existía.

Un rojo de gate se lee primero: qué etapa, qué error, y si el error es del
código o de la máquina.

## Unidades abiertas

| Unidad | Qué es | Detalle |
|---|---|---|
| **B3** (`C30`, `C9`, `C16`, `C10`, `C26`, `C15`, `C27`, `C22`, `C23`) | ontología y arquitectura simplificadas | `ROADMAP.md` §11 |
| **H5** (`C11`) | `SnapshotKey` e incrementalidad honesta | `consumer-contracts/specs/SPEC-CU-005-SNAPSHOT-INCREMENTAL.md` |
| **H3** (`C12`), **H4.4** (`C13`) | navegación Kotlin sin JVM y sus relaciones con Java | `ROADMAP.md` §11 |

**C6, C31, C5, C21, C8, C7, C32, C14 y C19 están cerrados.** El lazo de feedback ejecutaba la
biblioteca del crate y nunca el test de integración que se acababa de
escribir: ahora pide el binario que produce ese fichero —y sólo ese— sin
convertir el modo rápido en la batería completa. La autoridad de una
herramienta dejó de derivarse del catálogo del proyecto analizado. La línea
base vacía dejó de obligar a reejecutar la suite entera. La configuración
ejecutable dejó de apuntar a documentos que no existen, el guard de
procedencia pasó a gobernar el contrato del consumidor, y cada superficie de
prueba tiene ahora un carril declarado o la razón escrita de por qué se
queda sin él. Y el gate da el mismo resultado en cualquier máquina: los
laboratorios de git ya no heredan las reglas de quien los ejecuta.

**C14 se cerró con `v0.101.27`**: una PR fusionada no es una release, y esta
sí se publicó, verificada desde sus propios bytes y con los binarios
instalados arrancados fuera de cualquier repositorio.

## La misión

Misión `cognicode` declarada (`goal-compass.json`), con 12 candidatos que
llevan evidencia —una ruta que existe— y un criterio de cierre observable. La
cola está escrita en el orden que fija la auditoría del 2026-10-21 (sus bloques
B1 a B6), de mayor a menor prioridad.

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

- **La lane de publicación ya no es un bloqueo, pero casi lo fue**: le pedía
  al árbol un directorio que el candidato no escribe, comparaba el conjunto
  publicado contra los manifiestos que la etapa siguiente sube, y descargaba
  los assets justo después de sustituirlos. Los tres están arreglados y
  medidos; la lane llegó a publicar y se ejecutó dos veces sobre el mismo
  tag. Este corte la vuelve a ejercitar.
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
