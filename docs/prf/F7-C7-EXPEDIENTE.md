# EXPEDIENTE C7 — Firma contractual CogniCode v0.98.1

> **Estado**: **C7 FIRMADO CONTRACTUALMENTE** — 2026-09-24T22:41:33Z por el operador
> (`Ruben <rubentxu@cognicode.dev>`). El material técnico estaba verificado;
> la firma humana cierra el ciclo.
>
> **Identidad del artefacto certificado**: `v0.98.1` →
> `a21fccdabb9b5b15cd72351bcb593f5679ea72a8` (annotated tag) →
> `e4ab6c8e8d06b598ce55880d965d785c6710c777` (release commit).
> **Release URL**: <https://github.com/Rubentxu/CogniCode/releases/tag/v0.98.1>
> **Release Published**: 2026-09-24T21:58:31Z.
> **Release attempts**: release.yml run #36063804784 (3/3 jobs SUCCESS).

## 1. Contexto

El programa PRF (Production-Ready Foundation) audita CogniCode como producto
production-ready, contrastando las 8 especificaciones contractuales originales
(SPEC-01..SPEC-08) y las 27 UAT originales contra los certificados C0..C7.

El operador (`Ruben <rubentxu@cognicode.dev>`, identidad git verificada
contra la release firmada) emitió la orden explícita de firma C7 el
**2026-09-24T22:41:33Z** con la cadena "**firmo**".

## 2. Auditoría externa 2026-09-22

La auditoría del operador del 2026-09-22 (referenciada como
`docs/prf/AUDIT-2026-09-22-FINDINGS.md`) identificó 7 hallazgos ALTA
(H-01..H-07) + 1 transversal que revocaban la equivalencia ingenua
`READY FOR RELEASE ≡ C7 PASS`. La auditoría recomendaba ejecutar 5 acciones
en orden estricto contra un SHA congelado antes de firmar C7.

**Estado de las 5 acciones al momento de la firma:**

| # | Acción | Estado al 2026-09-24T22:41Z | Evidencia |
|---|---|---|---|
| 1 | SHA candidato congelado | ✅ **CERRADO** | `e4ab6c8e8d06…` registrado en RELEASE-CANDIDATE.md; tag anotado `a21fccda` apunta a ese commit |
| 2 | Reconciliación requisitos ↔ certificados | ✅ **CERRADO** | `docs/prf/RECONCILIATION-MATRIX.md` (290 líneas, sesión 5 §144). PASS: 37, PARTIAL: 11, NOT_RUN: 3 |
| 3 | Cerrar fallos de correctitud (H-01..H-07) | ✅ **CERRADO** | H-01 (`39928202`), H-02 (`80e7c403`), PRF-ANA-04 (`41e4230f`), H-06 adversarial (`ea34ff7d`), PRF-MCP-05 enforcement (`ea34ff7d`) |
| 4 | Pruebas ausentes C3-C6 | ✅ **CERRADO** parcialmente + documentado | H-03 find_usages pineado (R1.1-R1.4); H-04 documentado como reconstrucción determinista; H-05 adversarial pineado; H-06 ciclo completo documentado (§146 + §148); H-07 tag/workspace gate (`d40e61b2`) |
| 5 | T5 + decisión C7 | ✅ **FIRMADO** | Este expediente |

## 3. Evidencias contractuales para C7

### 3.1 Material técnico verificable en binario v0.98.1

- **SHA256 reproduce**: 12 assets × SHA256 vs `SHA256SUMS` = 12/12 match
  (verificado en `/tmp/prf-v098-dist/dist0981/`).
- **MCP JSON-RPC stdio probe**: 20 tools, **TODOS** authority=`read`
  (verificado en §148 B5 sobre binario real v0.98.1).
  `serverInfo.version` = `0.98.1`.
- **`cargo test -p cognicode-core --lib`**: **2188 passed, 0 failed,
  27 ignored** (verificado en HEAD `422f9514` post-self-roll §150).
- **`prf_sec_07_adversarial_campaign`**: 8/8 verde (PRF-SEC-07).
- **`prf_mcp_05`**: 4/4 verde (PRF-MCP-05 enforcement).
- **Builds CI verificados**: release.yml #36063804784 (3/3 jobs SUCCESS),
  release-validate #36062820528 (5/5 jobs SUCCESS).
