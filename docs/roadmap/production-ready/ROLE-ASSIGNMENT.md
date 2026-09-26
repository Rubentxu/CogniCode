# Recursos y asignación por rol

## Roles

| Rol | Responsabilidad principal | Acciones |
|---|---|---|
| Tech Lead / Architecture | decisiones de frontera, sequencing, aceptación de C8 y módulos profundos | QW-02, CR-01, CR-06, ST-01..05 |
| Senior Rust Engineer | implementación core, refactors, e91 | CR-03..07, ST-01..05 |
| DevEx / CI Engineer | workflows, clean-clone, adaptive testing, coverage | QW-03..06, CR-01, CR-08, CR-09 |
| Security / Supply-chain Engineer | advisories, provenance, pinning, OTel migration review | QW-05/06, CR-07 |
| QA / Performance Engineer | fixtures, UAT, scorecard, flake detection | CR-01..05, CR-09 |
| Release Operator | freeze de SHA, firma C8, tag/publicación | CR-01, publicación v0.99.0 |

## Prerequisitos técnicos

### Para C8-R
- capacidad de clonar el repositorio desde cero;
- toolchain Rust estable idéntico al CI;
- acceso de lectura a GitHub Actions y release artifacts;
- mecanismo para congelar un SHA sin working-tree residual.

### Para e91
- fixture multi-repo estable y versionado o materializable de forma determinista;
- medición repetible de wall-clock por etapa;
- evitar runners compartidos para un gate demasiado estricto; usar margen o percentile robusto.

### Para boundary hardening
- suite actual verde antes de introducir las nuevas rules;
- inventario inicial de violaciones existentes;
- decisión explícita sobre excepciones transitorias.

### Para refactors estratégicos
- tests caracterizadores alrededor de cada consumer real;
- prohibido introducir un port sin contrato probado por al menos un adapter o test fake útil;
- preferencia por migraciones verticales pequeñas en vez de reorganización horizontal masiva.

## Capacidad recomendada

Configuración óptima durante Fases 1–2:

- 1 Tech Lead al 30–40%;
- 1 Senior Rust al 100%;
- 1 DevEx/QA compartido al 50–70%;
- Security puntual en CR-07/QW-05.

Fase 3 se beneficia de dos Rust seniors en paralelo solo después de fijar CR-06; antes, paralelizar refactors aumenta el riesgo de crear interfaces incompatibles.
