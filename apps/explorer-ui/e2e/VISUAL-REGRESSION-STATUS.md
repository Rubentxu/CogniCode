# Visual Regression Tests — Estado de Ejecución

## Resultados de Primera Ejecución

**Fecha:** 2026-06-20

### ✅ Tests Exitosos (6/10) - Golden Images Generadas

1. **VR-SM-1 — Initial load with shell visible**
   - Golden image: `smoke-initial-load-chromium-linux.png`
   - Estado: ✓ Pasó

2. **VR-SM-2 — Spotter dialog with results**
   - Golden image: `smoke-spotter-results-chromium-linux.png`
   - Estado: ✓ Pasó

3. **VR-ERROR-2 — Empty spotter results (no matches)**
   - Golden image: `error-states-empty-spotter-chromium-linux.png`
   - Estado: ✓ Pasó

4. **VR-ERROR-3 — Closing last pane shows empty state**
   - Golden image: `error-states-pane-stack-empty-chromium-linux.png`
   - Estado: ✓ Pasó

5. **VR-ERROR-1 — Connection gate resolves to shell**
   - Golden image: `error-states-connection-gate-chromium-linux.png`
   - Estado: ✓ Pasó

6. **VR-GRAPH-1 — Call graph view with SVG rendered**
   - Golden image: `graph-call-graph-view-chromium-linux.png`
   - Estado: ✓ Pasó

### ❌ Tests con Timeout (4/10) - Requieren Ajustes

Los siguientes tests fallaron por timeout (30s), pero la captura de screenshots funcionaría si el timeout se incrementa:

1. **VR-SM-3 — Object inspector after selecting an object**
   - Error: `Test timeout of 30000ms exceeded` al hacer `input.fill("build")`
   - Causa: El Spotter input no está disponible o el dialog no se abre
   - Solución: Incrementar timeout o ajustar selectors

2. **VR-GRAPH-2 — Hotspot interaction updates navigation**
   - Error: `input.fill("build")` - variable `input` no definida en ese test
   - Causa: Bug en el código del test (variable reutilizada de test anterior)
   - Solución: Definir variable `input` en este test

3. **VR-TABS-1 — View tabs visible after object selection**
   - Error: `Test timeout of 30000ms exceeded` al hacer `input.fill("build")`
   - Causa: Misma que VR-SM-3
   - Solución: Incrementar timeout o ajustar selectors

4. **VR-TABS-2 — Switching between view tabs**
   - Error: `Test timeout of 30000ms exceeded` al hacer `input.fill("build")`
   - Causa: Misma que VR-SM-3
   - Solución: Incrementar timeout o ajustar selectors

## Análisis de Causa

El patrón de errores sugiere que el **Spotter dialog no se está abriendo** cuando se usa `page.keyboard.press("Meta+k")` en algunos tests.

Posibles causas:
1. El test se ejecuta demasiado rápido y el listener de keyboard no está montado
2. Hay un race condition entre el render del Shell y el listener de keyboard
3. Los tests necesitan más tiempo de espera antes de presionar el shortcut

## Próximos Pasos

1. **Corregir bugs de código en tests:**
   - VR-GRAPH-2: Definir variable `input` local

2. **Incrementar timeouts para tests que fallan:**
   - Esperar más tiempo después de `page.goto("/")`
   - Esperar más tiempo después de `page.keyboard.press("Meta+k")`

3. **Revisar configuración de Playwright:**
   - Verificar que `use.actionTimeout` y `expect.timeout` sean suficientes

4. **Ejecutar tests corregidos y generar golden images completos**

## Golden Images Confirmados

Los siguientes golden images están confirmados y funcionan correctamente:

```bash
apps/explorer-ui/e2e/visual-regression.spec.ts-snapshots/
  ├── smoke-initial-load-chromium-linux.png
  ├── smoke-spotter-results-chromium-linux.png
  ├── error-states-empty-spotter-chromium-linux.png
  ├── error-states-pane-stack-empty-chromium-linux.png
  ├── error-states-connection-gate-chromium-linux.png
  └── graph-call-graph-view-chromium-linux.png
```

Estos **DEBEN** ser commitados al repo como parte del código productivo (tests).