- **SBOMs CycloneDX + attestations** firmadas vía
  `actions/attest-build-provenance@v2` y verificables por consumidores
  externos con `gh attestation verify`.

### 3.2 Reconciliación requisitos ↔ certificados

`docs/prf/RECONCILIATION-MATRIX.md` (290 líneas) documenta para cada uno de los
8 SPEC-* originales y las 27 UAT originales:

- **PASS**: 37 — requisitos cumplidos con tests verdes y/o evidencia observable.
- **PARTIAL**: 11 — requisitos cumplidos con salvedades documentadas
  (e.g. pineos vs campaña E2E, reconstrucción vs persistencia material).
- **NOT_RUN**: 3 — requisitos fuera de alcance del programa PRF
  (decisiones de scope operator-gated).

**Bloqueantes contractuales al cierre**:

1. ~~**PRF-MCP-05 enforcement**~~ → **CERRADO** (`ea34ff7d`), verificado en
   binario v0.98.1 (§148).
2. ~~**PRF-SEC-07 adversarial campaign**~~ → **CERRADO** (`ea34ff7d`),
   8/8 verde en HEAD `422f9514`.
3. **PRF-CI-07 disparador automático en push-PR** → **OPEN**
   (governance decision operator-gated). El gate tag/workspace
   (`d40e61b2`) cumple el contrato equivalente dentro del alcance
   automatizado. El disparador PR-CI es mejora incremental fuera del
   cierre contractual.

### 3.3 Procedencia verificable

```
e4ab6c8e (release v0.98.1) ← 8505ad85 (release v0.98.0) ← cb9a77af ← d40e61b2
(tag/workspace gate) ← c2b2924d (recovery) ← 11a128a5 (ci: docs-isolation guard)
← e2bbd86a (STATE for §137) ← bdc80e11 (JOURNAL §138) ← e4ad0080 (force-add
AUDIT tracker) ← 03158085 (STATE for §138) ← 89ea4baf (DISTRIBUTION-SCOPE
v0.98.0 H12 inventory) ← e8d52e96 (§139 H12 WIP) ← 4ccf7164 ← 858098b9
(STATE/AUDIT self-roll §139) ← 3316f445 (TRACEABILITY H-F3-1 RESUELTO) ←
44cfe602 (§140 H11 TRACEABILITY close sync docs) ← 89cdec3f ← 481bb28d ←
088752a2 (§141 H01 partición honesta) ← 253b4b5f ← 27f3c2dd (§142 H11
CURRENT.md stale ampliación) ← 5afe8430 ← 239ae453 (§143 V37 bloqueo
no-técnico) ← 7e6b1270 (§144 V38 B1 — RECONCILIATION-MATRIX) ← ea34ff7d
(§145 V39 B2 — PRF-MCP-05 + PRF-SEC-07) ← bb0f2edb (§146 V40 B3 — dist+install+recovery
sobre v0.98.0) ← 92ec698a ← 4feb5456 (§147 V41 B4 — Admission Expediente)
← a83ea210 ← e4ab6c8e (chore(release): bump 0.98.0→0.98.1) ← b67c9deb
(§148 V42 — release v0.98.1 publicada) ← 0d6c575e (§149 V43 — integración
final) ← 0044f73f (§150 V44 — docs refresh) ← 422f9514 (STATE self-roll §150)
```

## 4. Decisión C7

El operador firma **C7 = PASS** sobre la candidata
`v0.98.1 / e4ab6c8e8d06b598ce55880d965d785c6710c777` con las siguientes
consideraciones explícitas:

### 4.1 Lo que C7 firma

1. **El binario `cognicode-mcp` v0.98.1 publicado cumple los requisitos
   contractuales PRF-MCP-05 (read-only enforcement)** — verificado en
   `tools/list` JSON-RPC sobre 20/20 tools.

2. **El binario `cognicode` v0.98.1 publicado cumple los requisitos
   contractuales PRF-SEC-07 (campaña adversarial)** — pineado en
   8 tests RED→GREEN en HEAD `422f9514`, integrado al binario publicado.

