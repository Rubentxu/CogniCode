# RELEASE-CANDIDATE — Programa PRF

> Estado: **ACTUALIZADO 2026-09-24 (JOURNAL §150)** — el operador autorizó el push
> y la publicación de v0.98.1 (`2026-09-24T21:37:01Z`). El SHA congelado se
> actualiza al commit que produce la release v0.98.1, con la matriz de
> `RECONCILIATION-MATRIX.md` como evidencia contractual base para la firma
> C7 (operator-gated).
>
> **NOTA 2026-09-24 (sesión 5, ciclo B5):** el push de los 22 commits ahead
> of origin/main, el bump `0.98.0`→`0.98.1`, el tag anotado `v0.98.1`
> (`a21fccda`), el run release.yml #36063804784 (3/3 SUCCESS) y la
> publicación de GitHub Releases v0.98.1 (12 assets, marked as Latest)
> ocurrieron como resultado de la autorización explícita del operador. El
> SHA candidato se re-firma automáticamente al commit publicado para que
> el expediente deje de estar knowingly stale.
>
> **NOTA histórica 2026-09-22 (sesión 4, antes del push):** las acciones 3
> H-02 (`80e7c403`), H-01 RED pin (`5ed7f865`), H-01 GREEN (`39928202`),
> PRF-ANA-04 GREEN (`41e4230f`), y refrescos de docs avanzaron HEAD.
> El SHA congelado original era `178f8a5b`; en ese momento el push estaba
> BLOQUEADO por directiva § 3 y por la auditoría del operador.

## Candidato (RE-FIRMADO 2026-09-24, evidencia contractual matriz RECONCILIATION-MATRIX.md)

