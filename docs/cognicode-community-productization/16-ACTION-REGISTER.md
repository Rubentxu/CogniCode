# Action Register — PRODUCT-1.0

| ID | Acción | Prioridad | Dependencia | Resultado / Acceptance |
|---|---|---:|---|---|
| A-001 | Crear track `community-productization` y puntero en ROADMAP | P0 | — | una sola autoridad de agenda |
| A-002 | Reconciliar versión release/main/README/repo metadata | P0 | A-001 | cero claims contradictorias |
| A-003 | Product manifest generado | P0 | A-002 | **CLOSED 2026-09-27** — schema `cognicode.product/v1`, `source_commit`, generator determinista, tests 5/5, ciclo `p-c1fac1fea05615c6/cp0-product-manifest`, commit `96d06854`; siguientes A-004/A-005 |
| A-004 | Tool/catalog generator | P0 | A-003 | **CLOSED 2026-09-27** — `cognicode.tools/v1`, captura runtime `tools/list` de 73 tools, generator determinista, schema, tests 4/4, ciclo `p-c1fac1fea05615c6/cp0-tool-catalog`, commit `85222f67`; A-005 queda siguiente |
| A-005 | Language/platform support matrix | P0 | A-003 | **CLOSED 2026-09-27** — `product/languages.json` + `product/platforms.json`, schema `languages.v1`, 30 lenguajes (18 supported / 12 experimental) y 3 plataformas; clasificaciones derivadas de la cobertura de aceptación real (m06 + m10) y de los lanes de release/CI, no declaradas; commit `73235889` |
| A-006 | Definir profiles core/reviewer/developer/experimental | P0 | A-004 | **CLOSED 2026-09-27** — `product/profiles.json` (`cognicode.profiles/v1`) pasa a ser la única fuente pública, reemplazando la constante `PUBLIC_PROFILES` hard-coded del manifest; profiles derivados de `release_contract.rs` + 2 declarados a mano (developer, experimental, `install:false`); commit `f24609f0` |
| A-007 | Resolver licencia repo y añadir ficheros | P0 | decisión operador | **CLOSED 2026-09-27** — decisión de licencia ya tomada en M0.9 (`f0708d4b`); CP1 no la re-decidió, la publicó: `LICENSE-MIT` + `LICENSE-APACHE` completos, `LICENSE` raíz, license-of-record en `[workspace.package]`, ADR-053; commit `95595af9` |
| A-008 | SECURITY/CONTRIBUTING/SUPPORT/CoC/templates | P0 | A-007 parcial | **CLOSED 2026-09-27** — `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `SUPPORT.md`, 3 issue forms y PR template, escritos desde los contratos reales; READMEs EN/ES reconciliados a cifras canónicas (73 tools, 30 lenguajes) citando los artefactos de origen; suite verificada por mutación (3 contratos rotos deliberadamente, 3 tests fallaron); commit `95595af9`. **Gate OSS incompleto**: CP1.7 Discussions sigue `operator-gated` (CP1-DEBT-02) |
| A-009 | Audit authority + agent-safe profile | P0 | A-006 | reviewer sin write/exec/network |
| A-010 | MCP conformance baseline | P0 | A-009 | revisión MCP declarada con PASS |
| A-011 | Schema snapshots stable profile | P0 | A-006 | drift incompatible fail |
| A-012 | Structured output stable reviewer tools | P1 | A-011 | no JSON-string parsing en tools objetivo |
| A-013 | Black-box lifecycle UAT | P0 | A-009 | artifacts: startup→shutdown PASS |
| A-014 | `cognicode capabilities --json` | P1 | A-003 | capability discovery machine-readable |
| A-015 | Simplificar onboarding `cogh setup` | P1 | A-014 | install→doctor→MCP en happy path |
| A-016 | Crear `Rubentxu/cognicode-site` | P0 | — | repo + baseline Astro/Starlight |
| A-017 | Landing hero/IA + 5-minute quickstart | P0 | A-002,A-016 | usuario llega a first useful call |
| A-018 | Generated reference ingestion | P0 | A-003,A-016 | site falla si manifest/schema inválido |
| A-019 | Cloudflare Pages project | P0 | A-016 | preview deployment funcional |
| A-020 | Asociar `cognicode.rubentxu.dev` | P0 | A-019 | HTTPS healthy |
| A-021 | CNAME Squarespace manual | P0 | A-020 | resolución estable documentada |
| A-022 | PipelineK site checks/deploy | P1 | A-019 | production deploy + receipt + rollback |
| A-023 | mise channel | P0 | release assets coherentes | fresh install PASS |
| A-024 | MCPB | P0 | A-009,A-013 | bundle instalado sin toolchain Rust |
| A-025 | MCP Registry | P0 | A-024 | discoverable entry publicada |
| A-026 | OCI/GHCR | P1 | A-013 | non-root/read-only default smoke |
| A-027 | macOS arm64 build/certification | P0 GA | release matrix | clean-host PASS |
| A-028 | Windows x64 build/certification | P0 GA | release matrix | clean-host PASS o explicitly deferred |
| A-029 | Aqua | P2 | stable release contract | install PASS |
| A-030 | Homebrew tap | P2 | A-027 | formula install/upgrade PASS |
| A-031 | Nix flake | P2 | stable release | nix profile smoke |
| A-032 | asdf plugin | P3 | adoption signal | compatibility channel |
| A-033 | skill `cognicode` | P0 | A-014 | eval suite PASS |
| A-034 | skill `cognicode-agent-hardness` | P0 | A-009,A-014 | eval suite PASS |
| A-035 | skill `cognicode-pr-review` | P1 | A-014 | eval suite PASS |
| A-036 | align `cognicode-quality-investigator` | P0 | A-014 | no stale tool assumptions |
| A-037 | skills.sh pack | P1 | A-033..036 | install pack smoke |
| A-038 | reproducible with/without benchmark | P1 | A-013 | methodology+raw evidence publicable |
| A-039 | 60–90 s demo | P1 | A-017,A-038 | landing-ready asset |
| A-040 | launch content drafts | P1 | Product Truth gate | READY_FOR_PUBLICATION |
| A-041 | Public Beta 0.99.x | P0 | PT+OSS+HARD+DOC+DIST core | external beta open |
| A-042 | External beta feedback round | P0 | A-041 | ≥10 diverse testers or evidence equivalent |
| A-043 | 1.0 admission | P0 | A-042 | GA checklist 100% |
| A-044 | Coordinated 1.0 launch | P0 | A-043 + operator | site/registry/skills/blog/social synced |
| A-045 | Optional DNS authority migration to Cloudflare | P3 | separate approval | IaC DNS, no launch dependency |

## Large-block execution

Para evitar micromanagement, agrupar en bloques:

1. **BLOCK-A Truth & OSS**: A-001..011.
2. **BLOCK-B Product DX**: A-012..018.
3. **BLOCK-C Site & Domain**: A-019..022.
4. **BLOCK-D Distribution**: A-023..032.
5. **BLOCK-E Agent Adoption**: A-033..040.
6. **BLOCK-F Beta → GA**: A-041..044.
7. **BLOCK-G DNS IaC**: A-045 opcional.
