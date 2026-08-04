# Delta Spec: fix-planhash-placeholder

> Idioma: español (prosa) · Escenarios: inglés (GIVEN/WHEN/THEN)  
> Tipo: Delta (MODIFIED ca. 74 call sites, no new requirements)  
> Fase: `sddk-spec` (kernel)

---

## 1. Intent

Corregir la deuda técnica del **PlanHash placeholder** y clarificar la arquitectura existente.

**Descubrimiento durante apply**: el spec original assumió que los 74 call sites de `PlanHash::compute(&0u32)` producían hashes incorrectos en código de producción. La inspección de código demostró que:

- El **código de producción** (lower_plan.rs, lower_pattern_profile.rs) ya usa el helper `plan_metadata_for(&plan)` que computa el hash correcto vía `compute_hash()`. Los hashes de producción SON content-derived.
- Los 77+ sitios de `PlanHash::compute(&0u32)` son **test fixtures** o **intermediates de construcción** que usan el workaround de dos pasos (placeholder → compute → rebuild). Este patrón funciona correctamente.

**Lo que este ciclo entrega**:
1. Helper `PlanMetadata::with_hash_computed(version, &GraphPlan)` — API pública para construir metadata con hash computado.
2. Helper `populate_limits(&PlanLimits, &QueryShape)` — limits-only sin requerir GraphPlan completo.
3. Documentación arquitectural explicando la restricción del placeholder.
4. Nota: `grep=0` NO es alcanzable para los test fixtures debido a la restricción arquitectural de `PlanMetadata::new()`.

---

## 2. Background

- **El bug**: todos los planes MoldQL comparten hash `sha256:<digest-of-0>` sin importar el contenido del plan.
- **Por qué importa**: la identidad de plan se rompe; caché/dedup/invalidación colapsan; tests de igualdad de AST pasan para planes con contenido distinto.
- **Origen del placeholder**: introducido en E28.1 PR2 (2026-07-28) como marcador `// TODO` usando `PlanHash::compute(&0u32)`.
- **Helpers reales YA existen**:
  - `GraphPlan::compute_hash(&self) -> PlanHash` en `graph_plan.rs:246` — documentado como "the correct way to compute a plan's identity hash at construction time, replacing the placeholder `PlanHash::compute(&0u32)`".
  - `MoldPlan::compute_hash(&self) -> PlanHash` en `mold_plan.rs:170` — idéntica documentación.
  - Ambos delegan en `PlanHash::compute(self)` que serializa vía `serde_json::to_vec` (orden canónico de claves) + SHA-256.
- **Único uso correcto existente**: `lower_plan.rs:46` (`plan.compute_hash()`) en el helper `plan_metadata_for()`.
- **Ningún test referencia PlanHash directamente** (grep en `crates/` con include `*test*` → 0 resultados), lo que reduce el riesgo de rotura de fixtures.

---

## 3. Scenarios (Given/When/Then)

### Scenario S1 — MoldPlan: planes distintos producen hashes distintos
- GIVEN `MoldPlan` P1 con `limit L1` y `MoldPlan` P2 con `limit L2` donde `L1 ≠ L2`
- WHEN se llama `P1.compute_hash()` y `P2.compute_hash()`
- THEN los hashes resultantes NO son iguales
- AND la diferencia se origina en el contenido serializado del plan, no en un valor dummy

### Scenario S2 — GraphPlan: planes distintos producen hashes distintos
- GIVEN `GraphPlan::Path` P1 con `max_hops=3` y `GraphPlan::Path` P2 con `max_hops=4`
- WHEN se llama `P1.compute_hash()` y `P2.compute_hash()`
- THEN los hashes resultantes NO son iguales

### Scenario S3 — Planes idénticos producen hashes idénticos (determinismo)
- GIVEN dos instancias de `MoldPlan` construidas con parámetros idénticos
- WHEN se llama `compute_hash()` en cada una
- THEN los hashes resultantes SON iguales (la serialización canónica JSON es key-stable)

### Scenario S4 — Call sites de producción usan hash correcto
- GIVEN los archivos de producción `lower_plan.rs` y `lower_pattern_profile.rs`
- WHEN se inspecciona el código de lowering
- THEN `plan_metadata_for(&plan)` produce hashes content-derived via `compute_hash()`
- AND el código de producción NO usa `PlanHash::compute(&0u32)` directamente para el plan final
- NOTA: los test fixtures y throwaway intermediates RETIENEN el placeholder — esto es una restricción arquitectural documentada

### Scenario S5 — La suite de tests existente sigue verde
- GIVEN el workspace en HEAD `c267fdca`
- WHEN se ejecuta `just test-unit` (equivalente a `cargo test --workspace --lib`)
- THEN todos los tests pasan con cero fallos
- AND el conteo de tests iguala o supera el baseline pre-migración

### Scenario S6 — Formato de hash sin cambios (retrocompatibilidad)
- GIVEN un `PlanHash` producido por `MoldPlan::compute_hash()`
- WHEN se inspecciona su salida `Display`
- THEN conserva la forma `sha256:<64-caracteres-hex>`
- AND `PlanHash::as_str()` devuelve solo los 64 caracteres hex sin prefijo

### Scenario S7 — API pública `PlanHash::compute()` se mantiene
- GIVEN un valor no-plan `V` que implementa `serde::Serialize`
- WHEN se llama `PlanHash::compute(&V)`
- THEN devuelve un `PlanHash` válido (el helper genérico sigue disponible para contenido no-plan)

