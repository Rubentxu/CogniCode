# CogniCode — TODO de la revisión externa (reconciliado)

> **Este fichero no es una autoridad de agenda.** Es el slot donde cae el TODO
> que llega de una revisión externa, hasta que se reparte entre las unidades
> que ya existen.
>
> - **La única autoridad de agenda** es `docs/roadmap/ROADMAP.md` §11.
> - **El recibo append-only** de cada unidad está en `docs/roadmap/JOURNAL.md`.
> - **La cola de trabajo de la misión** está en `goal-compass.json`.
> - **Cómo se decide qué se toca** está en `AGENTS.md`.

## Qué había aquí

Hasta `e3832b87` este fichero contenía el TODO de la revisión de código remoto
de Kotlin, **byte a byte idéntico** a
`docs/roadmap/consumer-contracts/kotlin/KOTLIN-LINE-K0-K3.md` —la copia
canónica, que es la que manda y que sigue en el árbol. Nada se pierde al
sustituirlo: la línea Kotlin ya está integrada en el roadmap vivo. K0 y K1
están cerradas con veredicto propio, K2 es `C13` en la cola, y K3 no está
abierta.

## Qué hay ahora

La auditoría externa del 2026-10-26 —base `main@87aab47f`— está reconciliada,
cada hallazgo medido sobre `e3832b87`. Su borrador queda como documento local
no versionado, igual que el de 2026-10-20: lo que se versiona no es la
auditoría, es lo que se decide con ella. El enrutado:

| Hallazgo de la auditoría | Dueño |
|---|---|
| H2 — el escaneo confunde un fallo de lectura con «no ha cambiado» | **`C49`**, cerrada el 2026-10-28 (`5c63e949`) |
| H4, H5, H6, H7a, H7b | `C23` (PR #464, bloqueada por el operador) |
| H3 — la resolución se decide por proximidad de ficheros | `C13` |
| H1 — ventana entre verificar y publicar | declarado, sin unidad: pide una decisión de alcance |
| H8 — el refactor construye grafos temporales | declarado, sin unidad: pide una decisión de alcance |
| H7c — el presenter reconstruye datos parseando texto | residuo dentro del bloque de `C23` |
| «Kotlin sigue declarado como no soportado» | **falso en este árbol**: `KNOWN_UNSUPPORTED_EXTENSIONS` está vacío desde K1 (`4920517b`) |

La reconciliación de las siete pull requests que seguían abiertas está medida
por SHA de blob contra `origin/main`: **#465–#470 ya están dentro de `main`** —
via el squash `#471` (`8689de6e`), y #470 además via `#472` (`c87340f0`)—, y
**#464 es `C23`**, sin ni un solo fichero en común con `main`. La causa está en
el historial: `#471` fue un squash de seis pull requests a la vez, así que sus
ramas quedaron viejas, no su trabajo pendiente.

## La auditoría del 2026-10-28

La segunda auditoría externa —base `main@2eef9d4f`— se reconcilió el mismo día,
cada hallazgo medido contra el árbol real (`main@5c63e949`). Su borrador queda
como documento local no versionado, igual que los anteriores. El enrutado:

| Hallazgo de la auditoría | Dueño |
|---|---|
| Lecturas silenciadas en el escaneo | **`C49`**, cerrada el 2026-10-28 (`5c63e949`) |
| `symbols_removed = deleted_files.len() * 2` — un recuento publicado como medido que no lo está | **`TASK-1`** |
| Resolución ambigua de símbolos en la capa de aplicación | **`TASK-2`** |
| Errores de arista descartados con `.ok()` | **`TASK-3`** |
| La PR #464 sube la deuda CR-06 de producción de 28 a 35 pares | **`TASK-4`** — puerta medible de `C23` |
| Dos propietarios potenciales del estado del grafo | **`TASK-5`** |
| Esperas fijas en la suite E2E de Explorer (59 en 24 ficheros) | **`TASK-6`** |
| Casos desactivados en la suite E2E de Explorer (16 en 13 ficheros) | **`TASK-7`** |
| Catálogo público desactualizado | `C44` |
| Acoplamiento aplicación↔infraestructura, `views.rs` (9 669 líneas), `AnalysisService` (4 517) | `C23` |
| TOCTOU entre verificar y publicar (`file_operations.rs:1281-1287`) | declarado, sin unidad: pide una decisión de alcance |
| Duplicidad de las specs de perspectiva de Explorer | **no se reproduce**: `perspective-toggle.spec.ts` prueba P2.1–P2.6 y `perspective-toggle-full.spec.ts` tiene un test de visibilidad y otro desactivado con `[DEBT: …]`; el caso desactivado es justamente el que la auditoría llama duplicado |
| Plan de cinco fases | no entra: su bloque de mayor retorno es `C23`, que ya está en la cola |
| Propuestas laterales A–D | `DRAFT-1` a `DRAFT-4`: son propuestas, no trabajo hasta que alguien las mida |

`TASK-1` a `TASK-7` y `DRAFT-1` a `DRAFT-5` viven en la bandeja local de
Backlog.md (`.backlog/`, fuera de Git por la política de documentos privados).
Sus identificadores y su recibo están en `docs/roadmap/JOURNAL.md`. La bandeja
no es una segunda agenda: la cola viva sigue en `goal-compass.json` y la
autoridad de agenda en `docs/roadmap/ROADMAP.md` §11.

## Lo que este fichero NO decide

- **No abre un programa nuevo.** El bloque de mayor retorno que propone la
  auditoría —una autoridad única de inventario y snapshot— es `C23`, que ya
  está en la cola con prioridad 50. Ejecutarlo dos veces sería el segundo
  roadmap que este proyecto no admite.
- **No reabre Kotlin ni el programa PRF.** Sus cierres están escritos y sus
  mediciones son evidencia histórica.
- **No fija la prioridad de la cola.** La prioridad es del operador; la
  medición de este documento la justifica, pero no la decide.

## La puerta que queda abierta

**El ciclo de C49 está cerrado en el remoto.** El pull request `#475` —la
publicación de la rama `feature/the-scan-declares-what-it-cannot-read`— se
fusionó por squash como `5c63e949`, con el `merge-gate` verde sobre
`7f6a110d`: `merge-gate.pipeline.kts`, 58 etapas, 0 rojas. El escaneo ya
declara los ficheros que no puede leer: un fichero ilegible se publica como
fallo, sin hash y con su causa; el índice guarda la ausencia; y la segunda
pasada vuelve a mirarlo.

La puerta que queda abierta es la de la cola:

- **`C23`** — la unidad de mayor retorno de todo lo que propone la auditoría
  externa. Sigue bloqueada por el operador: su pull request `#464` es la
  única de las siete que aún lleva trabajo íntegro.
- **`C44`** — el catálogo publicado dice de su runtime una versión que no es
  la. Prioridad 15, sin bloqueo.
- **`C13`** — las relaciones Kotlin↔Java sólo cuando se pueden probar, con un
  corpus de homónimos y sin ninguna arista bornada por coincidencia de nombre.
  Prioridad 10, sin bloqueo.