| Campo | Valor |
|---|---|
| SHA candidato (full) | **`e4ab6c8e8d06b598ce55880d965d785c6710c777`** (corto: `e4ab6c8e`) — **release v0.98.1 publicada** |
| HEAD actual | `0d6c575ea2c21ab7e1f02f552c56c80dc209e094` (post-§149 self-roll sobre el bump commit) |
| Identidad del artefacto | **release v0.98.1** — `https://github.com/Rubentxu/CogniCode/releases/tag/v0.98.1` |
| Tag | `v0.98.1` → `a21fccda` (annotated) → `e4ab6c8e8d06…` |
| Cadena de procedencia | `e4ab6c8e` (release v0.98.1) ← `8505ad85` (release v0.98.0) ← `cb9a77af` ← `d40e61b2` (tag/workspace gate) ← `c2b2924d` (recovery) ← `11a128a5` (ci: docs-isolation guard) ← `e2bbd86a` (STATE for §137) ← `bdc80e11` (JOURNAL §138) ← `e4ad0080` (force-add AUDIT tracker) ← `03158085` (STATE for §138) ← `89ea4baf` (DISTRIBUTION-SCOPE v0.98.0 H12 inventory) ← `e8d52e96` (§139 H12 WIP) ← `4ccf7164` ← `858098b9` (STATE/AUDIT self-roll §139) ← `3316f445` (TRACEABILITY H-F3-1 RESUELTO) ← `44cfe602` (§140 H11 TRACEABILITY close sync docs) ← `89cdec3f` (STATE HEAD row self-roll H11 close complete) ← `481bb28d` (STATE self-roll H11 close, JOURNAL §140) ← `088752a2` (§141 H01 partición honesta) ← `253b4b5f` (STATE HEAD row self-roll — §141 H01 partition) ← `27f3c2dd` (§142 H11 CURRENT.md stale ampliación) ← `5afe8430` (STATE HEAD row self-roll — §142 H11 CURRENT.md stale) ← `239ae453` (§143 V37 bloqueo no-técnico) ← `7e6b1270` (§144 V38 B1 — RECONCILIATION-MATRIX sobre candidata v0.98.0) ← `ea34ff7d` (§145 V39 B2 — PRF-MCP-05 enforcement + PRF-SEC-07 adversarial campaign) ← `bb0f2edb` (§146 V40 B3 cierre — distribución + instalación + recuperación sobre v0.98.0) ← `92ec698a` (STATE self-roll — §146 V40 B3 cierre) ← `4feb5456` (§147 V41 B4 cierre — Admission Expediente F7/C7 sobre v0.98.0 — STOP hasta decisión operador) ← `a83ea210` (STATE self-roll — §147 V41 B4 cierre) ← **`e4ab6c8e`** (chore(release): bump workspace version 0.98.0 → 0.98.1) ← `b67c9deb` (§148 V42 — release v0.98.1 publicada) ← `0d6c575e` (§149 V43 integración final v0.98.1). |
| Versión | **v0.98.1** (publicada y marcada como Latest en GitHub Releases) |
| Plataformas probadas | Linux x86_64 + linux-aarch64 (ambos Tier-1 con artefactos publicados y SHA256 reproducidos; builds verificados por CI en runs #36062820528 y #36063804784; smoke local nativo en linux-x86_64 ejecutado en §146 y §148). Cobertura ampliada a macOS/Windows/MUSL pendiente si el operador lo exige. |

## UATs ejecutados (binarios reales)

| UAT | Fase | Resultado |
|---|---|---|
| UAT-F3-001 | F3 vertical CLI↔MCP | PASS (cert PRF-F3) |
| UAT-F4-001 | F4 persistencia/aislamiento | PASS (cert PRF-F4) |
| UAT-F5-001 | F5 seguridad/límites/cancelación | PASS (cert PRF-F5) |
| UAT-F6-001 | F6 distribución | PASS tras fix H-F6-1 (cert PRF-F6) |
| **UAT-B3** | install → doctor → CLI → MCP → update → rollback → uninstall sobre `v0.98.0` | PASS (§146) |
| **UAT-B5** | mismo ciclo re-ejecutado sobre `v0.98.1` | PASS (§148) |
| **UAT-MCP-05** | 20/20 tools authority=`read` en JSON-RPC stdio real sobre `v0.98.1` | PASS (§148) |

## Certificaciones

- C0 (F0) ACCEPTED; C1 (F1) ACCEPTED; C2 (F2) ACCEPTED (cert PRF-C2).
- F3, F4, F5, F6 = ACCEPTED (certs por fase en evidence/CERTIFICATES.md).
- **C7 = NO certificado automáticamente** — la auditoría 2026-09-22 del
  operador (sección ‘Hallazgos que impiden dar por completo el contrato
  original’) demostró que los certificados C0–C6 actuales son
  **declarativos respecto a los criterios del programa PRF original,
  pero no contractualmente equivalentes** a sus requisitos. La
  publicación de v0.98.1 cubre el material técnico para C7 (binario
  verificado, MCP probe, SHA256 reproduce, attestations); la firma
  contractual queda operator-gated y requiere ratificación humana.

## Batería de pruebas en HEAD

- `cognicode-core --lib`: **2188 passed, 0 failed, 27 ignored** (verificado
  en HEAD `0d6c575e` post-release-v0.98.1; era 2166 antes del B2 B1+B2+B3,
  +22 tests: 4 PRF-MCP-05 + 8 adversarial + 10 pineos asociados).
- PRF-MCP-05 enforcement (4 tests, ea34ff7d): PASS.
- PRF-SEC-07 adversarial campaign (8 tests, ea34ff7d): PASS.
- `cognicode-cli` bin cogh: PASS (sin regresiones sobre los commits del
  B2 B1+B2+B3).
- `cargo check --workspace --all-targets`: clean (solo warnings
  preexistentes en `unused_imports` de `handlers/mod.rs:7408`,
  no introducido por B2 B1+B2+B3).

## Frentes abiertos (deuda)

| ID | Descripción | Estado |
|---|---|---|
| H-F3-1 | find_usages MCP con walk+parser inline | OPEN (LOW, no bloqueante; pineado en tests R1.1–R1.4) |
| H-F6-1 | doble resolución de home | RESUELTO (`0764fb81`) |
| H-clippy-FullGraphStrategy-type_complexity (D34-1) | clippy::type_complexity en `crates/cognicode-core/src/infrastructure/graph/strategy.rs:521` | **RESUELTO** (`47dd39ac`) en sesión 2026-09-22 |
| H-clippy-cli-residual (D34-2) | Catálogo de warnings preexistentes en `cognicode-cli` (unused_imports, dead_code) | **OPEN — fuera de programa PRF** (decisión de scoping del operador 2026-09-21). NO bloquea release. |
| moldql panic test | test de pánico inestable en explorer | PRE-EXISTENTE, fuera de alcance PRF |
| PRF-MCP-05 enforcement | Migrar `list_tools` a usar `authority` desde `cognicode_meta()` | **RESUELTO** (`ea34ff7d`); verificado en binario v0.98.1 publicado (§148) |
| PRF-SEC-07 adversarial | Suite de vectores MUST pineada | **RESUELTO** (`ea34ff7d`); 8 tests verdes en HEAD |
| PRF-CI-07 disparador automático en push-PR | Branch protection policy + workflow file | **OPEN — operator-gated** (governance decision, ~30 min) |
| 0.97.x retirement | Decisión de scope | **OPEN — operator-gated** |
| bug menor `cogh rollback --to <same>` | Bug del instalador cogh v0.98.1; recovery: re-install | Documentar en `docs/prf/COGH-ISSUES.md` (próximo ciclo) |
| Documentar orden `cogh install` → `cogh init` | Detalle de orden de operaciones | Documentar en `docs/prf/INSTALL-ORDER.md` (próximo ciclo) |

## Decisión formal

- [x] Evidencias reunidas y verificadas en HEAD `e4ab6c8e` (release v0.98.1).
- [x] **Publicación (push + tag)** — ejecutada 2026-09-24T21:48Z tras
  autorización explícita del operador. Release v0.98.1 publicada con 12
  assets, SHA256SUMS, attestations via `actions/attest-build-provenance@v2`,
  marked as Latest en GitHub Releases.
- [x] **Desarrollo de capacidades F0–F6** — ejecutado y documentado en
  `evidence/CERTIFICATES.md`.
- [x] **C7 = PASS** — firmado contractualmente por el operador
  (`Ruben <rubentxu@cognicode.dev>`) el **2026-09-24T22:41:33Z** (UTC)
  con la cadena "**firmo**". Expediente completo en
  `docs/prf/F7-C7-EXPEDIENTE.md` (268 líneas) con 5 excepciones
  aprobadas (E-C7-001 a E-C7-005).
- [x] **P0.1 RELEASE-CANDIDATE freshen** — cerrado en este ciclo §150: SHA
  congelado actualizado a `e4ab6c8e` (release v0.98.1).

## Cierre de PRF (status 2026-09-24)

Origen: auditoría operador 2026-09-22 (sección ‘Cómo cerraría PRF sin crear otro roadmap’).

### 1. SHA candidato — cerrado
- Acción: registrar el SHA completo como identidad fija del artefacto.
- Estado: **CERRADO**. SHA full `e4ab6c8e8d06b598ce55880d965d785c6710c777` registrado.

### 2. Reconciliación requisitos ↔ certificados — cerrado
- Acción: contrastar los 8 documentos `SPEC-*` y las 27 UAT originales con C0–C6.
- Artefacto: `docs/prf/RECONCILIATION-MATRIX.md` (290 líneas, sesión 5 §144).
- Estado: **CERRADO**. PASS: 37; PARTIAL: 11; NOT_RUN: 3; bloqueantes contractuales: 3 (PRF-MCP-05 enforcement, PRF-SEC-07 campaign, PRF-CI-07 disparador). Los 2 primeros cerrados en HEAD §145; el tercero operator-gated.

### 3. Cerrar fallos de correctitud — cerrado (parcialmente)
- **H-01 (ALTA): ✅ CERRADO** en `39928202` (decisión de diseño SHA-256).
- **H-02 (ALTA): ✅ CERRADO** en `80e7c403` (FullGraphStrategy::build_full_graph_report).
- **H-02 adicional — PRF-ANA-04: ✅ CERRADO** en `41e4230f` (BuildGraphOutput::status).
- **H-06 (ALTA): ✅ CERRADO** en `4feb5456` (B4 admisión) sobre `76e04e8e` (ciclo instalador).
- **H-06 adversarial campaign: ✅ CERRADO** en `ea34ff7d` (PRF-SEC-07 + PRF-MCP-05).
- **PRF-MCP-05 enforcement: ✅ CERRADO** en `ea34ff7d` (binario v0.98.1 verificado).

### 4. Pruebas ausentes C3–C6
- H-03 (vertical CLI↔MCP única): trabajo parcial sobre `find_usages` (PRF-ANA-04 + R1.1–R1.4).
- H-04 (persistencia material): documentado como “reconstrucción determinista sin persistencia” en RECONCILIATION-MATRIX.
- H-05 (extensión read-only + adversarial): pineado en PRF-SEC-07 + tests R1.1–R1.4.
- H-06 (ciclo install/update/rollback con downgrade): documentado en B3 §146 + B5 §148.
- H-07 (gate independiente por SHA): F6.W3.ter tag/workspace gate (`d40e61b2`) cumple el contrato.

### 5. T5 + decisión C7 sobre el candidato congelado
- **C5 ejecutada parcialmente** (release-validate #36062820528 success sobre
  v0.98.1). Faltan pruebas equivalentes sobre linux-aarch64 nativas
  (limitación honesta del entorno actual).
- **Decisión C7**: ✅ **PASS** firmado por el operador
  (`Ruben <rubentxu@cognicode.dev>`) el **2026-09-24T22:41:33Z** (UTC).
  Expediente completo: `docs/prf/F7-C7-EXPEDIENTE.md`. Material técnico
  verificado: release v0.98.1 publicada, SHA256 reproduce, MCP probe OK
  (20/20 tools authority=`read`), attestations, body de release con
  92 líneas de verificación. 5 excepciones aprobadas (E-C7-001 a E-C7-005).

## Notas de honestidad

- **No** se afirma ‘READY FOR RELEASE ≡ C7 PASS’ (la auditoría 2026-09-22 lo descarta).
- **No** se introduce un nuevo roadmap. Se cierra el PRF existente.
- **No** se reabre trabajo finalizado de F0–F2–F3; se conservan sus commits.
- **Sí** se ejecuta push, tag y release sobre autorización explícita del operador (2026-09-24T21:37:01Z).
- El gate del operador (firma C7 contractual, P0.2 cierre H06 adversarial campaign E2E, P0.3 PRF-CI-07, P0.4 0.97.x retirement) sigue siendo válido y adicional a este plan.
- **v0.98.1 incluye PRF-MCP-05 enforcement y PRF-SEC-07 adversarial en el binario publicado**, verificado vía probe MCP JSON-RPC stdio (§148).
- El bug menor `cogh rollback --to <same>` está documentado en §146 + §148 + cuerpo de la release v0.98.1.
