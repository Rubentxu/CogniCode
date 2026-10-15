# Goals y tareas — TH-DOC-01

Todos los goals están pendientes. La apertura de esta iniciativa no modifica
el README ni certifica los gates de abajo.

## THD-G1 — Resolver publicación y ruta canónica

- [ ] **THD-0.1** Registrar si el destino es `test-harness/README.md` o una
  ruta distinta; no inferirlo a partir de la copia local.
- [ ] **THD-0.2** Acordar cómo entra el paquete al checkout versionado sin
  forzar archivos ignorados ni cambiar `.gitignore` silenciosamente.
- [ ] **THD-0.3** Confirmar que la ruta acordada aparece en un checkout limpio.

## THD-G2 — Completar un resumen exacto para el lector

- [ ] **THD-1.1** Contrastar propósito, consumidores, límites, estado y
  autoridad PipelineK con las fuentes del paquete.
- [ ] **THD-1.2** Ajustar README para separar claramente especificación,
  propuestas e implementación disponible.
- [ ] **THD-1.3** Evitar duplicar en README las reglas normativas de specs y ADRs.

## THD-G3 — Hacer navegable el paquete

- [ ] **THD-2.1** Organizar el índice según tareas del contribuidor/revisor.
- [ ] **THD-2.2** Validar cada enlace relativo desde la ruta canónica acordada.
- [ ] **THD-2.3** Confirmar que auditoría, arquitectura, TH-S01..TH-S06,
  ADRs, contratos, roadmap, UAT y AAT están cubiertos o justificar su omisión.

## THD-G4 — Cerrar integridad y aceptación

- [ ] **THD-3.1** Verificar todos los comandos contra la interfaz real; mantener
  los no disponibles como propuestas y no ejecutarlos como si existieran.
- [ ] **THD-3.2** Regenerar/verificar digest y tamaño en
  `PACKAGE-MANIFEST.json` después de cualquier edición.
- [ ] **THD-3.3** Ejecutar el recorrido de onboarding en un checkout limpio y
  registrar respuestas y enlaces fallidos.
- [ ] **THD-3.4** Adjuntar el resultado de revisión y cerrar solo con todos los
  criterios requeridos en verde.

## Gates

| Gate | Estado inicial | Evidencia necesaria |
|---|---|---|
| Ruta/publicación en checkout limpio | **BLOCKED** | THD-0.1..0.3 |
| Exactitud y honestidad del estado | NOT_RUN | Revisión contra fuentes + UAT |
| Enlaces y navegación | NOT_RUN | Validación desde la ruta canónica |
| Manifiesto | NOT_RUN | Digest/tamaño calculados sobre el README final |
| Onboarding de contribuidor | NOT_RUN | Recibo del escenario THD-3.3 |
