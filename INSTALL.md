# Incorporación al repositorio

El ZIP está estructurado para descomprimirse directamente en la raíz de `Rubentxu/CogniCode`.

## Contenido nuevo

- `docs/roadmap/production-ready/**`
- `openspec/changes/2026-09-26-c8-recertification/**`
- `openspec/changes/2026-09-26-architecture-boundary-hardening/**`

No sobrescribe ficheros existentes de la auditoría base.

## Nota sobre `.gitignore`

La auditoría detectó que el repositorio ignora de forma amplia `docs/` y `openspec/*`. Por ello, después de descomprimir, el repositorio puede requerir `git add -f` o —preferentemente como primera acción QW-03— corregir las reglas de ignore para que estos directorios de gobernanza queden versionables de manera explícita. Esto es una condición conocida del repositorio, no una modificación necesaria del contenido del paquete.

## Orden de lectura

1. `docs/roadmap/production-ready/README.md`
2. `EXECUTIVE-SUMMARY.md`
3. `PRIORITIZATION-MATRIX.md`
4. `EXECUTION-PLAN.md`
5. `DEPENDENCY-MAP.md`
6. `phases/PHASE-1-QUICK-WINS.md`
