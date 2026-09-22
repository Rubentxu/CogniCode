# RELEASE-CANDIDATE — Programa PRF

> Estado: DRAFT — auditoría 2026-09-22 (operador) reveló gaps contractuales que invalidan la equivalencia "READY FOR RELEASE" ↔ "C7 PASS". El SHA candidato está **congelado** abajo; las acciones 1-5 del plan operador (sección 5 de la auditoría) se ejecutarán **en orden estricto** contra este SHA, sin reescribirlo.
>
> **NOTA 2026-09-22 (sesión actual):** la acción 3 H-02 (`80e7c403`) y el H-01 RED pin (`5ed7f865`) avanzaron HEAD. HEAD actual: `0a7eb8d2`. El SHA congelado `178f8a5b` queda **stale**; el operador deberá re-firmar el freeze antes de proseguir. NO se actualiza automáticamente: el push sigue BLOQUEADO.
>
> **Meta-nota sobre staleness iterativa:** cualquier commit que toque estos docs refresh mueve el HEAD, lo que inmediatamente stale-a este puntero. La solución canónica es **dejar de tocar los punteros** (mantener el freeze `178f8a5b` honestamente stale) hasta que el operador re-firme; o que el operador re-firme el SHA congelado a `0a7eb8d2` (o posterior) cuando lo autorice. Los refrescos intermedios solo sirven mientras se trabaja en la misma sesión y son aceptables porque el push sigue BLOQUEADO.

## Candidato (CONGELADO — STALE)

| Campo | Valor |
|---|---|
| SHA candidato (full) | **`178f8a5bf83b52433c46887456823c606ac786b7`** (corto: `178f8a5b`) — **STALE** (HEAD actual `0a7eb8d2`) |
| HEAD actual | `0a7eb8d20e2ccc79f8c355f8c94344cd89921f1d` (docs refresh post H-01 RED pin, JOURNAL §31) |
| Identidad del artefacto | **fija** — cualquier modificación posterior del HEAD exige nuevo proceso de release. No se firma C7 sobre "el HEAD en el momento de la firma". |
| Cadena de procedencia | `0a7eb8d2` (docs refresh post H-01 RED) → `5ed7f865` (H-01 RED pin) → `8074a426` (matrix recount) → `f8cd3375` (docs refresh) → `80e7c403` (H-02 GREEN) → `82f1ba54` (SHA congelado + matriz reconciliación) → `178f8a5b` (reconciliación C2) → `f0e25652` (pointer refresh) → `c1b14017` (RELEASE-CANDIDATE refresh) → `86df20de` (docs checkpoint) → `47dd39ac` (clippy-fix) → `5b96db43` (T4 base, último de origin/main). |
| Versión | Pendiente de decisión del operador (candidatos razonables: `v0.97.4` patch de clippy; `v0.98.0` minor; `v0.98.0-prf` cierre del programa; `v1.0.0-prf` release production-ready milestone). El tag v0.97.3 NO contiene estos fixes. |
| Plataformas probadas | Linux x86_64 (única plataforma con UAT ejecutada). Cobertura ampliada pendiente si el operador exige otras plataformas. |

## UATs ejecutados (binarios reales)

| UAT | Fase | Resultado |
|---|---|---|
| UAT-F3-001 | F3 vertical CLI↔MCP | PASS (cert PRF-F3) |
| UAT-F4-001 | F4 persistencia/aislamiento | PASS (cert PRF-F4) |
| UAT-F5-001 | F5 seguridad/límites/cancelación | PASS (cert PRF-F5) |
| UAT-F6-001 | F6 distribución | PASS tras fix H-F6-1 (cert PRF-F6) |

## Certificaciones

- C0 (F0) ACCEPTED; C1 (F1) ACCEPTED; C2 (F2) ACCEPTED (cert PRF-C2).
- F3, F4, F5, F6 = ACCEPTED (certs por fase en evidence/CERTIFICATES.md).
- C7 = NO CERTIFICADO hasta: aceptación de release firmada y
  frentes abiertos cerrados.

