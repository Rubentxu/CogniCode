# Puntos de decisión pendientes — no inferir autorizaciones

Se especifica una trayectoria segura por defecto, sin obligar al agente a detener todo el evolutivo en W0:

| Decisión | Default seguro | Se decide antes de |
|---|---|---|
| Tecnología CLI | Python stdlib pequeño, con core puro; Rust sólo si medición y ergonomía lo justifican | W2 |
| Ledger persistente | Evaluar SQLite WAL vs JSONL append-only con corte abrupto y concurrencia; elegir una autoridad | W4 |
| Nextest | Opcional / experimental, no sustituye Cargo en release | W5 |
| Migración física de pipelines | Mantener root como autoridad, centralizar drivers de prueba bajo subárbol | W6-W7 |
| Política reuse en merge/release | OFF hasta ADR aceptada y AAT de equivalencia | W6 |
| Estado XDG | `~/.local/state/cognicode-test-harness/` (override `XDG_STATE_HOME`) | W2 |
| Limpieza de wrappers legacy | Sólo después de inventario de consumidores y paridad | W7 |
| Uso de `cargo-llvm-cov` y cache | Herramienta/profiling separados, sin reciclar PASS de tests estándar | W2-W6 |
| Fallback de dependencias desconocidas | FULL/conservador, no noop | Todos |
| Paridad de rediscovery de tests con features | Fallar si una suite cambia inesperadamente sus tests cubiertos | W3-W6 |

## Estrategia de solución de bloqueos

1. No inventar política ni editar ADR aceptada sin decisión.
2. Registrar causa/tipo (`TECHNICAL`, `INFRA`, `POLICY`, `EXTERNAL`), prueba mínima, workaround que no baje garantías y siguiente acción.
3. Avanzar WU paralelas que no dependan del bloqueo y mantener recibos de éstas; no mezclar flujos.
4. Publicar un pequeño informe al maintainer sólo cuando la elección cambie semántica, seguridad, coste material o release.
5. No cerrar bloque si las pruebas que debía demostrar figuran `NOT_RUN`.
