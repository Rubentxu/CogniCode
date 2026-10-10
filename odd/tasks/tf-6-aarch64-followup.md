# Goal — TF-6 aarch64-linux-gnu build + publish

> **Estado**: PENDING (FUERA DE SCOPE para v0.101.10). Autorizado en el
> goal `v1015-consolidation-and-promote` como TF-6, documentado al
> scope-ear el `release-candidate.pipeline.kts` a x86_64-only.

## 0. Cómo se relaciona con el resto

| Artefacto | Rol |
|---|---|
| `odd/tasks/v1015-consolidation-and-promote.md` §TF-6 | origen |
| `release-candidate.pipeline.kts` líneas 70-86 | el `targets` list se scopeó a `["x86_64-unknown-linux-gnu"]` con comentario que apunta a este goal |
| `install.sh` líneas 36-44 | declara soporte para `x86_64, aarch64` (Tier 1); un host aarch64 hoy recibe `fail "no checksum found"` |
| `docs/distribution/INSTALL.md` | el doc de distribución que anuncia los 2 targets Tier 1 |
| TF-5e (recibo en v1015) | v0.101.10 publicado solo x86_64; TF-6 cierra el gap |

## 1. Objetivo

Re-habilitar el matrix Tier-1 (x86_64 + aarch64) en
`release-candidate.pipeline.kts`, producir los assets aarch64 de
v0.101.x, y publicarlos adjuntos a la release (o a una nueva release si
se bump-ea a v0.101.11). `install.sh` en un host aarch64 debe pasar
todos los 7 pasos con `checksum verified`.

## 2. Problema

El v0.101.10 se publicó solo para x86_64 porque el host de release
(Bazzite Kinoite x86_64) no tiene el cross-toolchain aarch64 y Bazzite
es inmutable (sin sudo passwordless para `dnf install` o
`rpm-ostree install`). El pipeline asume ambos targets Tier 1
(`val targets = listOf("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu")`),
así que sin toolchain el stage `toolchain-for-aarch64` falla
inmediatamente.

## 3. Por qué ahora (o por qué no, exactamente)

`install.sh` declara `Tier-1 channels support Linux only (x86_64, aarch64)`,
así que el contrato público dice "instalable en aarch64". Un host aarch64
que pinne `v0.101.10` recibe el SHA256SUMS, busca
`cogh-0.101.10-aarch64-unknown-linux-gnu.tar.gz`, no lo encuentra, y
falla cerrado (`"no checksum found for $asset in SHA256SUMS (fail closed)"`).

Esto no bloquea a nadie hoy (no hay usuarios aarch64 reportados en este
proyecto), pero **miente** sobre el contrato. Hasta que se cierre TF-6,
el README y el install.sh dicen una cosa y la realidad es otra.

## 4. Scope

### 4.1 In-scope

* Cross-toolchain aarch64-linux-gnu disponible en el entorno de release
  build. Opciones:
  - **(preferida)** un CI runner con el toolchain pre-instalado.
  - **(alternativa)** `distrobox create --image fedora:44` con
    `gcc-aarch64-linux-gnu` instalado, build desde dentro, copiar
    artefactos al `release/` del repo host.
  - **(alternativa)** `rpm-ostree install gcc-aarch64-linux-gnu` con
    sudo + reboot en un host Bazzite (no viable en el host actual sin
    sudo).
* Restaurar `val targets = listOf("x86_64-unknown-linux-gnu",
  "aarch64-unknown-linux-gnu")` en `release-candidate.pipeline.kts`.
* Build del target aarch64 vía el cross-toolchain; verificar
  `bin/cogh --version` en el artefacto aarch64 (con `qemu-aarch64` o
  el script `run-target-binary.sh` que el candidate pipeline ya usa).
* Producir los tarballs aarch64: `cogh-0.101.x-aarch64-unknown-linux-gnu.tar.gz`
  + idem `cognicode` + `cognicode-mcp`.
* Re-generar `SHA256SUMS` con las 4 entradas extra (3 binarios + 0 skill
  bundles nuevos; los skill bundles son arch-independent).
* Adjuntar a la release v0.101.10 (o nueva release si la versión bumpea).
* Re-verificar con `install.sh` desde un host aarch64 o con
  `qemu-aarch64` emulando el `bin/cogh --version` check.

### 4.2 Out-of-scope

* Builds para Windows / macOS. Esos son Wave B/C del programa
  PRODUCT-1.0 CP4 Distribution, no TF-6.
* Tier 2 platforms. TF-6 es solo el Tier 1 aarch64.
* Optimizar el tiempo de cross-compile. Aceptar lo que el toolchain da.

## 5. Constraints

* **Sin breaking change**: el output, wire format, y APIs son los mismos.
* `install.sh` debe pasar end-to-end en un host aarch64 con la versión
  pineada.
* `cognicode-release verify` R1-R9 verde con `--platform aarch64-unknown-linux-gnu`
  además de x86_64.
