# Checklist mínimo de cierre de cada CU-N

- [ ] **Producto:** un consumidor demuestra la capacidad útil que prometió el bloque; diff de código y docs coincide con el alcance.
- [ ] **Integridad de análisis:** completitud, precisión, versión de proveedor, scope, permisos y errores correctos.
- [ ] **Feedback rápido:** pruebas afectadas ejecutadas y resultado registrado; `test-fast.sh` no equivale a full.
- [ ] **Gate obligatorio:** `test-full.sh` PASS previo a PR/tag; `merge-gate.pipeline.kts` e `integration.pipeline.kts` vía entorno autorizado con SHA anotado.
- [ ] **Git remoto:** PR mergeado, `origin/main` apunta a SHA efectivamente integrado; Conventional Commits, clean tree.
- [ ] **Cabecera de fusión:** el título de la PR es el mensaje del commit que la fusión crea —la forja le añade la referencia—; el juez la rechaza antes del gate si no la admite, y se corrige el título, nunca la historia ya publicada.
- [ ] **RC:** artifacts construidos y certificados en `release-candidate.pipeline.kts`; no recompilar en `release.pipeline.kts`.
- [ ] **Publicación:** release remota, tag y assets reales instalables; comparar nombres completos del inventario con lo publicado, SHA256 contra bytes descargados.
- [ ] **Consumo:** CLI y MCP binarios descargados operativos; SDK/Guard compilado o status externo `BLOCKED_EXTERNAL` claro.
- [ ] **Test UAT/E2E/AAT:** resultado con expected/actual, rc, entorno y SHA. Guardar solo metadatos útiles sin secretos.
- [ ] **Limpieza:** test suite de sesión efímera retirada después de `PROMOTE|REPLACE|RETIRE|DEFER`; tests permanentes mínimos conservados.
- [ ] **Gobernanza:** JOURNAL append-only, CURRENT regenerado, ratchet verde, roadmap principal con un solo puntero. `C8-R` human signoff inalterado sin acción explícita.
