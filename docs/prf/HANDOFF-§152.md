# HANDOFF §152 — Sesión cerrada 2026-09-24T22:51:32Z (UTC)

> **Estado del programa PRF**: **CERRADO** — C7 firmado contractualmente.
>
> **Identidad verificada**: el operador `Ruben <rubentxu@cognicode.dev>`
> emitió la cadena "**firmo**" el 2026-09-24T22:41:33Z (UTC). Identidad
> confirmada contra `git cat-file -p v0.98.1` → `tagger Ruben <rubentxu@cognicode.dev>
> 1790286494 +0200` (tag anotado, no GPG-signed).
>
> **Este handoff es el punto de reanudación** para la sesión del
> 2026-09-25. Todo el contexto necesario para continuar está aquí o
> en los documentos versionados referenciados. **Nada se reconstruye por
> inferencia**: cada hecho cita su fuente.

## 1. Estado verificable al cierre de sesión (2026-09-24T22:51:32Z UTC)

```
HEAD local:       94cb44fae66d98406d47d918958b2b2b8a7ab46b
HEAD origin:      94cb44fae66d98406d47d918958b2b2b8a7ab46b  (synced)
Branch:           main, sin feature abierta
Working tree:     clean

Tag v0.98.1:      a21fccdabb9b5b15cd72351bcb593f5679ea72a8
                  → e4ab6c8e8d06b598ce55880d965d785c6710c777  (commit de release)
Tag v0.98.0:      d99d3911ded7ce807e5865905202843c7b5dda33
                  → 8505ad8506e68c85914eaaf2f71a7fde6149ed61

Release URL:      https://github.com/Rubentxu/CogniCode/releases/tag/v0.98.1
Release state:    isLatest=true, isDraft=false, isPrerelease=false
Published at:     2026-09-24T21:58:31Z (UTC)
Assets:           12 (SHA256 reproduce OK en /tmp/prf-v098-dist/)
Attestations:     actions/attest-build-provenance@v2 firmadas

Tests HEAD:       cognicode-core --lib 2188 passed, 0 failed, 27 ignored
                  prf_sec_07_adversarial_campaign 8/8 verde
                  prf_mcp_05 enforcement 4/4 verde

C7 firma:         PASS (2026-09-24T22:41:33Z UTC, operador "firmo")
Expediente:       docs/prf/F7-C7-EXPEDIENTE.md (268 líneas, 5 excepciones)

Bloqueos:         NINGUNO — PRF cerrado
Programa PRF:     CERRADO — C7 firmado
```

## 2. SHAs críticos a verificar al reanudar

```bash
# En orden de verificación al abrir la sesión del 2026-09-25:
cd /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode

git rev-parse HEAD                              # esperado: 94cb44fae66d98406d47d918958b2b2b8a7ab46b
git rev-parse origin/main                       # esperado: 94cb44fae66d98406d47d918958b2b2b8a7ab46b
git status --short --branch                     # esperado: clean, synced
git rev-parse v0.98.1                           # esperado: a21fccdabb9b5b15cd72351bcb593f5679ea72a8
git rev-parse v0.98.1^{commit}                  # esperado: e4ab6c8e8d06b598ce55880d965d785c6710c777

# Confirmación de release v0.98.1 (debe estar marcada como Latest):
gh release view v0.98.1 --repo Rubentxu/CogniCode \
  --json tagName,name,isDraft,isPrerelease,publishedAt  # isDraft=false, isPrerelease=false

# Estado de la release como Latest (la única Latest en el repo):
gh release list --repo Rubentxu/CogniCode --limit 1
# esperado: CogniCode v0.98.1	Latest	v0.98.1	2026-09-24T21:58:31Z
```

Si cualquiera de estas verificaciones falla, **STOP** — el estado fue
modificado fuera de esta sesión. Documentar divergencia y consultar
al operador antes de cualquier edición.

## 3. Documentos canónicos del programa PRF (versión actual)

### 3.1 Snapshot / fuente de verdad

| Archivo | Propósito | Líneas | Última edición |
|---|---|---|---|
| `docs/prf/STATE.md` | Snapshot + HEAD row + certificaciones + bloqueos | ~990 | §151 firma C7 |
| `docs/prf/CURRENT.md` | Puntero operativo (HEAD actual, decisiones pendientes) | 68 | §150 docs refresh |
| `docs/prf/JOURNAL.md` | Diario cronológico append-only completo | 12543 | §151 firma C7 |

### 3.2 Decisiones y evidencias contractuales

