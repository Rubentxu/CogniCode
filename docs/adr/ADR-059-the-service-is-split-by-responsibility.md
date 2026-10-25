# ADR-059 — El servicio de analisis se parte por responsabilidad, y cada parte tiene su nombre

**Estado**: EJECUTADO (decidido el 2026-10-25; ejecutado el 2026-10-25)
**Fecha**: 2026-10-25
**Decisores**: Maintainer
**Contexto**: C23 · el servicio de analisis (`crates/cognicode-core/src/application/services/analysis_service.rs`), candidato `C23` de la cola de la mision

---

## Contexto

### Lo que dice la cola

`C23` es el candidato que quedo vivo del bloque de arquitectura: *«el servicio de
analisis deja de ser un modulo de 4.100 lineas»*. Su criterio de cierre tiene
cuatro partes, y la primera ya estaba cerrada antes de abrir nada:

1. las tres rutas que construyen el grafo publican la misma cobertura, las mismas
   relaciones y los mismos huecos sobre el mismo corpus, y un test lo verifica;
2. **el archivo se parte por responsabilidad, y cada parte tiene su nombre**;
3. los contratos publicos del servicio no cambian;
4. la suite sigue verde.

La primera la midio y la cerro el ciclo anterior: la red de equivalencia
(`crates/cognicode-core/tests/h24_the_routes_say_the_same.rs`) mide que las tres
vias contestan lo mismo. Esta unidad ejecuta la segunda, y las otras dos son sus
guardas.

### Lo que el arbol muestra

MEDIDO sobre el arbol: el fichero tiene **4.354 lineas** y dentro vive el
servicio entero. La produccion termina en la linea 2.086; las otras 2.268 son
un modulo de pruebas escrito en linea. Las responsabilidades y sus lineas:

| Responsabilidad | Metodos | Lineas |
|---|---|---|
| Construccion del grafo | `build_project_graph`, `build_project_graph_filtered`, `build_project_graph_async`, `build_graph_per_file*`, `build_graph_lightweight`, `build_graph_with_strategy`, `build_full_graph` | 1.266 |
| Cache e indice | `set_symbol_index`, `has_symbol_index`, `ensure_symbol_index`, `invalidate_file_cache_for` | 60 |
| Resolucion | `find_symbol`, `find_symbol_usages`, `get_file_symbols`, `get_all_symbols`, `trace_path` | 279 |
| Metricas | `get_graph_stats`, `get_coverage_metrics`, `get_last_build_report` | 85 |
| Analisis del dominio | ciclos, impacto, complejidad, codigo muerto, dependencias entre modulos | 253 |

Cinco cosas que no se hablan entre si, en un fichero, con el indice de simbolos
y la cache de ficheros en un lado y el recorrido de ficheros en el otro. Un
cambio en la cache obliga a leer el constructor del grafo, y un cambio en las
metricas obliga a leer el lector de ficheros. El coste no es el tamano: es el
tiempo de lectura que cada cambio gasta en codigo que no tiene nada que ver con
el.

### La restriccion que no es de esta unidad

El servicio tiene un patron de dependencia que el guard arquitectonico mide:
`application` no debe alcanzar `infrastructure` directamente, y hoy lo hace en
tres sitios. El inventario de esa deuda es
`crates/cognicode-core/src/application/architecture/cr06_allowlist.rs`, y su
ratchet (`scripts/ci/test_cr06_ratchet.py`) mide la **deuda de produccion** por
par `(fichero, dependencia)` y no permite que crezca sin una aprobacion
nombrada.

Eso tiene una consecuencia que hay que decir antes de decidir nada: **partir
un fichero acoplado en varios sube ese numero aunque el acoplamiento sea
identico**, porque la entrada esta indexada por fichero. El acoplamiento no
crece; su distribucion si.

### Lo que ya existe y no se reabre

El programa que fijo ese patron (`ST-01..ST-05`, en
`docs/roadmap/production-ready/`) esta sellado como historico. Su accion
`ST-03` —«separar los modulos del servicio de analisis detrás de una
fachada profunda»— nombra exactamente este trabajo, y el inventario de la
deuda le asigna las tres entradas de este servicio. **Ese programa no se
reabre**: su detalle no se modifica y no se justifica trabajo nuevo contra el.
Lo que se decide aqui es una unidad de la cola viva, con su propio criterio; la
forma que este ADR fija se apoya en lo que el programa historico dejo medido
(que el acoplamiento es real, y que el remedy de verdad son los puertos), no
en su agenda.

---

## Decision

**El servicio se parte en un modulo por responsabilidad, y el modulo raiz se
queda con el estado y los constructores.**

```
application/services/analysis_service.rs                  — la fachada
application/services/analysis_service/construction.rs      — construir el grafo
application/services/analysis_service/cache.rs             — indice y cache de ficheros
application/services/analysis_service/resolution.rs        — resolver simbolos
application/services/analysis_service/metrics.rs           — medir el grafo
application/services/analysis_service/analysis.rs          — analizar el grafo
application/services/analysis_service/tests.rs             — las pruebas del modulo
application/services/analysis_service/find_symbol_usages_tests.rs
application/services/analysis_service/reader_failure_tests.rs
```

La fachada son 115 lineas: el estado del servicio, sus dos constructores, sus
accesores y las declaraciones de los modulos. El modulo mas grande se queda en
1.386 lineas —el que construye el grafo—, que es donde vive el trabajo de
verdad.

### Tres decisiones que la forma lleva dentro

