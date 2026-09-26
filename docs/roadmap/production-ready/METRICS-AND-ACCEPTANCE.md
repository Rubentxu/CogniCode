# Métricas de seguimiento y criterios de aceptación

| Acción | KPI | Objetivo | Método de medición |
|---|---|---|---|
| QW-01 | referencias contractuales rotas | 0 | script de existencia + `git ls-files` |
| QW-02 | estados contradictorios activos | 0 | revisión automatizada + grep de estados conocidos |
| QW-03 | bin/doc obligatorio untracked | 0 | test negativo con archivo plantado |
| QW-04 | certificaciones reproducibles | 100% | ejecutar preflight sobre SHA candidato |
| QW-05 | Actions release mutable tags | 0 | parser de workflows |
| QW-06 | tiempo hasta PR de dependency update | ≤ 7 días | updater logs |
| CR-01 | C8 clean-clone | PASS | runbook completo + SHA + checksums |
| CR-02 | constraints CP retornadas | exactamente 3 canonical IDs | integración TCP/HTTP real |
| CR-03 | etapas de e91 perfiladas | 100% del wall time atribuible | profiler/instrumentation report |
| CR-04 | p95 graph_insights | ≤ budget acordado; objetivo inicial ≤30 s fixture Tier-2 | benchmark estable |
| CR-05 | G5 scorecard | GREEN | scorecard run; streak según contrato |
| CR-06 | violations de boundary desconocidas | 0 desconocidas | self-host architecture report |
| CR-07 | RUSTSEC-2024-0437 | 0 | `cargo deny check advisories` sin ignore de ese ID |
| CR-08 | false-negative test selection | 0 | matriz de tests del selector |
| CR-09 | coverage regression | 0 en zonas gobernadas | `cargo llvm-cov` baseline/delta |
| ST-01 | imports application→MCP en FileOperations | 0 | architecture fitness + grep secundario |
| ST-02 | concreciones creadas dentro de WorkspaceSession | 0 para adapters objetivo | architecture test/constructor inspection |
| ST-03 | responsabilidades principales de AnalysisService | ≤ facade + módulos internos definidos | API inventory + tests |
| ST-04 | campos públicos HandlerContext | tendencia a 0; objetivo fields privados | rustdoc/API diff |
| ST-05 | propietarios de full graph semantics | 1 | contract mapping + code ownership |

## Definition of Done por acción

Una acción no está DONE hasta que cumple las cinco condiciones:

1. cambio implementado o decisión explícitamente registrada;
2. test/evidencia que demuestra la propiedad corregida;
3. tests afectados verdes;
4. documentación/roadmap reconciliados;
5. no queda un nuevo workaround sin owner/expiry.

## KPIs del programa

- **Reproducibility rate:** 100% de certificados repetibles desde SHA limpio.
- **Architecture boundary coverage:** 100% de boundaries críticos expresados como rules ejecutables.
- **Performance:** G5 GREEN en la línea v1.0 antes de release candidate.
- **Security debt:** 0 advisories vulnerables sin excepción explícita y temporal.
- **CI precision:** ≥90% de PRs ejecutan suites afectadas sin full suite innecesaria; 0 false negatives conocidos.
- **Flake rate:** <0,5% en suites obligatorias durante 20 ejecuciones consecutivas.
- **Public surface reduction:** HandlerContext y composition APIs disminuyen, no crecen, durante Fase 3.
