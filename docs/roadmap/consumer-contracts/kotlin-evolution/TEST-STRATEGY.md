# Testing orientado a entrega — aplicación real de `delivery-first-testing`

## Fuente de verdad y uso de la skill

Skill verificada en `Rubentxu/agent-skill/skills/delivery-first-testing/`, con `SKILL.md`, `references/navigation.md`, `references/lens-registry.json`, y lentes `change-impact`, `ephemeral-uat`, `regression-promotion`, `merge-assurance`, `integration-assurance`, `release-assurance`, `failure-triage` y `harness-economics`, todas `ready`. Las lentes `mutation-guidance`, `historical-test-impact` y `cross-repo-consumer` están `planned`: no ejecutarlas. Router `scripts/affordances.py` es puro, **solo sugiere enlaces**, no ejecuta ni certifica. Los comandos reales se descubren en la versión de CogniCode del workspace.

## Matriz por fase — no añadir pipelines

| Momento | Intención + lentes | Qué corre | Lo que NO corre |
|---|---|---|---|
| Implementar slice dentro de Kx | `feature`: change-impact → ephemeral-uat → regression-promotion | Tests afectados, parser/solver discriminante, smoke local si cambia API; `bash scripts/test-fast.sh` | full suite por cada commit, UAT globales |
| Preparar PR | `merge`: change-impact + merge-assurance | `bash scripts/test-full.sh` **exigido por `AGENTS.md`**, `merge-gate` real, compatibilidad pública en cambios de contrato | nuevos linters/pipelines que dupliquen el actual |
| Integración real de `main` | `integration`: integration-assurance | ejecutar matriz amplia existente cuando se consuma el merge; registrar resultado y regresiones | tests efímeros de cada variante histórica |
| Candidato de release | `release`: release-assurance | full gate/RC existente, instalar el artefacto real, 1 UAT de valor de Kx, integridad **y completitud** de assets | instalar desde el código fuente y llamarlo UAT de distribución |
| Test roto/flaky | `failure-triage` | reproducir caso mínimo y clasificar PRODUCT_DEFECT/TEST_COUPLING/INFRASTRUCTURE/STALE_EVIDENCE/FLAKY | repetir full indefinidamente |
| Gate caro | `audit`: harness-economics | medir p50/p95, duplicados, falsos verdes y suites inertes | nuevo tracking paralelo |

**Router state examples** (los valores deben reflejar observaciones, no son entradas para forzar un PASS):

```json
{"intent":"feature","inventory":"ready","impact":"known","basis":"current","changed":["domain","api"],"risk":"high","affected":"not_run","uat":"pending","regression":"pending"}
```

```json
{"intent":"release","inventory":"ready","impact":"none","basis":"current","changed":["release"],"risk":"high","artifact":"ready","release_checks":"not_run"}
```

Comprobar el router local disponible de la skill, por ejemplo: `python3 <ruta-skill>/scripts/affordances.py <ruta-a-sesion.json>`. **No ejecutar el helper suponiendo que ha ejecutado pruebas**. El agente elige la lente por la pregunta observada, no por checklist automática de todas.

## Mínimo permanente por hito (solo si no hay cobertura equivalente)

- K0: 1 contrato discriminante que muestra falso `Complete` sobre Kotlin omitido y otro sobre filtrado Full/PerFile si el primero no cubre el riesgo.
- K1: 1 test de símbolos/identidad Kotlin con overload/unicode y 1 prueba del endpoint público si faltaba.
- K2: 1 test de resolución inequívoca y 1 de ambigüedad sin arista; cross-language un caso mínimo.
- K3: 1 test de provider degrade/budget y 1 de conversión UTF-16; proveedor live en UAT efímera.
- K4: 1 compat test para DTO/SCIP si hay nuevo contrato; una AAT real efímera para selección de herramientas.

No convertir todos los ejemplos en tests permanentes. Antes de promocionar, buscar la garantía equivalente ya protegida y considerar un mismo test que cubra varios invariantes.

## Ciclo de pruebas efímeras y limpieza obligatoria

1. Crear un directorio único fuera del checkout: `UAT_DIR=$(mktemp -d "${TMPDIR:-/tmp}/cognicode-kotlin-Kx-XXXXXX")` (en bash); guardar ahí mini proyectos `.kt`, `.java`, `.kts`, índices falsos, scripts MCP, logs y `evidence.json`.
2. Formular oráculo **independiente del producto** antes de ejecutar: JSON esperado y resultado `PASS/FAIL/NOT_RUN`, SHA binario, versión, scope, stdout JSON, `stderr`, duración y exactitud.
3. Ejecutar UAT/E2E/AAT solamente las de `uat/ACCEPTANCE.md` aplicables. Un gate rojo causa triaje; nunca se vuelve verde con `|| true` o aumento ciego de timeout.
4. Al cierre decidir test por test: `PROMOTE` (garantía crítica sin test), `REPLACE` (unidad más barata), `RETIRE` (exploratorio/sobreacoplado/duplicado) o `DEFER` (riesgo abierto); citar caso y resultado en **journal existente**, no abrir un nuevo ledger.
5. **Después** de journal + decisión + release real, borrar `UAT_DIR` completo con path comprobado contra el temp root. Los E2E/AAT de sesión jamás se incorporan por defecto a `crates/*/tests` ni `sandbox/manifests`. Si se necesita CI permanente, reemplazar por un único test de contrato en el módulo dueño.
6. Si el hito fracasa, el temp puede conservarse transitoriamente fuera de Git para diagnóstico; nunca borrar la única evidencia de un fallo sin registrar. En la siguiente sesión el owner lo decide y lo elimina tras promover o archivar de forma segura.

### Checklist de cierre concreto Kx

- [ ] Tests afectados PASS y razón de exclusiones/saltos.
- [ ] Full gate obligatorio PASS sobre candidato PR/RC, número de tests reales no cero.
- [ ] UAT/E2E/AAT efímeros PASS / FAIL / NOT_RUN por caso; consumidor usa binarios release.
- [ ] Registro `PROMOTE/REPLACE/RETIRE/DEFER` cerrado; ningún test crítico se perdió.
- [ ] Carpeta temporal borrada **tras** registrar evidence, o motivo `DEFER` documentado.
- [ ] PR merge-gate PASS, merge `main`, tag remoto/release assets verificados.
