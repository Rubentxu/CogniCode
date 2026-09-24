# Admission Expediente — F7 / C7 sobre v0.98.0 (B4)

**Fecha:** 2026-09-24
**Estado:** DRAFT (STOP hasta decisión operador)
**Versión candidata:** v0.98.0 (release tag ya publicada, candidata lógica para F7)
**Candidata workspace:** `92ec698ab2e08479832c55b97126bd3f97ad6890` (HEAD actual, B1+B2+B3 commits)
**Commit v0.98.0 release-published:** `8505ad85` (workspace post-bump)
**Auditoría externa (recibida 2026-09-22):** sobre `93b7a9a3`

> Documento preparado por el agente principal al cierre del bloque B4 del plan
> prolongado del operador (B1→B2→B3→B4 AUTO, modo autónomo).
> **No es firma. No es push. No es tag. No es release.**
> Es la **lista de evidencias, gaps y opciones** que el operador necesita para
> decidir si firma F7/C7 sobre v0.98.0 o si exige candidata posterior.

---

## §1. Contexto de v0.98.0 publicada

- **Tag:** `v0.98.0` (firmado, NO; publicado por tag push workflow)
- **Release URL:** `https://github.com/Rubentxu/CogniCode/releases/tag/v0.98.0`
- **Published at:** 2026-09-24T17:36:17Z
- **Source commit (workspace):** `8505ad85` (post-bump `0.97.5`→`0.98.0`)
- **Tag commit (object):** `d99d3911…` (tag wrapper)
- **Asset count:** 13 archivos publicados
- **Run CI #36034410448:** `success` (5/5 jobs, binarios 0.98.0 firmados en el sitio)
- **Run CI #36038178581 (release-validate):** `success` sobre `d40e61b2` (post-release gate)
- **Run CI #36039255746 (release-validate):** `failure` — el **gate atrapó un mismatch workspace 0.99.0 vs release 0.98.0** (protegió contra confusión de candidata; PASS funcional del gate en sí).

**Release técnicamente publicada ✓.**
**Release contractualmente firmada (C7) NO** — la auditoría 2026-09-22 revocó `READY FOR RELEASE ≡ C7 PASS` (§138), y F6.W3.bis + F6.W3.ter + F6.W3.quarter se diseñaron específicamente para **no firmar automáticamente C7 al tag push**.

---

## §2. Estado de los 3 gaps bloqueantes identificados en B1 (REC ONCILIATION-MATRIX §7)

| Gap | Estado al cierre B4 | Evidencia | Cierra F7/C7 sobre v0.98.0? |
|---|---|---|---|
| **PRF-MCP-05 enforcement migration** | **CERRADO en HEAD** `ea34ff7d` | commit B2 (`ea34ff7d` feat(mcp/security)); tests verdes: 4/4 `prf_mcp_05`, 8/8 adversarial. `list_tools` y `read_only_mode` ahora consultan el campo declarado `cognicode.authority` con `MUTATING_TOOLS` como defensive subset-floor. | **NO automáticamente** (commit no publicado en release tag `8505ad85`); **SÍ como candidata v0.98.1** o vía addendum contractual. |
| **PRF-SEC-07 adversarial campaign** | **CERRADO en HEAD** `ea34ff7d` | commit B2 (mismo); 7 vectores MUST pineados: repo malicioso, symlink, parser binario, secreto señuelo, autoridad mutante, cliente desconectado, datos corruptos. | **NO automáticamente** (mismo motivo). |
| **PRF-CI-07 disparador automático** | **operator-gated** (no es código) | Política local-first documentada; gate demostrado OBSERVED por run #36033099039. Branch protection policy fuera del código. | **NO**; decisión governance, no candidata. |

**Conclusión**: la candidata v0.98.0 NO contiene los commits B1+B2+B3. Firmar C7 sobre v0.98.0 sin addendum sería firmar una release que **NO tiene PRF-MCP-05 enforcement ni PRF-SEC-07 adversarial campaign**.

---

## §3. Evidencias reutilizables desde v0.98.0 (no requieren candidata nueva)

§3.1 — Procedencia y chain-of-custody

