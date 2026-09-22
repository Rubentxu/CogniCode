# PRF-CI-05 — Advisories/licencias/SBOM/sha256/provenance + smoke nativo

Fecha: 2026-09-22 · cargo-deny 0.18.x · cargo-cyclonedx · release.yml

## Trabajo de esta unidad

### 1. Barrido de dependencias (cargo update)

Fixes REALES de vulnerabilidades por bump de lockfile:
- h2 0.4.13 → 0.4.19 (unbounded empty DATA frames)
- rustls 0.23.38 → 0.23.45 (TLS 1.3 cross-level handshake)
- rustls-webpki 0.103.12 → 0.103.15 (reachable panic en CRL parsing)
- crossbeam-epoch 0.9.18 → 0.9.21 (unsound non-Sync)
- chacha20 0.10.0 → 0.10.2
- inventory 0.1 → 0.3 (unsound downcast/NonSync; actualizado el Cargo.toml
  de cognicode-explorer, compila sin cambios de API)

### 2. Política de advisories (deny.toml)

Vulnerabilidades/unsound NUEVAS = error de release (gate bloqueante).
Deudas transitivas documentadas con motivo y camino de fix (ignores
individuales, no silencio global):

| RUSTSEC | Crate | Motivo / fix |
|---|---|---|
| 2024-0384 | instant (unmaintained) | vía notify 7; fix: notify 8 |
| 2026-0192 | ttf-parser (unmaintained) | vía mermaid-rs-renderer/fontdb; upstream |
| 2025-0141 | bincode 1.x (unmaintained) | transitive; workspace usa bincode 2 |
| 2023-0057 | libc unsound | vía build path; libc bump |
| 2024-0437 | protobuf 2.28 (VULNERABILITY) | vía prometheus 0.13/opentelemetry-prometheus 0.27; fix = migración OTel 0.27→0.28 (unidad propia). Exposición limitada al export /metrics |

`cargo deny check advisories` → **ok**.

### 3. SBOM (CycloneDX)

`cargo cyclonedx --format json` genera BOM por crate del workspace
(`crates/*.cdx.json`). Los BOMs se suben como artefactos de cada lane de
release (`payloads-<platform>`), junto a los tarballs con sus SHA256SUMS
ya existentes (smoke nativo del limpio HOME ya presente en release.yml).

### 4. Gates en release.yml

- `Advisories gate (cargo-deny)`: BLOQUEANTE antes de build.
- `SBOM (cargo-cyclonedx)`: genera BOMs; subidos con payloads.

## Verificación tras los bumps

- core lib: 2145 passed / 0 failed / 27 ignored (x2, estable).
- mcp continuation_e2e 5/5; cogh serial 304/304; lifecycle 7/7;
  prf_cli_01_uat 6/6. clippy workspace: 0 errores.

## Estado

PARTIAL (mejorado). Cubre advisories-gate bloqueante + SBOM + sha256 +
smoke nativo. Deuda declarada: scan de LICENCIAS no exigido como gate
(política de licencias pendiente de decisión); migración OTel 0.28 para
eliminar RUSTSEC-2024-0437 es unidad propia.
