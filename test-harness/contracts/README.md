# Contratos normativos propuestos

Los esquemas JSON (`*.schema.json`) son artefactos de diseño **no implementados**. Sirven para TDD de parsers/serializadores, compatibilidad de los adaptadores y pruebas mutacionales. Versionar de manera explícita cualquier ruptura de forma o semántica.

- [`plan.schema.json`](plan.schema.json): el plan ordenable de ejecución.
- [`receipt.schema.json`](receipt.schema.json): hecho histórico de ejecución de una suite.
- [`gate-verdict.schema.json`](gate-verdict.schema.json): prueba de satisfacción de obligaciones.
- [`examples/`](examples/): ejemplos contractuales; NO recibos de tests reales.

## Invariantes semánticos NO expresables completamente en JSON Schema

- `PASSED` requiere `exit_code=0`, lista de casos no vacía salvo excepción, `failed_cases=0`, y `executed_cases = passed_cases + failed_cases + skipped_cases` (según semántica documentada).
- `SATISFIED` requiere `gaps=[]`, exactamente una prueba válida por requirement (o pruebas equivalentes justificadas) y sin fallo actual.
- Un `REUSE` requiere `receipt_id` y `reuse_validity` verificada; `BLOCK` nunca produce PASS.
- `source_fingerprint`/`candidate_artifact_digest` deben corresponder a fuentes concretas, no a cadenas inventadas.
- Falta de un receipt puede ser normal durante desarrollo pero nunca sustituye un gate required.

Esos invariantes exigen tests *model-based/property* en `test-harness/tests/`, además de validar schemas.
