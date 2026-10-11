# MERGE-GATE BYPASS POLICY — B1.W4

> **Estado**: ACTIVE 2026-10-11. Bloque B1.W4 del roadmap B1..B6.
> Contrato leíble por máquina: scripts/pins.
> Autoridad última: branch protection + `scripts/ci/pipeline_authority.py`.

## 1. Principio

El gate `merge-gate` se satisface **únicamente** cuando el PipelineK
autoritativo lo declara PASS tras ejecutar todos los stages.

NO existe procedimiento automático de bypass. NO existe fallback para
declarar `merge-gate: success` desde un script ad-hoc cuando PipelineK
falla, está caído, o no responde.

## 2. Por qué existe esta política

El 2026-10-09, durante el squash-merge de PR #345, el operador publicó
manualmente `merge-gate: success` vía `gh api` para satisfacer
`branch_protection: strict:true, contexts:[merge-gate]` porque el
trigger automático de PipelineK se había eliminado al borrar
`.github/workflows/`. La acción fue:

  - Explícita (operador ejecutó el comando, no fue accidental).
  - Documentada (JOURNAL + el commit `3203bc75` del PR #345 lo cita).
  - Recuperable (cualquier re-run del gate real sobrescribe el status
    manual sin efectos colaterales).

Pero también fue el tipo de acción que, sin la disciplina de auditar,
se repite "esta vez está justificado, la próxima también". B1.W4
formaliza el NO.

## 3. Cuando el gate falla (infra rota, timeout, red caída)

Estado del ledger: **BLOCKED**, NO PASS.

Acciones permitidas:

  1. Diagnosticar la causa (logs del runner, red, configuración).
  2. Re-correr el pipeline (los `*.pipeline.kts` son idempotentes).
  3. Si el re-run no puede procederse en N reintentos, abrir issue
     con etiqueta `infra-blocked` y notificar al operador.
  4. NO publicar el PR. NO mergear. NO marcar `merge-gate: success`
     por ningún medio.

Acciones prohibidas (sin excepción de operador):

  - `gh api repos/.../statuses/... -f state=success` (manual override).
  - `gh pr merge --auto` con el bypass flag.
  - Cualquier script local que afirme `merge-gate: success` al
    branch protection.

## 4. Excepciones (operador + procedimiento explícito)

Una excepción SOLO es válida cuando:

  1. El operador (Ruben) emite un veredicto escrito en el journal
     del repositorio (`JOURNAL.md` del roadmap activo o
     `odd/tasks/<feature>.md`).
  2. El veredicto cita: SHA base, SHA candidato, motivo de la
     excepción, riesgo aceptado, plan de recuperación.
  3. El veredicto tiene fecha y firma.
  4. La acción de merge resultante lleva un commit que cita el
     veredicto en el mensaje (`Refs: <ruta al veredicto>`).

Sin esos 4 elementos, el merge NO es válido aunque pase.

## 5. Audit trail

Cualquier acción sobre `merge-gate:status` queda registrada en:

  - El commit de merge (mensaje + descripción del PR).
  - El journal (`docs/roadmap/JOURNAL.md`).
  - El goal doc activo (`odd/tasks/<feature>.md`) si la acción
    forma parte de un programa con goal tracking.

Un commit de merge SIN referencia a bypass documentado se considera
**FAIL-OPEN** y debe revertirse en el siguiente ciclo.

## 6. Contratos machine-checked

Los runbooks y scripts de CI en `scripts/ci/` pinean invariantes
relacionados con el gate. Los más relevantes:

  - `scripts/ci/test_no_actions_workflows.py`: cero workflows en
    `.github/`. La ausencia de workflows es la condición técnica
    que evita el bypass automático.
  - `scripts/ci/preflight-clean-clone.sh`: el gate corre sobre un
    clone limpio. Si la infra no soporta un clean clone, el gate
    debe fallar (no fallback).
  - `python3 scripts/ci/test_roadmap_version_ratchet.py`: la
    versión del workspace aparece en el ROADMAP o JOURNAL
    reciente (un release sin documentar es bypass por omisión).

Cualquier mutación que deshabilite uno de estos contratos requiere
PR con descripción explícita del nuevo bypass policy y una entrada
en `JOURNAL.md`.

## 7. Lo que NO cambia con esta política

  - PipelineK sigue siendo el ÚNICO orquestador (no GitHub Actions).
  - Branch protection sigue con `strict:true, contexts:[merge-gate]`.
  - `scripts/ci/test_no_actions_workflows.py` sigue pineando
    `cero workflows en .github/`.

## 8. Historia

  - 2026-10-09: bypass manual de PR #345 (autorizado, documentado).
  - 2026-10-11: B1.W4 formaliza el NO bypass automático. Este runbook
    entra en vigor.