# RELEASE-CANDIDATE — Programa PRF

> Estado: DRAFT — decisión formal pendiente del operador (push/tag).

## Candidato

| Campo | Valor |
|---|---|
| SHA candidato | `f49aff2d258b3647fc0225c42050fb7cff91c9ef` (el doc vive en este mismo commit; SHA leido al cerrar sesion: ver `git rev-parse HEAD`) (incluye `0764fb81` H-F6-1 y fix docs_extractor `67375e6c`) |
| Versión | 0.97.3 (el fix de HEAD aún no está publicado; el release tag v0.97.3 NO contiene estos fixes) |
| Plataformas probadas | Linux x86_64 (única plataforma con UAT ejecutada) |

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

- `cognicode-core --lib` (con `multimodal`): 2122 passed, 0 failed, 27 ignored.
- Workspace `--lib`: GREEN salvo `moldql::cursor::consume_keyword_panics_on_mismatch`
  (cognicode-explorer), PRE-EXISTENTE en HEAD limpio (verificado con
  stash), NO regresión del programa PRF.
- `cognicode-cli` bin cogh: 293 passed, 0 failed.

## Frentes abiertos (deuda)

| ID | Descripción | Estado |
|---|---|---|
| H-F3-1 | find_usages MCP con walk+parser inline | OPEN (LOW, no bloqueante) |
| H-F6-1 | doble resolución de home | RESUELTO (`0764fb81`) |
| moldql panic test | test de pánico inestable en explorer | PRE-EXISTENTE, fuera de alcance PRF |
| 6 fallos preexistentes | cogh_uninstall, manifest_upsert ladybug, rate-limit H10 | catalogados, no regresiones |

## Decisión formal

- [x] Evidencias reunidas y verificadas en HEAD.
- [ ] **READY FOR RELEASE / HOLD** — decisión del operador.
- La publicación efectiva (push, tag, distribución) requiere
  autorización explícita del operador y NO está cubierta por la
  preautorización de continuidad del programa.