## Batería de pruebas en HEAD

- `cognicode-core --lib` (con `multimodal`): **2128 passed, 0 failed, 27 ignored + 1 H-01 RED test failing as expected** (verificado en HEAD `0a7eb8d2` post-H-02 + H-01 RED pin + docs refresh; era 2126 antes de H-02, +3 tests nuevos: 2 H-02 + 1 H-01 RED).
- Workspace `--lib`: GREEN salvo `moldql::cursor::consume_keyword_panics_on_mismatch`
  (cognicode-explorer), PRE-EXISTENTE en HEAD limpio (verificado con
  stash), NO regresión del programa PRF.
- `cognicode-cli` bin cogh: 293 passed, 0 failed.
- `clippy -D warnings`: clean para `cognicode-core` (post `0a7eb8d2`); warning inventory de `cognicode-cli`/`cognicode-ladybug` verificado sin drift vs `5b96db43` (medido con `git stash` + diff antes/después).

## Frentes abiertos (deuda)

| ID | Descripción | Estado |
|---|---|---|
| H-F3-1 | find_usages MCP con walk+parser inline | OPEN (LOW, no bloqueante) |
| H-F6-1 | doble resolución de home | RESUELTO (`0764fb81`) |
| H-clippy-FullGraphStrategy-type_complexity (D34-1) | clippy::type_complexity en `crates/cognicode-core/src/infrastructure/graph/strategy.rs:521` (4-tuple a `struct FileData`) | **RESUELTO** (`47dd39ac`) en sesión 2026-09-22 |
| H-clippy-cli-residual (D34-2) | Catálogo de warnings preexistentes en `cognicode-cli` (unused_imports, dead_code) en `cmd/{lifecycle,release_contract,bundle_manifest,ide,layout,tracker,...}.rs` | **OPEN — fuera de programa PRF** (decisión de scoping del operador 2026-09-21; reservada a sesión propia). NO bloquea release. |
| moldql panic test | test de pánico inestable en explorer | PRE-EXISTENTE, fuera de alcance PRF |
| 6 fallos preexistentes | cogh_uninstall, manifest_upsert ladybug, rate-limit H10 | catalogados, no regresiones |

## Decisión formal

- [x] Evidencias reunidas y verificadas en HEAD `178f8a5b` (reconciliación C2).
- [x] **Desarrollo de capacidades F0–F6** — ejecutado y documentado.
- [ ] **C7 = NO CERTIFICADO** — la auditoría 2026-09-22 del operador (sección ‘Hallazgos que impiden dar por completo el contrato original’) demostró que los certificados C0–C6 actuales son **declarativos respecto a los criterios del programa PRF original, pero no contractualmente equivalentes** a sus requisitos. La diferencia técnica entre ‘READY FOR RELEASE’ (preparación completada) y ‘C7 PASS’ (garantías cumplidas) es ahora explícita en este documento y se cierra con el plan de la sección ‘Cierre de PRF’ más abajo.
- [ ] **Publicación (push + tag)** — **BLOQUEADA** por directive § 3 y por la auditoría del operador: “mantendría C7 pendiente y la publicación bloqueada, sin deshacer los avances de F0–F6”.

## Cierre de PRF (plan derivado de la auditoría)

Origen: auditoría operador 2026-09-22 (sección ‘Cómo cerraría PRF sin crear otro roadmap’). Las 5 acciones se ejecutan **en orden estricto** contra el SHA congelado arriba. Cualquier nueva corrección del HEAD invalida este plan.

### 1. SHA candidato (CONGELADO) — cerrado
- Acción: registrar el SHA completo como identidad fija del artefacto (no ‘HEAD al firmar’).
- Estado: cerrado en este commit. SHA full registrado arriba.

