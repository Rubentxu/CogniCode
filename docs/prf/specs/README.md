# Especificaciones de producto PRF

Normativas para capacidades **anunciadas como estables**; los detalles del cliente/flags deben inventariarse en F0, sin inventar APIs aún ausentes. Requisitos identificados PRF-CLI, PRF-MCP, PRF-ANA, PRF-STATE, PRF-SEC, PRF-EXT, PRF-DIST y PRF-CI. Aprobación y cambios de alcance se registran en [DECISIONS](../DECISIONS.md); ninguna especificación convierte automáticamente un evolutivo histórico en ACCEPTED.

| Spec | Alcance | Hito / UAT |
|---|---|---|
| [CLI](SPEC-CLI.md) | Comandos, errores, salida/compatibilidad | F0/F1/F3/F6; U01–U08,U10 |
| [MCP](SPEC-MCP.md) | JSON-RPC, lifecycle, seguridad de tools | F0/F1/F3/F5; U02,U04,U05,U08,U19,U25 |
| [Análisis](SPEC-ANALYSIS.md) | Correctitud, corpus, cobertura, basis | F2/F3; U03,U06,U08,U09,U11–U16 |
| [Estado](SPEC-STATE.md) | Persistencia, workspaces y migraciones | F4; U17,U18,U21,U22 |
| [Seguridad](SPEC-SECURITY.md) | Capabilities, rutas, secrets, budgets | F1/F5; U04,U06,U14,U19,U25 |
| [Extensibilidad](SPEC-EXTENSIBILITY.md) | Contrato de puertos/capability mínimo | F3/F5; U08,U10,U26 |
| [Distribución](SPEC-DISTRIBUTION.md) | `cogh`, bundle, plataformas y rollback | F1/F6/F7; U01,U20–U24 |
| [CI](SPEC-CI.md) | Gates por SHA, coverage y rendimiento | F0/F6/F7; U03,U12,U27 |

**Normativa de aceptación:** cada MUST se asigna a un test automático y al menos una evidencia de usuario real para operaciones core. Resultados `NOT_RUN` o `SKIP` de core no significan PASS.