* TDD: si el `SHA256SUMS` no contiene las 4 entradas nuevas, el verify
  falla. Eso es el RED.
* Sin reintroducir el `targets = listOf(...)` con un solo elemento. TF-6
  cierra el scope, no lo perpetúa.

## 6. Work items

| ID | Tarea | Verificación |
|----|-------|--------------|
| **TF-6.1** | Decidir el entorno del cross-toolchain (CI runner / distrobox / rpm-ostree) y dejarlo operacional | `rustup target list --installed` muestra `aarch64-unknown-linux-gnu`; `command -v aarch64-linux-gnu-gcc` resuelve |
| **TF-6.2** | Restaurar `val targets = listOf("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu")` en `release-candidate.pipeline.kts` y quitar el comentario de scope | diff en el .kts, sin otro cambio |
| **TF-6.3** | Build aarch64 vía el candidate pipeline (o manualmente si el pipeline sigue bloqueado por infra) | `target/aarch64-unknown-linux-gnu/release/cogh` existe y `./cogh --version` (vía qemu) → `cogh 0.101.x` |
| **TF-6.4** | Empaquetar los 3 tarballs aarch64, regenerar `SHA256SUMS` y manifests con `--platform linux-x86-64,linux-aarch64` | `cognicode-release verify --platform linux-x86-64,linux-aarch64` R1-R9 verde |
| **TF-6.5** | Adjuntar a la release v0.101.10 (o bump v0.101.11 si aplica); re-verificar con `install.sh` en host aarch64 o qemu | `install.sh COGNICODE_VERSION=v0.101.x` end-to-end verde |

## 7. Acceptance criteria

1. TF-6.1..5 en DONE.
2. `release-candidate.pipeline.kts` tiene el matrix Tier 1 completo
   (2 targets).
3. `gh release view v0.101.10 --json assets` (o el tag que sea) lista
   los 3 tarballs aarch64 + los 3 x86_64.
4. `cognicode-release verify --platform linux-x86-64,linux-aarch64`
   verde en R1-R9.
5. `install.sh` end-to-end verde en host aarch64 o vía `qemu-aarch64`
   para el `cogh --version` check.
6. Update a `odd/tasks/v1015-consolidation-and-promote.md` TF-6 row.

## 8. Risks

* **R-1**: la decisión del entorno (CI vs distrobox vs rpm-ostree) es
  operator-gated. Si la decisión tarda, TF-6 queda bloqueado.
* **R-2**: el cross-compile puede fallar por toolchain incompleto (faltan
  headers glibc-aarch64, etc.). Mitigación: usar una imagen Fedora
  completa (distrobox fedora:44 o runner) que ya tiene el toolchain bien.
* **R-3**: el verify con R9 "platform set complete" puede fallar si
  solo se publica aarch64 sin x86_64. Mitigación: publicar ambos en la
  misma release; no partir en releases separadas para mantener R1.

## 9. Recibo (append-only)

| Fecha | WU | Commit | Notas |
|-------|----|--------|-------|
| 2026-10-10 | TF-6.0 | `8c6d0f7` | **El operador autoriza CI con pipelinek 0.48.0-rc + cross toolchain `gcc-aarch64-linux-gnu` hasta que 0.48.0 esté disponible**. Decisión registrada; pendiente la provisión del CI runner. |
| 2026-10-10 | TF-6.0b | (este commit) | **Aclaración del operador: aarch64 queda FUERA de esta release**. La restauración del matrix Tier 1 en el .kts (commit `9cf095f`, revertido ahora) era prematura: aarch64 entra cuando la release venga, no en v0.101.10 ni en la próxima release, hasta que el CI runner esté provisionado Y probado end-to-end con un v0.101.11 no-op primero. El `release-candidate.pipeline.kts` se queda con `val targets = listOf("x86_64-unknown-linux-gnu")` para esta release. TF-6.1..5 siguen pendientes; el trigger para reabrirlos es "CI runner provisionado + v0.101.11 publicado como smoke test sin aarch64". |
| 2026-10-10 | TF-6.0c | (este commit) | intento de TF-6.1 (`9cf095f`) restaurando `val targets` matrix Tier 1; **revertido** tras la aclaración del operador en el mismo turno. El commit queda en el historial como evidencia del intento, pero la HEAD activa vuelve a x86_64-only. Lección: la decisión "CI con 0.48.0-rc" es un enabling, no una autorización para re-habilitar aarch64 en la release actual. |
| _pendiente_ | TF-6.1 | _pendiente_ | restaurar `val targets = listOf("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu")` en `release-candidate.pipeline.kts`. **Trigger: CI runner provisionado + v0.101.11 publicado como smoke test sin aarch64**. No antes. |
| _pendiente_ | TF-6.2..5 | _pendiente_ | build aarch64 en CI + package + verify + publish. Depende de TF-6.1. |