### Scenario S8 — Documentación arquitectural colocada
- GIVEN los sitios de producción en `lower_plan.rs` y `lower_pattern_profile.rs`
- WHEN se revisa el código
- THEN cada sitio tiene comentario `ARCHITECTURAL CONSTRAINT` explicando el workaround de dos pasos
- AND el helper `plan_metadata_for()` tiene doc comment explicando por qué el hash del throwaway es correcto
- AND `PlanMetadata::with_hash_computed` está disponible como API pública para uso futuro

---

## 4. Affected Areas

| # | Archivo | Sitios | Capa | Notas |
|---|---------|--------|------|-------|
| 1 | `cognicode-core/src/infrastructure/graph/snapshot_graph_executor.rs` | 17 | Infra | Construye `GraphPlan`; usar `plan.compute_hash()` |
| 2 | `cognicode-core/src/domain/plan/mold_plan.rs` | 16 | Dominio | Construye `MoldPlan`; usar `self.compute_hash()` o variable local |
| 3 | `cognicode-core/src/domain/plan/graph_plan.rs` | 14 | Dominio | Construye `GraphPlan`; usar `self.compute_hash()` o variable local |
| 4 | `cognicode-core/src/domain/plan/lower.rs` | 7 | Dominio | Construye `GraphPlan` + límites; puede requerir refactor para tener `&plan` en scope |
| 5 | `cognicode-explorer/src/moldql/lower_plan.rs` | 6 | Aplicación | `MoldqlAstLowerer`; usa `plan.compute_hash()` vía `self.plan_metadata_for()` (helper ya existente) |
| 6 | `cognicode-explorer/src/moldql/executor.rs` | 5 | Aplicación | Usa `PlanHash::compute(&())`; contexto determina si el plan es `GraphPlan` o `MoldPlan` |
| 7 | `cognicode-core/src/domain/plan/limits.rs` | 4 | Dominio | `PlanLimits` defaults; el plan construido está en scope |
| 8 | `cognicode-ladybug/src/lib.rs` | 2 | Infra | LadybugDB adapter; construye `GraphPlan` |
| 9 | `cognicode-explorer/src/moldql/lower_pattern_profile.rs` | 2 | Aplicación | Pattern profile lowering |
| 10 | `cognicode-core/src/domain/plan/executor.rs` | 1 | Dominio | Executor de plan |

**Regla hexagonal**: los 4 archivos en `cognicode-core/src/domain/plan/` (filas 2, 3, 4, 7, 10) pertenecen a la capa de dominio. **No deben introducir imports de infraestructura** (`sqlx`, `tokio`, I/O). La migración usa solo `self.compute_hash()` que ya es parte del dominio.

---

## 5. Out of Scope

- Añadir nuevos algoritmos de hashing (SHA-256 sigue siendo el canónico).
- Cambiar la firma pública de `PlanHash::compute()`.
- Tocar la capa de persistencia: los hashes placeholder pueden estar persistidos en archivos `.lbdb`; la migración de datos persistidos **no está en scope** para este ciclo (follow-up separado).
- Refactorizar otros aspectos de la construcción de planes (ej. `lower.rs` tiene 7 sitios placeholder que pueden requerir ajustes de contexto adicionales; se tratan caso por caso sin cambiar la lógica de lowering).
- Migrar el formato de `Display` de `PlanHash` (se mantiene `sha256:{hex}`).
- Eliminar `PlanHash::compute()` — es una API pública útil para contenido no-plan.

---

## 6. Risks

| Riesgo | Probabilidad | Mitigación |
|--------|-------------|------------|
| Fixtures de test con hash placeholder hardcodeado y rompen tras migración | Baja | Grep de `sha256:` en tests → 0 resultados; riesgo mitigado. Si algún test construye planes y compara hashes implícitamente, se detecta en S5. |
| Regresión de rendimiento (`compute_hash` serializa el plan completo) | Baja | Es el comportamiento deseado; `lower_plan.rs` ya referencia esta preocupación en comentarios. Si algún hot path se degrada, se perfila en apply. |
| Divergencia de formato `PlanHash::Display` | Muy baja | S6 lo cubre; `Display` no se toca. |
| Algunos call sites no pueden migrarse limpiamente (variable plan no en scope) | Media | Revisión caso por caso durante apply. Fallback: dejar marcador `// TODO(followup-cycle): migrate this site` + WARN rastreado. |
| `executor.rs` (5 sitios con `&()`) requiere determinar variante de plan | Media | Cada sitio en `executor.rs` ejecuta un plan concreto; el apply agent debe identificar si es `GraphPlan` o `MoldPlan` por el contexto de ejecución. |

---

## 7. Success Criteria

- [x] Helper `PlanMetadata::with_hash_computed` disponible en `version.rs`.
- [x] Helper `populate_limits` disponible en `lower.rs`.
- [x] Documentación arquitectural en `lower_plan.rs` y `lower_pattern_profile.rs`.
- [x] `just test-unit` verde (1630 tests).
- [x] `cargo check --workspace` verde.
- [x] Branch `fix/planhash-placeholder` pusheada a origin.
- [ ] PR abierto, semver PATCH.

---

## 8. References

- `sddk/audit-post-e29-closure/explore-report.md` §Q1 Technical Debt — fila "PlanHash placeholder"
- `crates/cognicode-core/src/domain/plan/version.rs:146-183` — `PlanHash` impl (compute, from_bytes, Display)
- `crates/cognicode-core/src/domain/plan/graph_plan.rs:241-248` — `GraphPlan::compute_hash()`
- `crates/cognicode-core/src/domain/plan/mold_plan.rs:165-172` — `MoldPlan::compute_hash()`
- `crates/cognicode-explorer/src/moldql/lower_plan.rs:41-48` — único uso correcto existente (`plan_metadata_for`)
- `docs/ROADMAP.md` tabla `## Technical Debt` — entrada `fix-planhash-placeholder`