- **PRF-DIST-01 (manifiesto canónico + sha256):** PASS contra v0.98.0
  - `bundle-0.98.0-{x86_64,aarch64}-unknown-linux-gnu.yaml` publicados
  - `release-inventory-0.98.0.json` con `source_commit: 8505ad85`
  - `SHA256SUMS` (12 hashes, todos verificados en `/tmp/prf-v098-dist/` y `/tmp/prf-v098-reconcile/`)
- **PRF-DIST-06 (procedencia desde release, no checkout):** PASS
  - Coherencia workspace↔tag↔SHA verificada por gate `d40e61b2` y runs `#36038178581` (success), `#36039255746` (gate atrapó mismatch, también éxito funcional).
- **PRF-CI-05 (parcial: SBOM + sha256 + smoke `release-install-smoke.sh` real):** PASS
- **PRF-CI-07 (gate demostrado):** PASS funcional
  - Run #36033099039: step 12 install-smoke salió 1 → draft-first safety net OK → pasos 13-21 SKIPPED → no publicación accidental.
- **PRF-F6-W3 bis/ter/quarter (5/5 jobs SUCCESS sobre run #36026057157):** PASS documentado en `ee834ff4` y `5cf910a7`.

§3.2 — Garantías heredables (código invariante entre `178f8a5b` y `8505ad85`)

Ver RECONCILIATION-MATRIX §4–§5 (~34 PASS heredables sobre ~60 total).

§3.3 — Distribución/instalación end-to-end (B3)

- DIST-02/03/05/01/06 PASS sobre `v0.98.0` ejecutando artefactos publicados en HOME aislado.
- MCP stdio JSON-RPC: 20 tools `read`, serverInfo.version=0.98.0 (ver §146).
- SHA256 reproduce en instalación limpia.
- Evidencia cruda: `/tmp/prf-v098-dist/`.

---

## §4. Hallazgos honestos del ciclo (no invalidantes, sí contractualmente relevantes)

### §4.1 — Bug menor del binario `cogh` v0.98.0 (instalador)

`cogh rollback --to <same-version>` deja estado parcial observable:
- tracker restored correctamente
- `versions/<v>/` y `shims/` borrados
- journal preservado con la lista de efectos, pero **no resucita el manifest desde el journal** (mensaje dice "retry with `cogh rollback --to <v>`" pero el retry falla con el mismo error)
- Recovery path probado: re-instalación limpia funciona y restaura doctor healthy

**Análisis**: cumple el contrato `refuses Ok on partial apply` (la release body línea `fix(cli): cmd_rollback owns shim resurrection, refuses Ok on partial apply` se cumple a medias: rechaza Ok pero la resurrección falla). El estado es **no-destructivo** (versions/ borrado pero cache/ y journal/ preservados, re-instalable desde red).

**Acción**: reportar upstream (issue contra `cogh`). NO bloquea F7/C7 sobre v0.98.0 porque el rollback es **un caso de borde** (rollback a la misma versión instalada), y la instalación limpia desde red es la solución oficial.

### §4.2 — El operador tenía `~/.cognicode/` apuntando a v0.97.3

Detectado durante B3: el `~/.cognicode/shims/cognicode-mcp` del operador apuntaba a `versions/0.97.3/`. La release v0.98.0 está publicada pero el operador no ha migrado su instalación personal.

**Acción recomendada al operador**: ejecutar `cogh update --ide opencode --ide zcode --ide claude --ide codex --channel stable` en su HOME real, o seguir la guía oficial de migración 0.97→0.98.

### §4.3 — `cogh install` no materializa `~/.cognicode/` markers

El install deja `tracker/version`, `versions/<v>/`, `shims/`, journal, `bundle.yaml`. Pero NO crea `.init`, ni `plugins/{codex,claude,zcode,mcp-server,sandbox-templates,skills-cognicode-core}/`, ni `bin/`. Esto requiere `cogh init` aparte.

**Acción**: documentar este orden en `docs/prf/UAT.md` o en una nota de release para v0.98.x.

### §4.4 — linux-aarch64 smoke NO ejecutado en este entorno

Limitación honesta del entorno actual (linux-x86_64 nativo). Si el operador exige verificación cross-arch para F7, requiere:
- runner nativo aarch64 (GitHub Actions `runs-on: ubuntu-24.04-arm`), o
- emulación QEMU con cargo-cross, o
- candidata posterior con job matrix

---

## §5. Opciones para F7/C7 firma (decisión operador)

### Opción A — Firmar F7/C7 sobre v0.98.0 con addendum contractual

**Descripción:** aceptar que la release v0.98.0 está publicada y firmar F7/C7 con un addendum firmado que reconozca los gaps B2 (PRF-MCP-05/SEC-07) NO incluidos en el binario publicado pero sí remediados en HEAD `92ec698a`.

**Pros:**
- Cierra el ciclo de release ya en producción
- Reutiliza todas las evidencias ya generadas (#36034410448, #36038178581, RECONCILIATION-MATRIX, §145, §146)
- Reconoce honestamente que el binario y el código divergen en estos 2 gaps

**Contras:**
- Establece precedente de "firmar addendum sobre release no-canónica"
- El binario en producción NO tiene los pineos B2 hasta que el operador haga `cogh update`
- Auditoría externa (2026-09-22) puede leerlo como evasión contractual

**Riesgo contractual:** medio-alto. Depende de la postura del operador sobre la relación código↔release.

### Opción B — Candidata posterior v0.98.1 con B2 incluidos

**Descripción:** generar candidata v0.98.1 que incluya los commits B2 (`ea34ff7d`, `9fa957ee`) y posiblemente el disparador PRF-CI-07 si hay decisión governance. Re-ejecutar ciclo completo:
1. bump workspace `0.98.0`→`0.98.1`
2. tag/workspace gate
3. release-validate SUCCESS sobre v0.98.1
4. release publish (idempotente: tag v0.98.1, draft-first)
5. ejecutar B3 sobre v0.98.1
6. firmar F7/C7 sobre v0.98.1 con todos los gaps cerrados en binario

**Pros:**
- F7/C7 firma contra binario que **sí tiene** PRF-MCP-05 enforcement + PRF-SEC-07 adversarial campaign
- Coherencia workspace↔binario↔release↔firma
- Auditoría externa coherente

**Contras:**
- 1-2 horas de pipeline + tiempo de release-validate
- v0.98.0 queda como release "técnica sin firma"; el operador debe comunicar que v0.98.1 la reemplaza contractualmente
- Costo de doble publicación (v0.98.0 + v0.98.1)

**Riesgo contractual:** bajo. Es el camino más limpio.

### Opción C — Cerrar C7 vía OBSERVED del run #36034410448 (sin candidata nueva)

**Descripción:** firmar C7 invocando el run CI real que publicó v0.98.0 como evidencia OBSERVED, declarando que la "firma C7" es el endoso del run CI verificado por la auditoría externa, no del binario en sí. Esta es la postura que el operador parecía insinuar en el ciclo F6.W3 (release v0.98.0 publicada por tag push sin firma C7 contractual).

**Pros:**
- Cero costo de pipeline
- Reutiliza el run CI existente como evidencia
- Consistente con la postura del release.yml (tag push publica, no firma)

**Contras:**
- Implica redefinición de C7: de "firma binario" a "firma proceso CI". Cambio semántico no trivial.
- Auditoría externa (2026-09-22) puede leerlo como "C7 era ya débil si se firma sobre CI y no sobre binario"
- El operador debe documentar este cambio de definición

**Riesgo contractual:** alto. Requiere aceptación explícita del operador y comunicación al auditor.

---

## §6. Recomendación del agente (no es firma, es análisis)

**Recomendación primaria: Opción B** (candidata v0.98.1 con B2 incluidos).

**Justificación:**
- Es el camino que minimiza ambigüedad contractual.
- El trabajo B2 ya está hecho (HEAD `ea34ff7d`); solo falta bumpear y publicar.
- El release.yml + draft-first safety net + release-validate.yml están operativos; el costo de pipeline es ~1 hora.
- Da coherencia workspace↔binario↔firma, que es la base del contrato PRF.
- Hace OBSOLETO el "addendum sobre v0.98.0" y reduce la deuda documental.

**Riesgo aceptable:**
- v0.98.0 queda como release técnica sin firma contractual (status quo actual).
- Operador debe comunicar a usuarios que migren a v0.98.1 (changelog en release body).

**Si el operador rechaza Opción B:**
- Opción A requiere addendum formal redactado por separado (no incluido en este expediente).
- Opción C requiere redefinición explícita de C7 (no recomendada, alto riesgo contractual).

---

## §7. Plan de validación `--validate` (preparado, no ejecutado)

Si el operador decide Opción B y quiere ejecutar release-validate sobre la candidata ANTES de tag/release, el comando es:

```bash
gh workflow run release-validate.yml \
  --repo Rubentxu/CogniCode \
  --ref <ref_o_branch>
```

Sin tag, sin publish. Run en frío espera ~5-15 min (3 jobs: build-bundle + install-smoke + negative-tests sobre el binario empaquetado).

**Para Opción B**, secuencia concreta:

```bash
# 1. Bump workspace 0.98.0 → 0.98.1
cd /var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode
# Editar Cargo.toml workspace members (cognicode-core, cognicode-mcp, cogh) + release-inventory version
# Commit bump + tag/workspace gate (NO crear tag aún)

# 2. release-validate sobre la rama con bump
gh workflow run release-validate.yml --repo Rubentxu/CogniCode --ref <branch>

# 3. Si release-validate SUCCESS → crear tag v0.98.1 sobre el bump commit
#    (draft-first safety net automáticamente)

# 4. Si release-validate SUCCESS → tag push publica v0.98.1
#    (run publish.yml sobre el tag)

# 5. Re-ejecutar B3 sobre v0.98.1 (DIST-02/03/05 sobre nuevos binarios)

# 6. Si B3 OK → firmar F7/C7 sobre v0.98.1
```

**Gate active**: ninguno de estos pasos se ejecuta sin orden explícita del operador.

---

## §8. Decisiones que necesito del operador

Marca una (o varias) y responde:

1. **¿Opción A, B o C?**
   - A → redactar addendum contractual y firma sobre v0.98.0 (recomendación: NO)
   - B → generar v0.98.1 con B2 incluidos (recomendación: SÍ) ← **recomendada**
   - C → redefinir C7 como firma sobre CI (recomendación: NO)

2. **Si Opción B**:
   - ¿El bump + release-validate + publish + B3 lo hago yo o el operador lo lanza manualmente?
   - ¿Se requiere ejecutarlo en CI antes de cualquier tag? (SÍ recomendado)
   - ¿linux-aarch64 se exige antes de firma, o se acepta como follow-up post-firma?

3. **Si Opción A**:
   - ¿Quién redacta el addendum (operador o agente)?
   - ¿Qué cláusulas mínimas debe contener?

4. **PRF-CI-07 disparador automático**:
   - ¿Se exige cerrar antes de firma? (cuesta branch protection policy o workflow file; ~30 min)
   - ¿Se acepta como follow-up post-firma documentado en F7?

5. **0.97.x retirement (P0.4)**:
   - ¿Bloqueante para F7 firma o follow-up?

6. **Push del trabajo B1+B2+B3 (commits no publicados)**:
   - ¿Push de los 20 commits ahead of origin/main antes de cualquier firma?
   - ¿O se acepta el "tag de release es workspace coherente" como suficiente?

---

## §9. Estado actual verificable

```text
HEAD local:      92ec698ab2e08479832c55b97126bd3f97ad6890
HEAD remoto:     03158085 (origin/main, 20 commits atrás)
Tag v0.98.0:     d99d3911… (publish commit)
Release v0.98.0: success #36034410448 (5/5 jobs)
release-validate: success #36038178581, gate-failure #36039255746 (ambos OK funcionalmente)
Working tree:    clean
Branch:          main (sin rama feature abierta)
Lockfile:        ausente (PID 3383777 no interfirió esta sesión)
```

---

## §10. Refs

- JOURNAL §138 (AUDIT findings), §140 (TRACEABILITY H11), §141 (H01 partition), §142 (H11 ampliación), §143 (H11-b archivado), §144 (B1 RECONCILIATION), §145 (B2 enforcement + adversarial), §146 (B3 distribución end-to-end).
- `docs/prf/RECONCILIATION-MATRIX.md` §4–§8.
- `docs/prf/AUDIT-2026-09-22-FINDINGS.md`.
- `docs/prf/STATE.md` (HEAD row, working tree, bloqueos conocidos).
- `/tmp/prf-v098-dist/` (evidencia cruda de B3).
- Release body v0.98.0 (`https://github.com/Rubentxu/CogniCode/releases/tag/v0.98.0`).

---

**STOP** — Este expediente NO se ejecuta sin orden explícita del operador.
Agente principal espera decisión sobre §8 antes de continuar.
