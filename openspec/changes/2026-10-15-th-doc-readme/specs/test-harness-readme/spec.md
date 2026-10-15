# Spec — README del Test Harness

## Requirement 1 — Publicación desde checkout limpio

El README solo puede declararse completo cuando su ruta canónica forma parte
del checkout distribuible. Un archivo existente únicamente bajo
`docs/test-harness/` mientras el directorio siga ignorado no satisface este
requisito.

### Scenario: el paquete sigue ignorado

- **GIVEN** `docs/test-harness/` está excluido por `.gitignore`
- **WHEN** se evalúa el gate THD-G1
- **THEN** la unidad queda bloqueada y no fuerza la inclusión del paquete ni
  altera el ignore global por inferencia

### Scenario: ruta canónica acordada

- **GIVEN** la ubicación se ha resuelto y está disponible en un checkout limpio
- **WHEN** se evalúa el gate THD-G1
- **THEN** la ruta registrada, el puntero del ROADMAP y los enlaces del README
  coinciden

## Requirement 2 — Estado y capacidades honestos

El README MUST distinguir la especificación del Test Harness de una
implementación ejecutable. Los comandos propuestos MUST seguir marcados como
propuestos hasta que el flujo real correspondiente se haya implementado y
aceptado mediante UAT.

### Scenario: lector encuentra una interfaz propuesta

- **GIVEN** la interfaz del harness aún no está implementada
- **WHEN** un lector revisa ejemplos o comandos
- **THEN** el README no afirma que estén disponibles y explica el estado real

## Requirement 3 — Navegación trazable

El README MUST orientar por tareas y enlazar solo a artefactos existentes en
la ubicación canónica. Los contratos de comportamiento permanecen en sus
specs/ADRs; el README los resume sin duplicar su autoridad.

### Scenario: lector sigue una ruta de lectura

- **GIVEN** un checkout limpio con el paquete publicado
- **WHEN** un contribuidor sigue los enlaces del README a auditoría,
  arquitectura, specs, ADRs, contratos, roadmap, UAT y AAT
- **THEN** cada enlace relativo resuelve a su destino y el lector identifica
  qué artefacto es normativo para cada tema

## Requirement 4 — Integridad del paquete

Si el README cambia, `PACKAGE-MANIFEST.json` MUST reflejar su digest y tamaño
reales. Las afirmaciones de operación, seguridad, release y autoridad MUST
tener una fuente local identificable o permanecer explícitamente como
propuestas/pendientes.

### Scenario: cambia el README

- **GIVEN** el contenido final del README se ha modificado
- **WHEN** se prepara el paquete
- **THEN** el digest/tamaño del manifiesto coincide y no se presenta evidencia
  de implementación como evidencia solo documental

## Requirement 5 — Aceptación por un lector nuevo

La aceptación MUST ejercitar el README desde un checkout limpio, no solo
comprobar que el archivo local se renderiza.

### Scenario: recorrido de onboarding documental

- **GIVEN** un contribuidor sin contexto previo
- **WHEN** usa el README para determinar propósito, estado, límites, primer
  documento de lectura y capacidades aún no disponibles
- **THEN** puede responder esas preguntas con enlaces resolubles y sin ejecutar
  una interfaz ficticia
