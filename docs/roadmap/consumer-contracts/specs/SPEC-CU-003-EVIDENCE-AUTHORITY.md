# SPEC-CU-003 — Evidencia canónica y transferencia a CogniCode Guard

**Estado:** PROPOSED · **Bloque:** CU-2 · **ADR:** `ADR-CU-003`

## Ontología no negociable

1. **Detection**: señal observada por un analizador, puede ser heurística.
2. **Canonical Evidence**: evidencia resuelta en `EvidenceLookup` del snapshot actual, con `FactId`, `EvidenceId`, grado, origen y scope.
3. **Verified Finding**: resultado de `FindingAssembler` + `FindingVerifier` con ejecución, causal chain, evidencia coherente y `AnalysisScope` válido. No basta con un string `evidence_id`.
4. **Rule/Detector Admission**: autorización de un detector/regla por autoridad explícita; no ocurre por adaptar una regla ni por crear `ArchitectureConstraint`.
5. **Policy Authorization/Decision**: responsabilidad exclusiva de Guard; nunca de la calidad del análisis. Firma y permisos son condiciones distintas de la existencia del finding.

## Contrato de integración

- **CU-003.1** Reutilizar `domain::findings::{FindingAssembler,FindingVerifier,VerificationError,...}`, `application::architecture::grounding` y kernel existente (feature gated). Ningún consumer inventa ids canónicos, les asigna timestamps ficticios o los convierte a `bool`.
- **CU-003.2** El DTO de salida de solo lectura contiene `snapshot/analysis_scope`, `source anchors` (byte spans preferidos), `detector execution`, `EvidenceId` verificables, nivel de precisión y `result_state`.
- **CU-003.3** Conversión total: `Verified(Finding) | Advisory(Observation) | Inconclusive(NonEmpty<Gap>) | Unsupported(Reason) | Failed(Error)`. Si falta store/capability/evidence, devolver discriminante honesto; nunca un `Verified` vacío de causal grounding.
- **CU-003.4** Guard solo traduce el DTO a su modelo de observaciones: no fabricará `AdmitterRole::HumanPromoter`, `admitted_at: "0"`, ni `signature` para la evaluación técnica. Debe verificar `ApprovalRecord` con trust root propia antes de emitir `Block`.
- **CU-003.5** Pruebas adversariales: evidence-id desconocido; evidence correcto en snapshot incorrecto; fact-id que no coincide; step sin causal grounding; signature bytes arbitrarios; detector no admitido. Todos rechazan promoción a bloqueo.
- **CU-003.6** Compatibilidad `assurance-evidence/v1`: la proyección Guard no altera orden de campos, digest o expectativas de consumidores Kotlin. Si se necesita otro formato, versionarlo por separado.
- **CU-003.7** Un resultado de `ArchitectureDiagnostic` o `Impact` es solo consultivo; no gateable aunque tenga `severity=error` o `confidence=1.0` sin prueba y admisión correctas.

## Transferencia segura

```text
CogniCode: Source -> Detection -> EvidenceBinding -> Verification -> VerifiedFinding
                                                               |
                                                               v
Guard: RulePolicy + SignedAdmission -> Decision(Allow|Warn|Block|Inconclusive) -> Receipt
```

Nunca fusionar `VerifiedFinding` y `AdmittedFinding` porque describen dimensiones distintas. `Block` requiere ambas garantías de forma explícita y su alcance coherente.

**Permanentes:** 1 negative scope mismatch, 1 negative missing evidence o forged admission en el consumer. **Efímeras:** UAT-CU-2/AAT-CU-2. Referencias: `crates/cognicode-core/src/domain/findings/{assembler,verifier,admission,binding}.rs`, `application/architecture/grounding.rs`, `crates/cognicode-core/src/assurance_export/envelope.rs`; Guard `guard-domain/src/{observation,admission}.rs`, `guard-policy/src/admission.rs`.