| Archivo | Propósito | Líneas |
|---|---|---|
| `docs/prf/F7-C7-EXPEDIENTE.md` | **Expediente de firma C7 sobre v0.98.1** (8 secciones, 5 excepciones) | 268 |
| `docs/prf/RELEASE-CANDIDATE.md` | SHA candidato congelado + decisión formal + cierre de PRF | 154 |
| `docs/prf/RECONCILIATION-MATRIX.md` | Reconciliación requisitos ↔ certificados (PASS: 37, PARTIAL: 11, NOT_RUN: 3) + §12 ratificación | 317 |
| `docs/prf/AUDIT-2026-09-22-FINDINGS.md` | Auditoría externa del operador (7 hallazgos ALTA + 1 transversal) | — |
| `docs/prf/DISTRIBUTION-SCOPE.md` | Single-source declaration del release pipeline | — |
| `docs/prf/ADMISSION-EXPEDIENTE-F7-C7-v0.98.0.md` | Expediente previo v0.98.0 (superseded por F7-C7-EXPEDIENTE.md) | — |

### 3.3 Documentación operacional (nueva §150)

| Archivo | Propósito | Líneas |
|---|---|---|
| `docs/prf/INSTALL-ORDER.md` | Orden verificado install → init → install --profile reviewer → doctor | 83 |
| `docs/prf/COGH-ISSUES.md` | 3 issues del binario `cogh` con recovery paths (ISSUE-1 severidad media: rollback --to <same>) | 116 |

### 3.4 Documentos históricos (consultables, no fuente de verdad)

- `docs/prf/STATE-SNAPSHOT.md` (snapshot antiguo, no vigente)
- `docs/prf/HANDOFF-§125.md` (handoff de sesión 2026-09-23)
- `docs/prf/GUARANTEES_ROLLUP_2026-09-24.md` (rollup intermedio)
- `docs/prf/REMOTE-VALIDATE-PROPOSAL-2026-09-24.md` (propuesta técnica)
- `docs/prf/HANDOFF-§152.md` (este documento)

## 4. Trazabilidad del cierre B1→B5→firma C7

| § | Etapa | Commit(s) principal(es) | Resultado |
|---|---|---|---|
| §144 | B1 RECONCILIATION-MATRIX sobre candidata v0.98.0 | `7e6b1270` | PASS: 37, PARTIAL: 11, NOT_RUN: 3 |
| §145 | B2 PRF-MCP-05 enforcement + PRF-SEC-07 adversarial campaign | `ea34ff7d`, `9fa957ee` | +22 tests verdes (2166 → 2188) |
| §146 | B3 distribución + instalación + recuperación sobre v0.98.0 | `bb0f2edb`, `92ec698a` | UAT-B3 PASS; 3 issues cogh descubiertos |
| §147 | B4 Admission Expediente F7/C7 sobre v0.98.0 — STOP hasta operador | `4feb5456`, `a83ea210` | Documentación + STOP |
| §148 | V42 release v0.98.1 publicada | `e4ab6c8e`, `b67c9deb` | release-validate #36062820528 SUCCESS, release.yml #36063804784 SUCCESS |
| §149 | V43 integración final v0.98.1 | `0d6c575e` | release body enrichment (92 líneas); attestations verificadas |
| §150 | V44 docs refresh post-release | `0044f73f`, `422f9514` | CURRENT.md + RELEASE-CANDIDATE.md freshen + INSTALL-ORDER.md + COGH-ISSUES.md |
| §151 | V45 firma C7 contractual | `1a17c8be`, `94cb44fa` | Expediente F7-C7-EXPEDIENTE.md, RECONCILIATION §12 ratificación, RELEASE-CANDIDATE §Decisión PASS, STATE snapshot C7 |

## 5. Evidencia cruda preservada (local-only)

Directorio: `/tmp/prf-v098-dist/` (~12 MB)

```
/tmp/prf-v098-dist/
├── bundle-0.98.0-x86_64-unknown-linux-gnu.yaml
├── cogh-0.98.0-x86_64-unknown-linux-gnu.tar.gz       # 3.3 MB
├── cognicode-0.98.0-x86_64-unknown-linux-gnu.tar.gz  # 8.9 MB
├── dist0981/                                          # B5 sobre v0.98.1 (binarios + home3 + staging)
├── extracted/                                         # binarios extraídos
├── home1/                                             # install inicial v0.98.0
├── home2/                                             # upgrade install v0.98.0
├── home3/                                             # install v0.98.1
├── probe.isolated.jsonrpc.out                         # MCP JSON-RPC probe sobre v0.98.1 (isolated home)
├── probe.jsonrpc.out                                  # MCP JSON-RPC probe sobre v0.98.1
├── release.json                                       # GitHub API v0.98.0
├── release2.json                                      # GitHub API v0.98.1
├── release-inventory-0.98.0.json
├── serve.isolated.stderr
├── serve.stderr
├── SHA256SUMS
└── staging/                                           # staging dir usado en install/upgrade
```

