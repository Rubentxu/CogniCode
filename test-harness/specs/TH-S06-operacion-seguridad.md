# TH-S06 — Seguridad, aislamiento, coste, fiabilidad y ergonomía del agente

**Estado:** PROPOSED.

## Persistencia fuera del checkout

- Control: `$XDG_STATE_HOME/cognicode-test-harness/<repo-name>--<short-hash>/` (por defecto `~/.local/state/…`); digest basado en ruta real de checkout + origen Git si existe para discriminar clones homónimos.
- Un workspace por checkout/worktree; evidencia portable entre ellos sólo si inputs son verificablemente idénticos, no copiar directorios de recibos ciegamente.
- No crear `.harness`, `.pipelinek`, `.cache` ni *receipts* en el repo; si PipelineK requiere SQLite en `--db`, darle ruta externa explícita. Binarios Cargo siguen su configuración existente.
- El árbol `test-harness/` sí es versionado: contiene herramienta, contratos y specs, **no** datos de ejecución.
- No sobrescribir configuración `~/.cargo` ni cambiar flags globales, no limpiar target directory compartido, no forzar upgrade de toolchain sin consentimiento.

## Concurrencia y recursos

- `resource_lock`: DB, puerto, env global, checkout, contenedor, fichero de estado, `CARGO_TARGET_DIR` si hay incompatibilidad.
- Usar presupuestos `max_processes`, `max_rss_mb`, `max_wall_seconds`, `max_log_bytes`; defaults iniciales conservadores, configurables por usuario, no extraer capacidades de otro proceso.
- En Bazzite/Linux con `HOME` a menudo symlink a `/var/home`, resolver canónicos sin seguir rutas de salida fuera de state root y validar las autorizaciones.
- TTL, retención y GC nunca borran el último recibo de una release ni evidencia de rechazo sin aprobación.
- Logs pueden contener secretos: redacción, permisos `0700` directorios, `0600` ficheros, no persistir variables sensibles; por defecto no subida a servicios externos ni copias portables Nextest públicas.

## Fallos seguros

- Cortes de energía: una escritura parcial de recibo no pasa como PASS; WAL/atomic rename y pruebas SIGKILL.
- Corrupción de `state`: modo lectura/repair con auditoría; ignorar sólo caches derivables, nunca reemplazar recibo inválido por uno fabricado.
- Workspace no versionado: selección conservadora; scripts sin metadata de dependencias => no reutilizar.
- Un runner no disponible: `INFRA_ERROR` con pasos para reparar, sin degradar a `SKIP` exitoso.
- La desaparición del archivo de test o su feature guard errónea invalida el inventario de casos.
- Podman: rootless, no montar raíz del HOME, imágenes por digest y puertos/DB específicos, cleanup garantizado; tests que requieran red declaran esa capacidad.

## Operación amigable para agentes

- `plan` y `status` deben ser rápidos y no arrancar tests/cargo builds pesados.
- Salidas `--format json` con `schema_version` estable, decisiones explicables; `--format text` con resúmenes legibles para dev novel.
- Promover feedback incremental: antes de modificar código, mostrar `tests impacted`, `last failed`, `valid reused`, `expected time from measured history` y qué podría bloquear el gate.
- Al fallo: reportar test exacto, comando de reproducción, localización del log y dependientes pendientes; ofrecer `resume` **sin prometer green**.
- Actualizar automáticamente progreso de H# en informes; SDDK/journal Post-PRF existente sigue siendo fuente de estado del proyecto.

## Métricas y medición objetiva

Recopilar por suite: `wall_ms`, `cpu_ms` cuando posible, tests descubiertos, ejecutados, cache reuse, compile wait, flags, fallos, flaky classification, intentos, tiempo desde primer fallo a PASS, tiempo total de gate.

**Experimento comparativo obligatorio:** en misma máquina con condiciones comparables, tres ejecuciones: cold baseline, warm baseline, warm incremental/harness. Repetir >=3 y comparar medianas + intervalos/dispersiones. No atribuir a harness mejoras debidas sólo al cache de compilación de Cargo. Poner presupuesto de overhead de planificador y objetivo de reducción a partir de las mediciones H1, no inventar porcentajes como hecho.
