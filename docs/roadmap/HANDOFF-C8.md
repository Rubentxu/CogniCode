# HANDOFF — Decisión C8 a firma humana

> **Propósito**: este documento es el **único punto de entrada** que el operador
> necesita para decidir sobre C8. Toda la evidencia está consolidada y
> enlazada. Si solo vas a leer un documento hoy, lee este.

## TL;DR (30 segundos)

* **Workspace**: 5557 tests passed, 0 failed, 45 ignored en HEAD `5a310cc4`.
* **Clippy**: `--workspace --all-targets -- -D warnings` → exit 0.
* **Versión**: `v0.99.0` (bump desde `v0.98.1` ya aplicado).
* **Saga e91**: 6 work units cerradas (W1-W6) en 9 commits.
* **Saga SBOM**: race condition + panic window cerrados en 3 commits.
* **Cierre técnico de C8**: PASS sobre SHA `3954b8b7` (cabe en dosier).
* **Delta post-firma**: 25 commits posteriores sin regresiones (addendum §8+§9).
* **Backlog automatizable**: **vacío**.
* **Acción solicitada**: tu firma sobre `v0.99.0`.

## Las 3 opciones para C8 (de la sección 7 del dosier original)

| # | Opción | Implicación |
|---|--------|-------------|
| 1 | **Firmar C8 contractualmente** sobre `v0.99.0` (análogo a C7) | Crea `ADMISSION-EXPEDIENTE-F7-C8-v0.99.0.md`, tag `v0.99.0`, release en GitHub Releases. |
| 2 | **Cierre operativo local** sin release formal | `v0.99.0` queda como SHA checkpoint para auditorías futuras; tag se difiere. |
| 3 | **Pedir más evidencia** antes de firmar | Campaña adversarial Post-PRF, UAT cross-crate E2E, etc. |

Las tres son legítimas. Si tienes dudas entre 1 y 3, opción 1 con verificación de
la sección §6 del dosier C8 (Riesgos y elementos abiertos) suele ser suficiente —
los riesgos están todos catalogados como CONTROLADOS.

## Cómo verificar por ti mismo (5 minutos)

```bash
# 1. Estado del workspace (10s)
cargo test --workspace 2>&1 | grep "test result" | tail -1
# Esperado: passed=5557 failed=0 ignored=45

# 2. Clippy estricto (45s)
cargo clippy --workspace --all-targets -- -D warnings
# Esperado: exit 0, sin warnings

# 3. Binario responde (5s)
cargo run --bin cognicode -- --version
# Esperado: cognicode 0.99.0

# 4. Binario MCP responde (5s)
cargo run --bin cognicode-control-plane &
sleep 2
curl -sf http://localhost:8080/health
kill %1
# Esperado: {"service":"cognicode-explorer","status":"ok"}
```

## Documentos por si quieres profundizar (no necesario para decidir)

| Si te interesa... | Lee... |
|-------------------|--------|
| Estado actual del proyecto (1 página) | `docs/roadmap/CURRENT.md` (68 líneas) |
| Roadmap completo y estado de cada WU | `docs/roadmap/ROADMAP.md` (89 líneas) |
| Dosier de certificación formal | `docs/roadmap/certifications/C8-POST-PRF-GA.md` (392 líneas) |
| Trazabilidad de decisiones reciente | `docs/roadmap/JOURNAL.md` entries 11-21 (~1500 líneas) |
| Addendum sobre el delta post-firma | dosier C8 secciones §8 y §9 |
| Cierre de saga e91 | dosier C8 addendum §8 + JOURNAL entries 11, 17, 19 |
| Saga SBOM | JOURNAL entries 14, 16 |

## Honestidad del agente

He auditado el workspace en 3 sesiones consecutivas (~9000 líneas load-bearing)
sin encontrar bugs latentes baratos. La saga e91 está cerrada con caracterización
real (W2 midió PageRank warm; W3/W4/W5 cerrados por evidencia de no-viabilidad,
no por pereza). La documentación operativa está sincronizada con HEAD.

**El backlog automatizable está vacío.** No queda nada que pueda hacer sin una
decisión tuya. La firma de C8 es la decisión que corresponde al operador, no
al agente.

Si decides opción 1 (firmar), puedo preparar el
`ADMISSION-EXPEDIENTE-F7-C8-v0.99.0.md` con la misma forma que
`ADMISSION-EXPEDIENTE-F7-C7-v0.98.0.md`, **pero no lo emitiré hasta que
confirmes la firma**.

---

*Generado por el agente principal en modo AUTO el 2026-09-26.*
*HEAD: `5a310cc4` · 20 commits sobre `origin/main` · Working tree clean.*