**1. Cada modulo declara sus propias dependencias de infraestructura.** No se
enrutan por `super::`. La razon es medida, no estetica: el evaluador
arquitectonico solo ve las rutas `crate::...`, asi que un salto por `super::`
**volveria a ciegas al guard** el acoplamiento que existe. Ese agujero ya se
cerró una vez, cuando el guard aprendio a ver la dependencia escrita donde se
usa y no solo la importada. Un guard que no ve el acoplamiento es peor que no
tener guard: parece que vigila. El acoplamiento se ve, en el modulo que lo
tiene.

**2. Los modulos de pruebas son ficheros aparte, y cada uno lleva su ambito
`#[cfg(test)]` dentro.** El clasificador del ratchet decide si una entrada es
deuda de produucion leyendo el fichero: un `use` fuera de un ambito `#[cfg(test)]`
es deuda, y dentro no lo es. Un fichero de pruebas que declara sus imports
arriba, sin el ambito, seria contado como deuda de produccion aunque no lo
sea. Por eso los tres ficheros de pruebas envuelven su contenido en
`#[cfg(test)] mod body { ... }`: el clasificador los ve como lo que son, y el
ratchet los excluye del numero —16 de las 51 entradas son de solo prueba.

**3. Los contratos publicos no cambian.** La ruta publica del servicio sigue
siendo `application::services::analysis_service::AnalysisService`, y los tipos
publicos que se movieron (`UsageSearchParams`, `UsageResult`, `ContextData`) se
reexportan desde la fachada. La unica ampliacion de visibilidad es
`pub(super)` en dos sitios: el indice de simbolos, que consulta la resolucion, y
el lector de ficheros, que usa una prueba.

### Lo que el inventario de la deuda hace con esto

Las tres entradas que tenia el servicio se quedan en el fichero que las tiene y
siguen a su dueño. Las nueve que nacen de la particion se nombran en el
inventario con el mismo dueño y la misma razon, y el ratchet registra la
aprobacion de cada par por nombre. **La deuda de produccion medida va de 28 a
35**, y el motivo esta escrito en el propio ratchet: el acoplamiento no cambio,
lo que cambio es que un fichero son seis y la entrada esta indexada por
fichero.

### Alternativas consideradas

**Mover los tipos detras de un puerto (el alcance completo del recorte
historico).** Es la unica forma de que el numero baje de verdad, porque saca
la dependencia del sitio en vez de repartirla. Se descarto **en esta unidad**
por dos razones, y las dos son del contrato: cambiaria las firmas publicas
(`find_symbol` devuelve `Vec<SymbolLocation>`, que es un tipo de
infraestructura), y el criterio de cierre dice que los contratos publicos no
cambian. Queda como unidad propia, con su propio criterio.

**Enrutar los tipos por `super::` para que el numero no se moviera.** Se
descarto por la razon de arriba: es exactamente el agujero que el guard ya
cerro una vez. Habria hecho subir el codigo y bajar el numero, que es la
direccion opuesta a la que sirve un guard.

**Partir en menos modulos para reducir el crecimiento.** Se descarto: el
crecimiento del inventario es una consecuencia de medir por fichero, y
reducirlo solo parte menos. Con tres modulos el numero sube menos, pero el
servicio queda menos navegable y el criterio «cada parte tiene su nombre» se
cumple peor.

**Dejar las pruebas en linea.** Se descarto: la mitad del fichero eran
pruebas, y el coste de leer un modulo es leer el codigo que no tiene que ver
con el cambio.

---

## Consecuencias

### Positivas

- El servicio se lee por partes: un cambio en la metricas no exige abrir el
  constructor del grafo.
- La fachada queda en 115 lineas y el modulo mas grande en 1.386, frente a
  4.354 en un solo fichero.
- Las pruebas salen del modulo de produccion: 2.268 lineas que no estorban a
  quien lee el servicio.
- El inventario de la deuda queda alineado con el codigo: cada entrada
  describes una dependencia real, en el fichero que la tiene.

### Negativas

- **La deuda de produccion del ratchet sube de 28 a 35.** El acoplamiento es
  el mismo; el numero mide ficheros. La aprobacion queda escrita y nombrada.
- Los metodos privados que cruzan de responsabilidad pasaron a `pub(super)`.
  Es visible dentro del modulo y no fuera, asi que la superficie publica no
  cambia, pero el interior es menos cerrado que antes.
- Los modulos de pruebas usan rutas absolutas (`crate::application::...`) en
  vez de `super::`, porque el nivel de anidamiento cambio. Es mas verboso y es
  a prueba de que el troceo vuelva a cambiar.

### Mitigaciones

- La red de equivalencia de las tres vias (`h24_the_routes_say_the_same.rs`) y
  el contrato de comportamiento de esta unidad cubren lo que el troceo podia
  romper.
- Un contrato nuevo mide la forma: cada responsabilidad en su fichero, y el
  fichero que declara el servicio sin implementar ninguna.

---

## References

- `crates/cognicode-core/src/application/services/analysis_service.rs` y su
  directorio `analysis_service/`
- `crates/cognicode-core/tests/c23_the_responsibilities_still_answer_together.rs`
- `scripts/ci/test_the_service_is_split_by_responsibility.py`
- `crates/cognicode-core/src/application/architecture/cr06_allowlist.rs`
- `scripts/ci/test_cr06_ratchet.py`
- `docs/adr/ADR-056-the-routes-say-the-same.md` — la red de equivalencia que
  hace segura esta extraccion
