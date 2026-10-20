# Incorporación al repositorio

El ZIP está estructurado para descomprimirse directamente en la raíz de `Rubentxu/CogniCode`.

## Contenido nuevo

- `docs/roadmap/production-ready/**`

No sobrescribe ficheros existentes de la auditoría base.

## Nota sobre `.gitignore`

La auditoría detectó que el repositorio ignora de forma amplia `docs/`. Por ello, después de descomprimir, el repositorio puede requerir `git add -f` o —preferentemente como primera acción QW-03— corregir las reglas de ignore para que el directorio de gobernanza quede versionable de manera explícita. Esto es una condición conocida del repositorio, no una modificación necesaria del contenido del paquete.

## Orden de lectura

1. `docs/roadmap/production-ready/README.md`
2. `EXECUTIVE-SUMMARY.md`
3. `PRIORITIZATION-MATRIX.md`
4. `EXECUTION-PLAN.md`
5. `DEPENDENCY-MAP.md`
6. `phases/PHASE-1-QUICK-WINS.md`