### 2. Reconciliación requisitos ↔ certificados
- Acción: contrastar los 8 documentos `SPEC-*` y las 27 UAT originales con C0–C6. Para cada ítem original, registrar disposición explícita: probado / sustituido / excluido del alcance / pendiente.
- Artefacto: matriz `docs/prf/specs/RECONCILIATION-MATRIX.md` (a crear).
- Por qué primero: sin esta base no se firma C7 ni se acepta nada como ‘cumple el contrato PRF’.

### 3. Cerrar fallos de correctitud que afectan al producto estable
- **H-01 (ALTA): RED pin ✅ en `5ed7f865` (JOURNAL §31); GREEN pendiente.** Test `h01_byte_change_with_same_mtime_and_same_size_must_invalidate_cache` pinea el caso exacto (cambio de bytes preservando TANTO mtime COMO size; falla hoy con "stale cache entry was served"). El F2.W9 test existente cubre solo "size cambia", no "size preservado". La elección de la señal derivada del contenido (SHA-256, xxhash, BLAKE3, hash incremental durante walk, o "documentar la limitación") es **decisión del operador**. Implementación pendiente de autorización.
- **H-02 (ALTA): ✅ CERRADO en `80e7c403` (JOURNAL §30)** — `FullGraphStrategy::build_full_graph_report` añadido como inherent method (paralelo al `PerFileStrategy` existente). Captura walk errors, read errors, parser-init, find_* errors y UnsupportedExtension como `SkippedFile` con `SkipReason` clasificado. Status `Complete`/`Partial` explícito. Contrato del trait original `build_full_graph` preservado (no se cambió la firma). 2 nuevos tests RED→GREEN; suite 2128/0/27; clippy `-D warnings` clean.
- Por qué después de la reconciliación: la reconciliación puede descubrir que algunos de estos 'fallos' son en realidad 'capacidades declaradas fuera de alcance'; el fix se ajusta entonces.

### 4. Completar pruebas ausentes de C3–C6
- H-03: hacer converger **una vertical CLI↔MCP hacia un único caso de uso** (reutilizar piezas no es converger).
- H-04: probar persistencia material **o** documentar que la capacidad es ‘reconstrucción determinista sin persistencia’. Si se documenta como reconstrucción, no presentarla como certificación de almacenamiento persistente.
- H-05: UAT de extensión read-only con el contrato previsto; cancelación durante ejecución de operación costosa (no solo ‘cancelación que llega después’); seguridad adversarial en el alcance anunciado (permisos R vs W/E, symlinks, traversal, fuga de secretos).
- H-06: ciclo `instalar A → actualizar a B → rollback o rechazo de downgrade` con dos paquetes diferenciados en un canal de pruebas verificable; repetir `--home` sobre el artefacto que contiene la corrección.
- H-07: gate independiente por SHA — definir el mecanismo (remoto o local con protección equivalente y verificable), ejecutar la prueba negativa (test rojo deliberado debe impedir integración/publicación).

### 5. T5 + decisión C7 sobre el candidato congelado
- Solo después de 1–4 cerrados. T5 ejecuta la batería completa del perfil C7 sobre `178f8a5b` con plataforma, hashes, pruebas, excepciones aprobadas y condiciones de soporte **fijadas antes de la ejecución** (no después). Decisión C7 firmada o rechazada con la matriz de la acción 2 como evidencia contractual.

## Notas de honestidad

- **No** se afirma ‘READY FOR RELEASE ≡ C7 PASS’. La auditoría 2026-09-22 lo descarta.
- **No** se introduce un nuevo roadmap. Se cierra el PRF existente.
- **No** se reabre trabajo finalizado de F0–F2–F3; se conservan sus commits y se actúa sobre los gaps contractuales declarados en la auditoría.
- **No** se ejecuta push ni tag — la publicación sigue bloqueada por la propia auditoría.
- El gate del operador (push, tag, decisión de versión, abrir sesión D34-2) sigue siendo válido y adicional a este plan.
