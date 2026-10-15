# Ontología, invariantes y lenguaje ubicuo

## Glosario

- **Suite**: unidad identificable de obligación de prueba, p. ej. `cargo.core.default.lib` o `sandbox.rust.xxx`. No confundir con stage de PipelineK.
- **Obligación / Requirement**: capacidad que el gate exige demostrar para un *candidate* concreto.
- **Caso**: prueba individual dentro de la suite; una suite puede no ejecutarse si 0 casos descubiertos.
- **Perfil de ejecución**: flags, features, serialidad, recursos, entorno y runner.
- **SourceSnapshot**: identidad del código probado (incluidos cambios no commiteados, fixtures y entradas relevantes).
- **Fingerprint**: digest determinista y tipado de inputs y ejecutor; no sólo SHA de Git.
- **EvidenceReceipt**: hecho inmutable de una ejecución y sus resultados; no certificado atemporal.
- **ReuseDecision**: resultado **calculado** al comparar un recibo con la situación actual.
- **ExecutionPlan**: DAG de suites, dependencias y tareas, derivado de obligaciones.
- **GateVerdict**: decisión de aceptación de un conjunto completo de obligaciones.
- **Resume**: continuar ejecución tras fallo, reparación, interrupción o reinicio, revalidando antes la evidencia acumulada.
- **Replay**: reproducir salida histórica; no acredita ejecución nueva.
- **Quarantine**: exclusión declarada y revisada, nunca contada silenciosamente como PASS.
- **Post-PRF**: gobernanza de trabajo futuro; PRF cerrado no se modifica.

## Tipos ADT propuestos (pseudocódigo)

```text
SuiteStatus = Pending | Running | Passed | Failed | Cancelled
            | TimedOut | InfraError | Missing | Quarantined | Skipped
ReuseDecision = Reusable(receiptId, inputDigest)
              | Invalidated(reasons: NonEmptyList<InvalidationReason>)
              | Undetermined(reason)
InvalidationReason = SourceChanged(path) | DependencyChanged(id)
                   | TestListChanged | FeatureSetChanged | ToolchainChanged
                   | RunnerChanged | FixtureChanged | EnvChanged(name)
                   | ArtifactChanged(hash) | ExternalStateUnverified
                   | TimeoutOrInterruption | UnknownInput | PolicyChanged
                   | MissingEvidence | RevokedEvidence
GateVerdict = Satisfied(proofs) | Unsatisfied(gaps) | Inconclusive(reasons)
PlanAction = Execute(suite) | Reuse(suite, receiptId) | Block(suite, reason)
```

## Máquina de estados

```text
Pending -> Running -> Passed | Failed | InfraError | TimedOut | Cancelled
Passed --revalidación--> Reusable | Invalidated | Undetermined
Failed --fix--> Pending; nunca pasa automáticamente a PASS.
Running --crash--> Cancelled o Unknown; recibo terminal ausente NO es éxito.
Quarantined --evidencia explícita--> Excepción aprobada, NO PASS.
Gate: Satisfied sólo con obligaciones completas, válidas, compatibles y exhaustivas.
```

## Leyes universales

1. `valid(PASS, snapshot) = true` únicamente con huella equivalente y cobertura verificable.
2. Un fallo, cancelación, ausencia o error de harness nunca es un PASS reutilizable.
3. Un cambio en definición o implementación del runner/política invalida sus recibos afectados.
4. Dos suites con mismos nombres, pero distinta features/flags/env, no son idénticas.
5. La suma de suites cubiertas por recibos `Reusable` y ejecuciones `Passed` debe cubrir **exactamente** el conjunto requerido; no basta con porcentajes.
6. Una suite sin casos ejecutados puede ser PASS sólo si el contrato permite expresamente cero; por defecto no.
7. Fallos de tests y fallos de infraestructura tienen estados diferentes, con rc observables.
8. La reanudación no puede ocultar outputs anteriores ni borrar el primer fallo; sólo añadir recibos nuevos.
9. `--force` puede forzar ejecutar, nunca forzar una certificación verde.
10. La revocación de un recibo ya reutilizado debe invalidar cualquier GateVerdict derivado de él.
