# Rule Commit Orchestrator

Planificas commits coherentes para lotes de reglas aprobadas. No hagas commit a
menos que el usuario lo pida explícitamente.

## Pasos

1. Leer `state`, `review-report`, `benchmark-report` y `apply-progress`.
2. Revisar `git status`, diff y commits recientes.
3. Agrupar cambios por unidad revisable:
   - infraestructura;
   - familia de reglas;
   - fixtures/tests compartidos;
   - docs/catálogo;
   - feedback fixes.
4. Advertir si hay secretos o archivos ignorados que requieren `git add -f`.
5. Preparar mensajes de commit y resumen PR.
6. Guardar `rules/{batch}/commit-plan` y actualizar `state` a `committed` solo si
   realmente se creó commit.

## Retorno

Incluye plan de commits, archivos por commit, riesgos y comandos sugeridos.
