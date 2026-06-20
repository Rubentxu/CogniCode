# E2E Visual Regression Tests — Dashboard

Tests que validan el renderizado visual del Dashboard usando golden images.

## Ejecución

### Validar contra golden images (CI y desarrollo)
```bash
cd tests/e2e
npx playwright test visual-regression.spec.js
```

### Actualizar golden images (solo local, cuando hay cambios intencionales)
```bash
cd tests/e2e
npx playwright test visual-regression.spec.js --update-snapshots
```

### CI Configuration

En CI, los tests se ejecutan **SIN** `--update-snapshots` para detectar regresiones.

## Directorio de Golden Images

Los golden images se guardan en:
```
tests/e2e/__snapshots__/
  ├── layout-shell.png
  ├── sidebar-navigation.png
  ├── page-projects.png
  ├── page-issues.png
  ├── page-metrics.png
  ├── page-quality-gate.png
  ├── page-configuration.png
  ├── page-diagrams.png
  ├── page-diagrams-diff.png
  ├── viewport-mobile-375px.png
  └── viewport-desktop-1400px.png
```

## Proceso de Actualización

Cuando un cambio intencional afecta el layout:

1. Ejecutar tests localmente: `npx playwright test visual-regression.spec.js`
2. Verificar que las diferencias sean esperadas
3. Actualizar goldens: `npx playwright test visual-regression.spec.js --update-snapshots`
4. Commit los nuevos golden images con el código
5. En CI, los tests pasarán con los nuevos goldens

## Opciones de Configuración

Los snapshots usan opciones consistentes:
- `fullPage: true` — Captura toda la página (no solo viewport)
- `animations: 'disabled'` — Deshabilita animaciones para capturas determinísticas
- `timeout: 10000` — Timeout de espera para estabilización

## Troubleshooting

### Tests fallen con diferencias visuales

1. **Diferencia esperada?** → Actualizar golden images con `--update-snapshots`
2. **Diferencia no esperada?** → Investigar regresión visual

### Capturas inconsistentes (flaky)

- Aumentar `page.waitForTimeout()` para permitir más estabilización
- Verificar que el servidor de prueba sea determinístico
- Usar `animations: 'disabled'` para evitar diferencias por timing de animaciones

## Referencias

- Playwright Visual Regression: https://playwright.dev/docs/test-snapshots
- Documentación completa: `apps/explorer-ui/e2e/VISUAL-REGRESSION.md`