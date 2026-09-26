# Plan de ejecución

## Catálogo de acciones

| ID | Hallazgo asociado | Acción | Ubicación | Esfuerzo | Prioridad | Dependencia |
|---|---|---|---|---:|---|---|
| QW-01 | Expediente C8 ausente | Crear/versionar expediente C8 y eliminar referencias rotas | `docs/roadmap/certifications/` | 2–4 h | P0 | — |
| QW-02 | Drift documental | Reconciliar ROADMAP/JOURNAL/MAINTENANCE, archivar e90 y declarar e91 | `docs/roadmap/`, `openspec/changes/` | 4–6 h | P0 | QW-01 |
| QW-03 | `.gitignore` demasiado amplio | Añadir prueba que falle si source/docs obligatorios quedan ignored/untracked | `.gitignore`, `scripts/ci/` + test contractual | 4–6 h | P0 | — |
| QW-04 | Certificación desde working tree | Implementar preflight `clean clone + clean tree + referenced-files tracked` | `scripts/ci/` | 1 d | P0 | QW-03 |
| QW-05 | Actions mutables | Pinar Actions del release por SHA documentado | `.github/workflows/release*.yml` | 3–5 h | P2 | — |
| QW-06 | Sin updater de dependencias | Añadir Dependabot/Renovate solo para Cargo/GHA con cadence controlada | `.github/dependabot.yml` o equivalente | 2–4 h | P3 | QW-05 |
| QW-07 | `.env` trackeado | Sustituir `.env` versionado por `.env.example`/config local | raíz repo | 1–2 h | P3 | — |
| CR-01 | C8 no reproducible | Recertificar C8 desde clone limpio sobre nuevo SHA congelado | runbook C8 | 1–2 d | P0 | QW-01..04 |
| CR-02 | CP evidencia insuficiente | UAT exige exactamente 3 canonical constraints + respuesta coherente | CP integration/UAT | 0.5–1 d | P0 | CR-01 preparación |
| CR-03 | e91 G5 RED | Perf profiling por etapa con fixture multi-repo | e91 / graph insights | 1–2 d | P0 | QW-02 |
| CR-04 | e91 G5 RED | Implementar optimización mínima basada en profiling | community/graph insights | 2–4 d | P0 | CR-03 |
| CR-05 | Sin perf regression gate | Test budget + scorecard y streak válido | e91 + CI perf lane | 1–2 d | P0 | CR-04 |
| CR-06 | CP no protege application | Añadir fitness functions `application_no_infrastructure` y `application_no_interface` | architecture constraints | 1–2 d | P1 | — |
| CR-07 | Advisory protobuf | Migrar OTel 0.27→0.28 y eliminar `RUSTSEC-2024-0437` | Cargo manifests/metrics | 1–3 d | P1 | — |
| CR-08 | PR-CI no sensible a paths | Selector determinista de suites afectadas + fallback seguro | `.github/workflows/pr-ci.yml`, scripts CI | 2–3 d | P1 | CR-06 |
| CR-09 | Coverage sin gobernanza | Rebaseline HEAD + policy no-regression + cobertura crítica | CI/coverage | 1–2 d | P2 | CR-08 |
| ST-01 | FileOperations depende de MCP/infra | Extraer `PathPolicy`, parser/filesystem/verifier ports | application/file operations | 2–4 d | P1 | CR-06 |
| ST-02 | WorkspaceSession compone concreciones | Mover wiring a composition root; inyectar capacidades | workspace/runtime | 3–5 d | P1 | ST-01 |
| ST-03 | AnalysisService mega-módulo | Separar GraphBuild/GraphQuery/Coverage/SymbolIndex detrás de facade profunda | application/services | 5–8 d | P1 | CR-06, ST-02 |
| ST-04 | HandlerContext service locator | Privatizar campos y pasar capabilities/facades por grupo de handlers | interface/mcp | 4–6 d | P1 | ST-01, ST-03 |
| ST-05 | Múltiples graph-build semantics | Designar un único owner semántico y retirar/encapsular rutas divergentes | graph/application | 4–6 d | P1 | ST-03 |

## Acciones detalladas

### QW-01 — Expediente C8 versionado
**Qué:** materializar el expediente que ROADMAP/JOURNAL ya tratan como autoridad de firma.  
**Cómo:** crear el documento bajo una ruta no ignorada, incluir SHA, comandos reproducibles, resultados y decisión de firma. Añadir test de existencia de todos los archivos enlazados por el bloque C8.  
**Aceptación:** ningún path contractual del ROADMAP resuelve a 404.

### QW-02 — Reconciliación documental
**Qué:** eliminar estados contradictorios de SemVer, e90/e91 y F0.1.  
**Cómo:** una única pasada de reconciliación; no reescribir historia, solo actualizar estado vigente y mover e90 a archive si su propio addendum lo declara cerrado.  
**Aceptación:** ROADMAP es suficiente para responder “qué sigue” sin consultar documentos históricos.

### QW-03 — Guard de archivos ignorados/untracked
**Qué:** impedir otra pérdida silenciosa como `control_plane.rs`.  
**Cómo:** test contractual que enumere bin targets y documentos obligatorios y verifique `git ls-files --error-unmatch`; adicionalmente revisar negaciones necesarias de `.gitignore`.  
**Aceptación:** plantar un bin declarado pero untracked hace fallar el test.

### QW-04 — Certification preflight
**Qué:** convertir clean-clone en una propiedad verificable.  
**Cómo:** script que clone/checkout SHA en tempdir, valide tree limpio, compile targets declarados y compruebe referencias de certificación.  
**Aceptación:** el script falla si hay dependencia de un archivo local no versionado.

### CR-01 — C8-R
Ejecutar exclusivamente el runbook `runbooks/C8-RECERTIFICATION-RUNBOOK.md`. No reutilizar el resultado C8 antiguo como certificado final.

### CR-03/04/05 — e91
El profiling decide el cambio. No crear `InsightCache` por defecto. Optimizar primero el coste dominante y fijar un budget de regresión sobre fixture estable.

### CR-06 — Architecture fitness functions
Añadir reglas ejecutables antes de refactorizar. El primer resultado esperado puede ser RED: ese RED es inventario de deuda, no fallo del plan. Crear allowlist temporal únicamente si contiene owner, motivo y fecha de retirada.

### CR-08 — CI adaptativa
El selector no debe inferir solo por extensión. Debe mapear paths a suites declaradas, con `unknown -> safe fallback`. Cambios en Cargo/workflows/architecture/release fuerzan suites amplias.

### ST-01..05 — arquitectura emergente
No ejecutar como big-bang. Cada slice debe:
1. añadir seam/test caracterizador;
2. migrar un consumidor real;
3. eliminar la dependencia concreta;
4. comprobar que la interfaz nueva es menor que la complejidad ocultada;
5. evitar crear “ports” sin más de un consumidor real o sin sustitución útil.
