# Estrategia de aceptación CU — delivery-first-testing (router y lentes reales)

## Fuente de la skill

`Rubentxu/agent-skill/skills/delivery-first-testing/SKILL.md` (consultada 2026-10-16). Rutas efectivamente existentes:

- Router `SKILL.md` y `references/navigation.md`; `references/lens-registry.json`, `references/skill-registry.json`.
- `change-impact`, `ephemeral-uat`, `regression-promotion`, `failure-triage`, `merge-assurance`, `integration-assurance`, `release-assurance`, `harness-economics`, todos `status=ready`.
- `cross-repo-consumer` aparece **planned**, NO es lente disponible. Puede orientarse manualmente mediante `integration-assurance` y `release-assurance`; no fingir que el router la puede ejecutar.

El helper opcional `scripts/affordances.py` es pura recomendación, **no ejecuta ni certifica pruebas**. Si la skill instalada no está presente en el cliente del agente, consultar su repo y aplicar la misma política manualmente, sin decir que se invocó.

## Router por fase

| Momento | Intención del router | Lentes que sí existen | Verificación mínima |
|---|---|---|---|
| Implementar comportamiento | `feature` | `change-impact` → `ephemeral-uat` → `regression-promotion` | test unitario afectado + un contrato de seam |
| Preparar PR | `merge` | `change-impact` + `merge-assurance` | selector y `bash scripts/test-full.sh` por política actual del repo; `merge-gate.pipeline.kts` |
| Integración | `integration` | `integration-assurance`, si rojo `failure-triage` | `integration.pipeline.kts`, tests amplios y frontera consumers |
| Candidato/release | `release` | `release-assurance` | binarios reales, SHA, inventory completo, install/update y consumidor |
| Borrar harness obsoleto | `audit` | `harness-economics` y `regression-promotion` | prueba equivalente, coste/beneficio medidos, sin borrar seguridad/durabilidad |

## Presupuesto de pruebas persistentes y efímeras

**Cada bloque:** máximo **1 UAT funcional + 1 AAT negativa + 1 E2E de transporte/distribución si no queda cubierto por UAT**. Es una guía de mínimo, **no techo contra riesgos críticos**. No replicar UAT por cada work unit. Mantener aproximadamente 1–2 tests unitario/contrato por invariante fundamental, reutilizando los ya existentes en vez de sumar 8×2 sin investigar cobertura.

- **Permanentes inevitables:** leyes `Partial` no vacío, scope de evidencia, invalidación manifest, permisos/authority, esquema/salida, inventory completeness (cuando no exista test equivalente). Elegir nivel dominio/contrato más barato.
- **Efímeros en ruta fuera del repositorio**, preferible `${TMPDIR:-/tmp}/cognicode-cu-$BLOCK-$SHA-XXXX`; usar `mktemp -d`; no guardar con `git add` tests UAT/E2E/AAT de sesión.
- Test efímero = comandos reales, fixtures pequeños generados, timeout, comprobación del output y `exit code`; nunca cuenta como aprobado por mirar una doc o porque el bin no está instalado.
- **No eliminar seguridad, concurrencia, persistencia, integridad, aislamiento o mecanismos de recuperación sin cobertura equivalente**. Si E2E encontró bug crítico, `PROMOTE` la mínima ley que lo detecta antes de `RETIRE` el guion de sesión.

## Cierre y borrado obligatorio al final de CU-N

1. Enumerar UAT/E2E/AAT temporales creados por la sesión, resultado ejecutado `PASS/FAIL/BLOCKED/NOT_RUN` con fuente de evidencia (SHA + artefacto + comando + expected vs actual).
2. Decidir `PROMOTE|REPLACE|RETIRE|DEFER` por garantía (no por fichero), vincular la regresión protectora. No promocionar todo el escenario temporal.
3. Borrar **exclusivamente** directorios temporales propiedad de esta sesión tras el recibo, mediante cleanup/trap protegido y ruta comprobada dentro del temp root; jamás ejecutar `rm -rf` sobre paths de repo o datos ajenos.
4. Verificar `git status` sin pruebas efímeras versionadas; preparar el receipt de entrega de CU-N con conteo de tests retenidos, sustituidos y retirados, y baseline gate time observado.
5. Si un test crítico solo puede ejecutarse temporalmente sin protección equivalente, `DEFER` la eliminación y mantener el bloque en `PAUSED` hasta decidir una garantía perdurable razonable.

## Evidencia de cada cierre (plantilla)

```text
CU-N · SHA HEAD=<sha> · origin/main=<sha> · tag=<tag> · artifact digests=<sha256>
Selected tests=<cmd + rc> · full/merge/integration=<cmd + rc> · RC=<cmd + rc>
UAT: <id,expected,actual,rc> · AAT: <id,expected,actual,rc> · E2E: <id or N/A>
Temp: <dir> · disposition: PROMOTE/REPLACE/RETIRE/DEFER · clean status=<git status>
Release remote: <URL+asset inventory verified> · consumer: <Guard SHA/version>
Next: <blocking fact or following CU-N+1> · human signoff C8-R: <as observed>
```

No inventar números ni afirmar PASS si tests no se ejecutaron. Un `SKIP` no sustituye a un gate.
