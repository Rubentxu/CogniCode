# ADR-TH-004 — Nextest como adaptador opcional, no autoridad de validez

- **Estado:** PROPOSED.
- **Decisión:** habilitar `cargo nextest` opcionalmente para tests Rust compatibles y usar `-R <id>` sólo con la grabación experimental activada y previa comprobación del `Fingerprint` de suites. Preferir ID explícito; no `latest` global cuando concurren agentes.
- **Fallback:** `cargo test` estándar, sin degradar cobertura de doctests, binarios ni pruebas cuyo orden/entorno no pueda expresarse en Nextest.
- **Riesgo:** la propia documentación de Nextest aclara que el rerun no detecta cambios de código ni build. Grabación es experimental; registrar versión exacta y capacidades detectadas.
- **Medida:** comparar cobertura por identificadores de caso y *wall-clock* con Cargo sobre los mismos inputs, no sólo `exit=0`.
- **Fuentes oficiales:** https://nexte.st/docs/features/record-replay-rerun/rerun/ ; https://nexte.st/docs/features/record-replay-rerun/ ; https://doc.rust-lang.org/cargo/commands/cargo-test.html
