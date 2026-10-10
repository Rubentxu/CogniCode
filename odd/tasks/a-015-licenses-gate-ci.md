# Feature: A-015 — `cargo deny check licenses` step en CI (PR-SEC remaining)

> **Goal:** Cerrar el outcome PR-SEC del programa production-ready stabilization. El gate `cargo deny check licenses` ya pasa localmente (`licenses ok`) pero NO corre en CI. Un cambio de dependencia que añada una licencia no permitida pasaría el `merge-gate` sin detección.

## Estado al inicio

- **HEAD:** `ef06bcb2678cc7198d47c3c61f309264d3f98767` (rama `docs/cp2-a012-closure`, 7 ahead de origin/main).
- **QW-05 (Actions SHA pinning):** cerrado. Todos los `uses:` en `.github/workflows/*.yml` pineados por SHA.
- **cargo-deny advisories:** `licenses ok` + `advisories ok` localmente. Step `Advisories gate (cargo-deny)` ya existe en `release-validate.yml:121`.
- **Falta:** Step `Licenses gate (cargo-deny)` en `release-validate.yml`. Sin él, un cambio de licencia en una dependencia no se detecta en CI.

## Disciplina: Strict TDD

1. **RED** — test contractual que ejecuta `cargo deny check licenses` y pinea el resultado.
2. **GREEN** — añadir step CI en `release-validate.yml`. El test pasa porque el gate ya está en `deny.toml`.
3. **TRIANGULATE** — añadir mutación que verifique la robustez del gate (e.g., `cargo deny check licenses` no debe pasar si la policy `[licenses]` se ignora accidentalmente).
4. **REFACTOR** — solo si las triangulaciones revelan duplicación o mejorable.

## Tasks

Ver `todo` projection.

## Out of scope

- Resolver `RUSTSEC-2024-0437` (protobuf advisory) — requiere migración OTel 0.27→0.28 (refactor + bump de versiones). Out of scope aquí.
- Reorganizar otros workflows (e.g., `ci.yml`, `pr-ci.yml`) — solo el `release-validate.yml` necesita el step.
- Promover el gate a un required check separado de `merge-gate` — decisión de governance separada.
- Modificar `deny.toml` — la policy ya está bien configurada.
- Cualquier bump de SemVer — `chore(ci)` no es feat/fix, v0.99.2 sin cambios.

## Riesgos

- El step CI añade ~30 segundos al workflow (instalación de cargo-deny + ejecución). Mitigación: reusar la instalación del step `Advisories gate` (mismo job, misma action step).
- Si `deny.toml` se desincroniza con `Cargo.lock`, el gate puede fallar. Mitigación: el test contractual pine el resultado directamente, sin depender de la action.

## Recibo (al cierre)

| Fecha | Estado | Evidencia |
|---|---|---|
| 2026-10-10 | **DONE en código** | `cargo test -p cognicode-cli --test a015_licenses_gate` → 3/3 verde. `cargo deny check licenses` corre en `merge-gate.pipeline.kts:219-220` Y `release-candidate.pipeline.kts:205-206` (Stage `licenses` en el bloque `supply-chain` de ambas pipelines). Test contractual `a015_licenses_gate` pinea el resultado. Wired en `merge-gate.pipeline.kts:cli-a015-licenses-gate` (stage que corre el test). Caveat: el goal doc original proponía añadir el step en `release-validate.yml`; ese workflow ya no existe — fue eliminado en el cutover GitHub Actions → PipelineK (commits `b651774a`, `d3426966`). El gate vive ahora en los `.pipeline.kts`. La policy `[licenses]` está en `deny.toml` per commit `f551311c` (cargo-deny allow-list añadido). Doc-debt cleanup. |