**Propósito de preservación**: poder re-ejecutar el ciclo completo
install → doctor → CLI → MCP → update → rollback → uninstall en cualquier
momento sin re-descargar de red. También útil para reproducir la
campaña adversarial y los probes JSON-RPC.

**Riesgo de preservación**: `/tmp` se borra entre reboots; si el host
se reinicia esta evidencia se pierde. Si el operador quiere
preservación cross-reboot, considerar copia a `~/.cognicode/evidence/`
o `/var/mnt/.../CogniCode/evidence/` (path persistente).

## 6. Items operator-gated pendientes (NO bloquean C7 firmado)

Estos quedan documentados pero **NO requieren acción inmediata** porque
C7 ya está firmado. Son mejora incremental fuera del cierre contractual.

### 6.1 Governance / CI

| ID | Descripción | Estimación |
|---|---|---|
| P0.3 / PRF-CI-07 | Disparador automático en push-PR (branch protection + workflow file) | ~30 min cuando se active |

### 6.2 Seguridad / adversarial

| ID | Descripción | Estimación |
|---|---|---|
| P0.2 / H06 | Adversarial campaign E2E sobre binario en red hostil (actualmente pineado en cargo test, no campaña E2E) | ~2-4h |

### 6.3 Lifecycle / scope

| ID | Descripción | Estimación |
|---|---|---|
| P0.4 | 0.97.x retirement (v0.97.3 sigue publicada; decisión de scope) | Variable |
| ISSUE-1 cogh | Bug `cogh rollback --to <same>` — recovery path verificado, fix upstream pendiente | Variable |

### 6.4 Mejoras incrementales fuera del cierre contractual

- H-clippy-cli-residual (D34-2): catálogo de warnings preexistentes en `cognicode-cli` (unused_imports, dead_code).
- moldql panic test preexistente en explorer.
- find_usages CLI equivalente al MCP tool (R1.1-R1.4 ya pineados en cargo test).

## 7. Decisiones tomadas por el operador durante esta sesión

| Timestamp (UTC) | Comando | Efecto |
|---|---|---|
| `2026-09-24T21:37:01Z` | "sube todo crea release tag lo que sea" | Push 22 commits + bump 0.98.0→0.98.1 + tag + release |
| `2026-09-24T22:10:11Z` | "crea release e integra" | Enrichment del release body + verificación de coherencia |
| `2026-09-24T22:32Z` | "A tu criterio" | Carte blanche: agent selecciona items dentro de autonomía |
| `2026-09-24T22:41:33Z` | "firmo" | **C7 firmado contractualmente sobre v0.98.1** |
| `2026-09-24T22:51:32Z` | "cerramos sesion persiste todo el contexto del trabajo actual para mañana" | Cierre de sesión + handoff |

## 8. Acciones de reanudación (sesión 2026-09-25)

### 8.1 Verificación de invariantes (orden estricto)

```bash
cd /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode

# 1. Estado git
git status --short --branch                         # debe ser clean, synced

# 2. Tests verdes sobre HEAD (smoke rápido, no exhaustivo)
cargo test -p cognicode-core --lib --quiet          # 2188/0/27 esperado
cargo test -p cognicode-core --test prf_sec_07_adversarial_campaign --quiet  # 8/8 esperado
cargo test -p cognicode-core --lib prf_mcp_05 --quiet                       # 4/4 esperado

# 3. Tag anotado y release siguen vigentes
git rev-parse v0.98.1                               # a21fccda esperado
git rev-parse v0.98.1^{commit}                      # e4ab6c8e esperado
gh release view v0.98.1 --repo Rubentxu/CogniCode \
  --json isDraft,isPrerelease                       # ambos false

# 4. Documentos canónicos existen y son legibles
test -f docs/prf/STATE.md && echo "STATE OK"
test -f docs/prf/CURRENT.md && echo "CURRENT OK"
test -f docs/prf/JOURNAL.md && echo "JOURNAL OK"
test -f docs/prf/F7-C7-EXPEDIENTE.md && echo "EXPEDIENTE OK"
test -f docs/prf/RELEASE-CANDIDATE.md && echo "RC OK"
test -f docs/prf/RECONCILIATION-MATRIX.md && echo "MATRIX OK"

# 5. Evidencia cruda preservada (si sigue en /tmp)
ls /tmp/prf-v098-dist/SHA256SUMS 2>&1               # si falta, el host se reinició
```