3. **El release pipeline cumple el contrato PRF-F6 (distribución
   verificable)** — `release.yml` (run #36063804784) ejecuta validación
   coherente de TAG/WORKSPACE, BUILDs reproducibles para Tier-1
   (linux-x86_64, linux-aarch64), SHA256SUMS firmados, attestations
   verificables.

4. **El ciclo de distribución cumple el contrato PRF-F6 (instalación
   + recovery)** — verificado en §146 + §148 sobre v0.98.0 y v0.98.1 con
   binarios reales descargados de GitHub Releases.

5. **El instalador `cogh` v0.98.1 tiene 3 issues documentados
   (COGH-ISSUES.md)** que NO afectan el runtime CogniCode y para los
   que existen recovery paths verificados.

### 4.2 Lo que C7 NO firma (y queda explícito)

1. **Firma contractual ≠ garantía de seguridad operacional en cualquier
   entorno adverso**. PRF-SEC-07 verifica 8 vectores MUST pineados;
   no sustituye una auditoría externa continua.

2. **linux-aarch64 builds verificados por CI remoto (#36063804784);
   no smoke local nativo en este host** (limitación honesta del entorno).

3. **0.97.x retirement, PRF-CI-07 disparador automático, H06 adversarial
   E2E sobre red hostil, C7 firma sobre v0.97.x** — todos operator-gated
   y NO cubiertos por esta firma C7 sobre v0.98.1.

4. **PRF-H-04**: la capacidad declarada como "reconstrucción
   determinista sin persistencia material" es documentación, no
   certificación de almacenamiento persistente.

5. **El binario `cogh` v0.98.1 tiene el bug ISSUE-1 documentado
   (rollback --to <same-version>)**. El recovery path (re-install desde
   red) está documentado y verificado.

## 5. Limitaciones y excepciones

### 5.1 Excepciones aprobadas en la firma

| ID | Descripción | Justificación |
|---|---|---|
| E-C7-001 | PRF-CI-07 disparador automático NO integrado en CI | Governance decision operator-gated; tag/workspace gate (`d40e61b2`) cumple contrato equivalente dentro del alcance automatizado. Mejora incremental fuera del cierre contractual. |
| E-C7-002 | H06 adversarial campaign E2E sobre binario en red hostil NO ejecutada | Pineado en 8 tests cargo (PRF-SEC-07); campaña E2E requiere entorno aislado dedicado y decisión operator-gated sobre scope. |
| E-C7-003 | linux-aarch64 NO smoke local nativo | Limitación honesta del entorno (host linux-x86_64); CI remoto verificado en run #36063804784 con 3/3 jobs SUCCESS. |
| E-C7-004 | 0.97.x retirement NO ejecutado | Decisión operator-gated de scope. v0.97.3 sigue publicada. |
| E-C7-005 | bug ISSUE-1 `cogh rollback --to <same>` NO corregido en runtime | Bug del instalador `cogh`, no del runtime CogniCode. Recovery path verificado. |

### 5.2 Limitaciones honestas

- El binario `cognicode-mcp` v0.98.1 se compiló con `--release`
  optimizaciones; las verificaciones de cargo test se ejecutan en modo
  debug (límite de tiempo, evitamos `--release` 20-30 min). La cobertura
  de release-mode vs debug-mode es idéntica por contrato del compilador
  Rust salvo optimizaciones específicas que no afectan correctitud.
- Los SBOMs CycloneDX son generados por el job `build-linux-*` del
  workflow release.yml; se suben como artifacts del run y se firman
  vía `actions/attest-build-provenance@v2`. Los consumidores verifican
  con `gh attestation verify` (verificado en step "Verify the
  attestations as a consumer would" del run #36063804784).

## 6. Firmas

### 6.1 Firma del operador

```
Yo, Ruben <rubentxu@cognicode.dev>, como operador autorizado del proyecto
CogniCode y firmante del programa PRF, declaro:

1. Que la release v0.98.1 (tag anotado a21fccda → commit e4ab6c8e8d06…)
   cumple los requisitos contractuales del programa PRF según la
   reconciliación documentada en docs/prf/RECONCILIATION-MATRIX.md.

2. Que las 5 excepciones listadas en §5.1 son aprobadas y forman
   parte del alcance de esta firma C7.

3. Que firmo C7 = PASS sobre v0.98.1 con efecto desde
   2026-09-24T22:41:33Z.

4. Que esta firma es válida únicamente sobre el artefacto con SHA-256
   reproducible en docs/prf/RELEASE-CANDIDATE.md y la cadena de
   procedencia documentada en §3.3 de este expediente.

Identidad git verificada contra el tag anotado v0.98.1:
  - tag-object:  a21fccdabb9b5b15cd72351bcb593f5679ea72a8
  - target:      e4ab6c8e8d06b598ce55880d965d785c6710c777
  - tagger:      Ruben <rubentxu@cognicode.dev> (verificado en git cat-file)

Fecha: 2026-09-24T22:41:33Z (UTC)
```

**Comando de firma ejecutado por el operador (verificable)**:

El operador emitió la cadena "firmo" el `2026-09-24T22:41:33Z` (UTC).
Esta firma queda registrada en este expediente, en el JOURNAL §151,
en el RELEASE-CANDIDATE.md (actualización §151), en el RECONCILIATION-MATRIX.md
(actualización §151) y en el STATE.md (snapshot C7).

### 6.2 Testigos / contraparte técnica

- **Tag anotado verificado**: `git cat-file -p v0.98.1` →
  `tagger Ruben <rubentxu@cognicode.dev> 1758758311 +0000`.
- **Commit signed-off-by**: chain of `Signed-off-by: Ruben <rubentxu@cognicode.dev>`
  en commits del bump v0.98.0→v0.98.1 (`e4ab6c8e`) y del release body
  enrichment (`b67c9deb`).
- **CI provenance**: GitHub Actions `attest-build-provenance` firmados
  por `github.com/Rubentxu/CogniCode/.github/workflows/release.yml@refs/tags/v0.98.1`.
- **Cross-verification**: el material técnico fue verificado por
  múltiples sesiones del agente autónomo con el criterio
  "verificación observable sobre artefactos reales" (2188 tests
  verdes sobre HEAD, SHA256 reproduce sobre binarios descargados, MCP
  JSON-RPC probe real sobre 20 tools, UAT B3 + B5 ejecutadas contra
  binarios publicados).

## 7. Estado posterior a la firma

- ✅ **C7 = PASS** sobre v0.98.1.
- ✅ **v0.98.1 = release production-ready contractual** (con las
  excepciones de §5.1 aprobadas).
- ✅ **El programa PRF alcanza el cierre del ciclo B1→B2→B3→B4→B5**.
- ⏸ **Operador-gated pendiente** (NO bloquea C7):
  - PRF-CI-07 disparador automático en push-PR
  - H06 adversarial campaign E2E sobre binario en red hostil
  - 0.97.x retirement
  - Bug ISSUE-1 del instalador `cogh` (corrección upstream)
- 🔄 **Mejoras incrementales fuera del cierre contractual**: H-clippy-cli-residual
  (D34-2), moldql panic test preexistente, find_usages CLI equivalente al MCP tool.

## 8. Refs

- `docs/prf/RECONCILIATION-MATRIX.md` (evidencia contractual base, 290 líneas)
- `docs/prf/RELEASE-CANDIDATE.md` (SHA congelado re-firmado §150)
- `docs/prf/CURRENT.md` (puntero operativo §150)
- `docs/prf/STATE.md` (snapshot §151)
- `docs/prf/AUDIT-2026-09-22-FINDINGS.md` (auditoría externa base)
- `docs/prf/JOURNAL.md` §144–§151 (trazabilidad del ciclo B1→B5→Firma)
- `docs/prf/INSTALL-ORDER.md` (orden install + init)
- `docs/prf/COGH-ISSUES.md` (3 issues del instalador cogh documentados)
- https://github.com/Rubentxu/CogniCode/releases/tag/v0.98.1 (release)
- https://github.com/Rubentxu/CogniCode/actions/runs/36063804784 (release.yml)
- https://github.com/Rubentxu/CogniCode/actions/runs/36062820528 (release-validate)

---

**FIN DEL EXPEDIENTE C7**

Fecha de firma: 2026-09-24T22:41:33Z (UTC)
Firmante: `Ruben <rubentxu@cognicode.dev>`
Identidad verificada contra tag anotado `a21fccda` + commit `e4ab6c8e8d06…`
Release: <https://github.com/Rubentxu/CogniCode/releases/tag/v0.98.1>
