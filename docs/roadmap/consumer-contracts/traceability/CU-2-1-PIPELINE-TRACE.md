# CU-2.1 — Trazado del pipeline upstream (trazar, no implementar)

**Estado:** DONE · **Bloque:** CU-2 · **Spec:** `SPEC-CU-003` · **ADR:** `ADR-CU-003`

## Objetivo

Identificar dónde se obtiene evidencia canónica REAL en el pipeline
existente y dónde solo existe indicio. Este hito es **de trazado**:
no introduce código nuevo. La salida es este documento + la lista
de tipos localizados y tests que ejercitan los caminos REAL.

## Pipeline (detect → ground → verify → produce evidence)

```
Detector (dataflow, AST, architecture, taint, …)
   │
   │ ejecuta contra un snapshot
   ▼
DetectorOutcome (domain::findings::outcome)
   │ DetectorDiagnostic (no verificable) + DetectorMatch (con evidence ids)
   ▼
FindingAssembler (domain::findings::assembler)
   │ combina outcome + causal chain + ids canónicos
   ▼
Finding (domain::findings::finding)
   │ FindingGate (gateable?), EvidenceClass (Heuristic/Observation/…)
   │ FindingOrigin, FindingSeverity, FindingStatus, RiskLevel
   ▼
FindingVerifier (domain::findings::verifier)
   │ coteja cada EvidenceId contra EvidenceLookup
   │ comprueba AnalysisScope del snapshot actual
   │ refuta causal steps sin grounding
   │ firma/promueve solo si EvidenceClass == Verified
   ▼
Result<Finding, VerificationError>
   │ can_block(finding, gate) -> bool
   ▼
FindingGate (gateable verificado)  ←—— ESTE es el que puede promover a Block
```

## Pipeline paralelo (architecture violations)

```
ArchitectureEvaluator (domain::architecture)
   │
   │ emite ArchitectureViolation (consultivo, NO gateable)
   ▼
ArchitectureGroundingBridge (application::architecture::grounding)
   │ resuelve GroundingRef (None | Some) en cada violation
   │ convierte a ProducedEvidence (Grounded | Ungrounded)
   ▼
ProducedEvidence (domain::findings::outcome)
   │ usada para enriquecer el snapshot de evidencia, no para promover Block
```

Importante: `ArchitectureViolation` **nunca** es gateable aunque
tenga `severity=error` o `confidence=1.0` sin admisión (SPEC-CU-003.7).
Solo el `FindingGate` que pasa `FindingVerifier::verify_for_gate` con
`Result::Ok` puede ascender al motor de políticas Guard.

## Type map (autoridad y seam)

| Tipo | Archivo | Autoridad | Estado |
|---|---|---|---|
| `FindingAssembler` | `crates/cognicode-core/src/domain/findings/assembler.rs` | CogniCode (técnica) | combina evidence + causal |
| `FindingVerifier` | `crates/cognicode-core/src/domain/findings/verifier.rs` | CogniCode (técnica) | coteja evidence real |
| `VerificationError` | `crates/cognicode-core/src/domain/findings/verifier.rs:177` | CogniCode (técnica) | refutación con causa |
| `EvidenceBinding` | `crates/cognicode-core/src/domain/findings/binding.rs` | CogniCode (técnica) | link evidence ↔ fact |
| `ProducedEvidence` | `crates/cognicode-core/src/domain/findings/outcome.rs` | CogniCode (técnica) | observación verificable |
| `AnalysisScope` | `crates/cognicode-core/src/domain/findings/scope.rs` | CogniCode (técnica) | snapshot del cual la evidencia es válida |
| `ArchitectureViolation` | `crates/cognicode-core/src/domain/architecture/violation.rs` | CogniCode (técnica) | **consultivo, NO gateable** |
| `ArchitectureGroundingBridge` | `crates/cognicode-core/src/application/architecture/grounding.rs` | CogniCode (técnica) | bridge violation → produced evidence |
| `GroundedFindingReport` | `crates/cognicode-core/src/application/findings/grounded_finding_flow.rs` | CogniCode (técnica) | reporte final con grounding |
| `AdmittedDetector` | `crates/cognicode-core/src/domain/findings/admission.rs` | **Guard** (admisión) | detector admitido por autoridad explícita |
| `AdmissionRef` | `crates/cognicode-core/src/domain/findings/admission.rs` | **Guard** (admisión) | ref a ApprovalRecord externa |
| `ApprovalVerifier` (trait) | `crates/cognicode-core/src/domain/findings/admission.rs` | **Guard** (admisión) | trust root propia |
| `EvidenceExport` (envelope) | `crates/cognicode-core/src/assurance_export/envelope.rs` | DTO read-only | shape público consumers |

## Evidencia canónica REAL — dónde se obtiene