### 8.2 Si todo verifica → opciones para el operador

El operador decide entre:
1. **Cerrar el día** — no hacer más trabajo hasta nueva orden.
2. **Trabajar en operator-gated pendientes** — elegir uno de §6 y abrirlo.
3. **Mejoras incrementales** — elegir uno de §6.4.
4. **Nueva F-unit** — abrir trabajo nuevo no contemplado en el roadmap actual.

### 8.3 Si algo no verifica

- **HEAD != `94cb44fa`**: STOP. `git diff 94cb44fa HEAD` para ver qué cambió. Consultar al operador antes de cualquier edición.
- **Tag v0.98.1 != `a21fccda`**: STOP. El tag anotado es la identidad contractual firmada; modificarlo invalida C7.
- **Working tree != clean**: STOP. `git status` para ver cambios sin commit. Posible pérdida de contexto si los cambios se descartan sin revisión.
- **Tests no verdes**: STOP. Identificar qué test falló y por qué. NO hacer `cargo test --release` por la regla de tiempo de compilación; usar `cargo check` + `cargo test` debug.
- **Evidencia cruda faltante en /tmp**: NO crítico para el programa PRF (todo está versionado en git). Documentar si se requiere para futura re-ejecución de campañas.

## 9. Lo que NO debe hacer la sesión del 2026-09-25 sin orden explícita

- **NO abrir nuevas F-units** — el programa PRF está cerrado.
- **NO modificar el tag anotado `v0.98.1`** — es la identidad contractual firmada.
- **NO crear nuevas releases** sin orden explícita del operador.
- **NO re-firmar C7** — ya está firmado; un nuevo intento se considera duplicado contractual.
- **NO abrir RPC/Control Plane/packs/IA autónoma** sin orden explícita (regla AGENTS.md).
- **NO borrar /tmp/prf-v098-dist/** sin antes archivar la evidencia si el operador la considera útil.

## 10. Reglas operativas recordatorias

- **Path reassignment bloqueado por risk-assessment** — usar siempre paths absolutos.
- **Isolation guard** — `git check-ignore docs/prf/*` antes de `git commit`; usar `git add -f` para los gitignored.
- **Commits atómicos** — un cambio lógico por commit, conventional commits.
- **Self-roll discipline** — un self-roll por bloque; HEAD row apunta a penúltimo commit.
- **No push automático** — solo cuando el operador autoriza explícitamente.
- **No firma C7 automática** — solo cuando el operador emite orden explícita.

## 11. Identidad y permisos

- **Operador**: `Ruben <rubentxu@cognicode.dev>`
- **Identidad git verificada contra**: tag anotado `v0.98.1` (`a21fccda` → commit `e4ab6c8e`).
- **Permisos del agente**: agente autónomo con capacidad de push, tag, release, force-add docs/prf/*. NO tiene capacidad de firmar C7 sin orden explícita.
- **Sesión paralela**: `cognicode-prf` (PID 3383777) — sigue durmiendo, no interferida.

## 12. Resumen ejecutivo (TL;DR)

**CogniCode v0.98.1 es la release production-ready contractual del programa PRF.**

- HEAD: `94cb44fa` (main, clean, synced).
- Release: v0.98.1 publicada 2026-09-24T21:58:31Z, marked as Latest.
- C7: PASS firmado contractualmente por el operador el 2026-09-24T22:41:33Z UTC con la cadena "firmo".
- Expediente: `docs/prf/F7-C7-EXPEDIENTE.md` (268 líneas, 5 excepciones aprobadas).
- Tests: 2188/0/27 (cognicode-core lib), 8/8 adversarial, 4/4 PRF-MCP-05.
- PRF cerrado con firma humana; operator-gated pendientes son mejora incremental.

**Próxima sesión 2026-09-25**: operador decide. Sin orden explícita, no abrir nuevas F-units.

---

**HANDOFF §152 — generado 2026-09-24T22:51:32Z (UTC)**

Agente: Jcode (orquestador SDDK + autonomía operativa sobre el programa PRF)
Sesión cerrada por orden explícita del operador: "cerramos sesion persiste todo el contexto del trabajo actual para mañana"
Persistencia: este documento + todos los docs/prf/* versionados en git + evidencia cruda en /tmp/prf-v098-dist/
Resumibilidad: §8 contiene las acciones exactas de reanudación para 2026-09-25.
