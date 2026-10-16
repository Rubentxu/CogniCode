# Tasks (work items dentro de cinco hitos, no micro-releases)

**Status:** [ ] = NOT_STARTED. La aprobación humana/operador y permisos de publicación siguen sujetos a la gobernanza existente. Todas las casillas se completan sobre checkout real, nunca con inferencia desde el ZIP.

## 1. B0 — Publicación verificable (1 release)

- [ ] 1.1 Verificar SHA base/HEAD remoto, tag y artefactos `latest`; registrar baseline `v0.101.17` si sigue pendiente.
- [ ] 1.2 Ejecutar pipeline existente contra candidato; corregir el primer error real sin duplicar el release factory.
- [ ] 1.3 Ejecutar pruebas efímeras B0-UAT/E2E/AAT con negativo de asset faltante/sha inválido.
- [ ] 1.4 Ejecutar regresiones permanentes, borrar efímeras, guard, full y pipeline release; merge remoto, publicación y consumidor real.

## 2. B1 — Semántica / producto veraz (1 release)

- [ ] 2.1 Probar RED: `run_petgraph_plan` vacío sin ejecutar y `QualityStore Err` en vista.
- [ ] 2.2 Introducir ADT(s) compartidos y proyecciones puras sin cambiar silenciosamente schema público.
- [ ] 2.3 Conectar plan al grafo REAL o devolver `Unsupported`; distinguir métricas ausentes y kind desconocido.
- [ ] 2.4 Aceptación efímera B1, contratos permanentes, limpieza y release; documentar breaking/semver si aplica.

## 3. B2 — Seams con valor (1 release)

- [ ] 3.1 Auditoría por *call sites* ST-03/04/05; identificar un solo owner de build/query y estados del grafo.
- [ ] 3.2 Extraer pure functions/ports mínimos, migrar consumidor real, quitar concreciones antiguas.
- [ ] 3.3 Reconciliar caches/snapshots/revisiones sin duplicar owners; tests de leyes/fitness ejecutables.
- [ ] 3.4 Aceptación efímera B2, gate completo tras borrar efímeras, merge y release.

## 4. B3 — Vertical Explorer real (1 release)

- [ ] 4.1 Sustituir scripts PG legacy por `explorer-api` LadybugDB y fixture temporal.
- [ ] 4.2 Implementar recorrido OPEN→SCAN→landing→Spotter→inspector→call graph real y estados de error.
- [ ] 4.3 Mantener MSW como UI contract separado; reducir suite de navegador a pruebas independientes significativas.
- [ ] 4.4 Aceptación efímera B3, negativo de backend desactivado, evidencia, limpieza, merge, release.

## 5. B4 — Harness central, poda y CI precisa (1 release)

- [ ] 5.1 Catalogar ejecutores (Rust/Playwright/sandbox) y todos los tests del árbol, vincular capacidades y costes medidos.
- [ ] 5.2 Implementar `plan` del selector puro, luego adaptador de ejecución; fallo cerrado ante unknown/empty/unreachable.
- [ ] 5.3 Auditar 106 candidatos más 19 adicionales; borrar legado redundante SOLAMENTE después de cobertura equivalente probada.
- [ ] 5.4 AAT de mutación de gates y UAT/E2E de consumidor; promover regresiones permanentes, borrar temporales.
- [ ] 5.5 Comparar tiempos, falsos positivos/negativos y flakiness con baseline, merge y release.

## Verificaciones transversales

- [ ] Todas las rutas mencionadas existen en HEAD o se han reconciliado, sin inventar contratos ni copiar estados de PRF.
- [ ] `python3 test-harness/tools/ephemeral_guard.py --root . --phase merge` sin efímeros; `test-harness/inventory/test_inventory.csv` clasificado (no eliminar sin evidencia).
- [ ] Tests reales que protegen seguridad, aislamiento, compatibilidad y completeness de releases siguen ejecutándose; simular fallo para demostrar que el gate cae.
- [ ] `origin/main` contiene el commit, release publicada y consumible; journal append-only, rama cerrada y siguiente trabajo seleccionable.
