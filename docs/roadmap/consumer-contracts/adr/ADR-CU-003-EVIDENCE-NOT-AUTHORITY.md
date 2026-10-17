# ADR-CU-003 — Separación formal: observación, evidencia, admisión y decisión

**Estado:** PROPOSED · **Bloque:** CU-2

## Contexto

`ArchitectureEvaluator` emite `ArchitectureViolation`, nunca un finding gateable; `FindingVerifier` coteja hechos, evidence ids y snapshot real. Guard fabrica metadatos `AdmitterRole::HumanPromoter` al crear constraints y admite `AdmittedFinding` con predicate basado en `signature.is_empty()`. Son responsabilidades diferentes y los resultados no se deben promover por mera serialización.

## Decisión propuesta

```text
Observe (puede ser heurístico) -> Ground (EvidenceBinding) -> Verify (FindingVerifier)
   -> Verified/Advisory/Unknown -> Guard RuleAdmission (firma verificada)
   -> Guard PolicyDecision (Allow/Warn/Block/Inconclusive) -> Receipt
```

- CogniCode es autoridad exclusiva del **hecho técnico y su scope**; no decide si una política corporativa bloquea.
- Guard es autoridad exclusiva de **admisión y política**; no crea hechos, confianza o firmas de análisis técnico.
- El seam es un DTO/ADT de solo lectura y total. Gaps son valores y no excepciones invisibles; una observación `Ungrounded` no es `Verified`.
- No reetiquetar heurística como certeza por `confidence=1.0`. La autorización de cambios mutantes sigue en `refactor::apply`/Gate de ejecución, fuera del permiso de lectura.

## Alternativas rechazadas

- «Toda alerta es un finding»: causa bloqueos falsos.
- «Cualquier firma no vacía = autorización»: no verifica trust root ni payload.
- «Reutilizar un EvidenceId de otro snapshot»: rompe integridad referencial.

**Aceptación adversarial:** los 6 negativos de SPEC-CU-003 no consiguen `Block` por vía de evidencia falsa; las reglas aprobadas con evidencia verificada sí pueden pasar al motor de políticas Guard con su autorización real.
