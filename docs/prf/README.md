# CogniCode PRF — fuente de verdad del programa

**Estado inicial:** PLANIFICADO; NO CERTIFICADO. **Baseline de planificación:** `0903108fc372766a69a89a76ed7f79ffff90502d` (2026-09-20). Este paquete no modifica código ejecutable, ni acredita que los tests pasen.

**Producto objetivo:** `cognicode` (CLI de análisis) + `cognicode-mcp` (MCP stdio) + `cogh` (gestor de instalación/versión). Linux GNU x86_64 y aarch64 como candidatas iniciales a soporte, sujeto a prueba de artefactos reales. `explorer-mcp` y `explorer-api`: inventariar compatibilidad y deprecación antes de decidir; no borrarlos ni romper contratos existentes.

## Lee en este orden

1. [AGENTS.md](../../AGENTS.md): protocolo obligatorio de recuperación y cierre de cada sesión.
2. [Estado/puntero](STATE.md): único marcador de hito y unidad de trabajo activa; no confundir planes con hechos.
3. [Diario](JOURNAL.md): eventos append-only, evidencias, SHA, riesgos y siguiente acción.
4. [Roadmap secuencial](ROADMAP.md) y [certificaciones](CERTIFICATION.md): prioridades y criterios de salida innegociables.
5. [Especificaciones](specs/README.md), [UAT](UAT.md), [arquitectura](ARCHITECTURE.md), [decisiones propuestas](DECISIONS.md).
6. [Trazabilidad y legado](TRACEABILITY.md), [histórico](../historico/README.md).

**Regla:** `IMPLEMENTED` ≠ `INTEGRATED` ≠ `ACCEPTED` ≠ `RELEASED`. Un ciclo archivado no demuestra funcionamiento de producto. Cada requisito debe enlazar prueba real y SHA.

El paquete PRF de planificación anterior (33 documentos, 2026-09-21) es material fuente; ESTE directorio del repositorio es la referencia operativa consolidada. No mantener dos roadmaps activos.

## Alcance consciente

CLI/MCP local-first y sin servicios de red/telemetría obligatorios para arrancar; análisis y refactor con autorización diferenciada; estado recuperable; contratos estables y extensibles mediante un solo caso de uso compartido. No prometer CLI/MCP idénticos en comandos o esquemas, sino igualdad de semántica para capacidades equivalentes.

**Fuera del camino crítico:** gRPC/daemon RPC, Control Plane/Backstage, packs dinámicos, AI autónoma, federación y migración LSI integral. Solo reabrir con consumidor real, ADR nuevo, impacto en gates y un requisito PRF explícito.

## Política documental

No mover ni reescribir `openspec/specs`, `openspec/changes`, ADR aceptados o documentos utilizados por scripts: forman el registro contractual/histórico. Los planes antiguos de nivel superior se conservan intactos en [docs/historico](../historico/README.md) con redirects de compatibilidad. Nunca borrar evidencia ni reescribir el pasado para aumentar un porcentaje.