1. **`EvidenceLookup` trait** (`domain::findings::verifier.rs:401`):
   `resolve(EvidenceId) -> EvidenceResolution` y `scope() -> Option<&AnalysisScope>`.
   Implementaciones:
   - `SetLookup` (test-only, en el mismo archivo)
   - `infrastructure::evidence_kernel` (producción)
   - `infrastructure::findings::in_memory_evidence` (test)
2. **Origen canónico**: `crates/cognicode-core/src/domain/evidence_kernel/evidence.rs`
   (EvidenceDescriptor + EvidenceId construction).
3. **Sealed a snapshot**: `AnalysisScope` se inyecta en el lookup;
   refutación si evidence pertenece a otro snapshot (verifier.rs:576).

## Indicio (no verificable) — qué evitar como autoridad

- `ArchitectureViolation` sin `GroundingRef` → `ProducedEvidence::Ungrounded`
  (consultivo, nunca gatea).
- `Finding` con `EvidenceClass::Heuristic` o sin `EvidenceBinding` →
  `FindingVerifier::verify_for_gate` devuelve `Err`.
- `signature.bytes.len() > 0` (regla de Guard) sin `ApprovalVerifier`
  → no es autorización.

## Tests que ejercitan los caminos REAL (verificados 2026-10-17)

| Test | Archivo | Cubre |
|---|---|---|
| `refuting_evidence_never_gates` | `domain/findings/verifier.rs:524` | evidencia refutada no asciende |
| `a_fact_that_disagrees_with_its_evidence_is_rejected` | `verifier.rs:549` | fact/evidence incoherente |
| `an_ungrounded_causal_step_is_rejected` | `verifier.rs:562` | causal sin grounding |
| `a_fact_from_another_snapshot_is_rejected` | `verifier.rs:576` | cross-snapshot invalid |
| `ungrounded_violation_becomes_ungrounded_produced_evidence` | `application/architecture/grounding.rs:116` | violation sin grounding |
| `grounded_violation_becomes_grounded_produced_evidence` | `application/architecture/grounding.rs:125` | violation con grounding |
| `empty_violations_yield_empty_produced_evidence` | `application/architecture/grounding.rs:135` | sin violations → sin evidence |
| `architecture_e77_1_wu3_canonical_grounding_e2e` | `tests/` | e2e de grounding canónico |
| `architecture_e77_1_wu0_gap_characterization_e2e` | `tests/` | gaps reales vs indicio |
| `architecture_drift_e2e` | `tests/` | drift entre observaciones y gate |

## Tests AAT-CU-2 que **faltan** (a escribir en CU-2.2/CU-2.3)

Los 6 negativos de SPEC-CU-003.5:

1. `evidence-id desconocido` → rechaza Block
2. `evidence correcto en snapshot incorrecto` → rechaza Block
3. `fact-id que no coincide` → rechaza Block
4. `step sin causal grounding` → rechaza Block
5. `signature bytes arbitrarios` → no es Block
6. `detector no admitido` → rechaza Block

(1)-(4) ya están parcialmente cubiertos por los tests de `verifier.rs`
listados arriba. (5)-(6) son territorio de Guard (fuera del repo
CogniCode). CU-2.3 (contrato de transferencia a Guard) cierra (5)-(6).

## Estado de la WU

- [x] Tipos localizados y autoridad identificada.
- [x] Pipeline trazado (ver diagrama).
- [x] Evidencia REAL vs indicio separados.
- [x] Tests existentes que cubren caminos REAL listados.
- [x] Tests AAT faltantes explicitados (deuda para CU-2.2/CU-2.3).
- [ ] Aceptación negativa consolidada (CU-2.2 entrega).

**No se introduce código nuevo en CU-2.1.** El seam entre
CogniCode (técnica) y Guard (admisión+política) está limpio: los
tipos `AdmittedDetector`, `AdmissionRef`, `ApprovalVerifier` viven
en CogniCode pero su semántica (trust root, AdmissionRecord) es
propiedad de Guard. La conversión `VerifiedFinding → AdmittedFinding`
ocurre **fuera** de CogniCode, en un adaptador Guard (CU-2.3).

## Riesgos identificados (no bloqueantes)

1. `signature` no se valida en CogniCode (es responsabilidad de
   Guard via `ApprovalVerifier`). Si Guard olvida validarla,
   la regla "firma no vacía = autorizado" vuelve. CU-2.3
   codifica el negativo.
2. `ArchitectureViolation` tiene `severity=error` y
   `confidence=1.0` por defecto en algunas variantes. La
   GRPC/MCP surface que la devuelve debe etiquetarla como
   `Consultivo` o el consumer (Guard) la puede promover por
   error. CU-2.4 valida esto con un UAT real.
3. `EvidenceKernel` puede tener feature flags que desconectan
   el grounding. Si el binario se publica sin la feature, los
   tests `find_refuting` siguen pasando pero el AAT negativo
   cambia su forma. CU-2.5 (release) cierra esto con un smoke
   contra el binario REAL con feature set documentado.
