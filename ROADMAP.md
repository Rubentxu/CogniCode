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
están cerradas con veredicto propio, K2 es `C13` y ya está cerrada, y K3 no
está abierta.

## Qué hay ahora

La auditoría externa del 2026-10-26 —base `main@87aab47f`— está reconciliada,
cada hallazgo medido sobre `e3832b87`. Su borrador queda como documento local
no versionado, igual que el de 2026-10-20: lo que se versiona no es la
auditoría, es lo que se decide con ella. El enrutado:

| Hallazgo de la auditoría | Dueño |
|---|---|
| H2 — el escaneo confunde un fallo de lectura con «no ha cambiado» | **`C49`**, cerrada el 2026-10-28 (`5c63e949`) |
| H4, H5, H6, H7a, H7b | `C23` (PR #464, bloqueada por el operador) |
| H3 — la resolución se decide por proximidad de ficheros | **`C13`** (`H4.4`), cerrada el 2026-10-29 (`d9bf2b45`); su residuo declarado —la pérdida de una relación no se publica— lo toma **`TASK-14`**, cerrada el 2026-10-29 (`00169311`) |
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
| Recuentos de símbolos publicados como medidos y no medidos (`symbols_removed = deleted_files.len() * 2`, `symbols_added += 1`, el `symbol_count` del manifiesto) | **`TASK-1`**, cerrada el 2026-10-28 (`ca378733`) |
| Resolución ambigua de símbolos en la capa de aplicación | **`TASK-2`**, cerrada el 2026-10-28 (`c7e5439c`) |
| Errores de arista descartados con `.ok()` | **`TASK-3`**, cerrada el 2026-10-28 (`b75d2028`) |
| La PR #464 sube la deuda CR-06 de producción de 28 a 35 pares | **`TASK-4`** — puerta medible de `C23` |
| Dos propietarios potenciales del estado del grafo | **`TASK-5`** |
| Esperas fijas en la suite E2E de Explorer (59 en 24 ficheros) | **`TASK-6`**, cerrada el 2026-10-29 (`14a67d7f`) |
| Casos desactivados en la suite E2E de Explorer (17 en 14 ficheros; la auditoría contó 16 en 13) | **`TASK-7`**, cerrada el 2026-10-29 (`9f8b2181`) |
| Catálogo público desactualizado | **`C44`**, cerrada el 2026-10-29 (`cf8ad557`) |
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

**El ciclo de TASK-14 está cerrado en el remoto.** El pull request `#492` —la
publicación de la rama `fix/the-refusal-is-published`— se fusionó por squash
como `00169311`, con el `merge-gate` verde sobre `3c86d2e8`:
`merge-gate.pipeline.kts`, 58 etapas, 0 rojas. La pérdida que el cierre
anterior dejó medida y con dueño ya no desaparece sin decirse. MEDIDO sobre el
corpus `sandbox/fixtures/kotlin-java/`: el extractor lee SEIS llamadas y el
grafo publica CUATRO relaciones; las dos que no publica son las dos que no se
pueden probar, y de esas dos no quedaba NADA publicado —la estrategia `full`
publicaba un número, `unresolved_edges`, y la estrategia `per_file` publicaba
un `BuildStatus::Complete` que afirmaba una cobertura que no tenía. Eso
contradecía K-REL-02 y K-REL-07 del contrato de Kotlin. Ahora la pérdida se
publica con su IDENTIDAD —qué llamada, en qué fichero y en qué línea— y con su
MOTIVO —cuántos candidatos tiene el nombre—, y `Complete` pasa a significar
«no quedó nada sin atar»: ni un fichero sin leer ni una referencia sin atar a
una declaración. Las dos estrategias de build publican la MISMA pérdida, porque
las dos resuelven contra el mismo índice del proyecto.

**La forma del dominio no cambia, y por eso no es una preferencia.**
`TargetResolution` sigue TENIENDO TRES variantes: una cuarta rompería el
`match` exhaustivo que el contrato de la unidad anterior fijó, y exigiría una
tabla de símbolos externos que el producto no tiene. La negativa se modela
donde ya había forma para ella —`PendingClass` y `PendingReference`—, y lo que
faltaba era el TRANSPORTE, no la forma: la arista descartada no dejaba sitio
donde escribir su motivo porque la arista no existe. El motivo lo produce la
resolución y lo decide el dominio (`PendingReference::of_call`), de modo que
ninguna ruta lo decide por su cuenta, y un valor de dominio nuevo transporta la
línea donde la llamada está escrita. No se creó ningún campo nuevo en ningún
contrato público: lo que cambia es que se llena lo que ya existía.

**Lo que este cierre no arregla, y por eso lo nombra.** La llamada sin
candidato —el nombre que el proyecto no declara— sigue siendo un número, y es
una decisión de alcance, no una omisión: publicar todos los nombres de
biblioteca llenaría la lista de huecos de ruido y la volvería inútil. El
overload correcto tampoco es implementable hoy: el pipeline sólo transporta el
NOMBRE del callee (`FileFacts.calls` es `Vec<(Symbol, String)>`), así que la
aridad que distinguiría los dos `invoke` no llega al resolvedor; está
declarado, no fingido. La equivalencia CLI/MCP medida hasta hoy cubre
Terraform, no este corpus. Y la ruta semántica —la que usa un lector con
dominio, como Terraform o Ansible— sigue sin publicar sus llamadas no
resueltas: su lector no tiene índice del proyecto, así que no puede distinguir
una llamada ambigua de una que vive fuera del proyecto.

**Y lo que este cierre le da a `C23`, sin cerrarla.** El primer criterio de
cierre de `C23` pide que las TRES rutas que construyen el grafo publiquen la
misma cobertura, las mismas relaciones y los mismos huecos sobre el mismo
corpus, y que un test lo verifique. `TASK-14` da el primer par medido de esa
clase —completa y per-file, con un test que las compara y que no puede pasar
por vacío—, pero `C23` exige las tres rutas por donde el grafo entra en
producción y además el corte del fichero de 4.369 líneas: `C23` sigue
bloqueada por el operador, y esta unidad no la desplaza.

La puerta que queda abierta es la de la cola:

- **`C23`** — la unidad de mayor retorno de todo lo que propone la auditoría
  externa. Sigue bloqueada por el operador: su pull request `#464` es la
  única de las siete que aún lleva trabajo íntegro.

Y, con `TASK-14` cerrado, la cola viva se queda **sin ninguna unidad sin
bloqueo** otra vez: `TASK-14` era el residuo que el cierre de `M2.T03` dejó
medido y con dueño, y ya está hecha. Las vecinas del entorno siguen
igualmente atadas: `TASK-5` por una decisión de alcance del operador y
`TASK-4` por `C23`. La siguiente unidad, por tanto, **no la decide este
documento**: la desbloquea el operador, y hasta entonces el trabajo de este
ciclo es el que queda medido y enrutado.
