# E2E Visual Regression Tests — Explorer UI

Tests que validan el renderizado visual del Explorer UI usando golden images.

## Ejecución

### Validar contra golden images (CI y desarrollo)
```bash
cd apps/explorer-ui
npm run test:e2e:visual
# o
npx playwright test e2e/visual-regression.spec.ts
```

### Actualizar golden images (solo local, cuando hay cambios intencionales)
```bash
npx playwright test e2e/visual-regression.spec.ts --update-snapshots
```

### CI Configuration

En CI, los tests se ejecutan **SIN** `--update-snapshots` para detectar regresiones.

## Directorio de Golden Images

Los golden images se guardan en:
```
apps/explorer-ui/e2e/__snapshots__/
  ├── smoke-initial-load.png
  ├── smoke-spotter-results.png
  ├── smoke-object-inspector.png
  ├── graph-call-graph-view.png
  ├── graph-hotspot-click.png
  ├── error-states-connection-gate.png
  └── ...
```

## Proceso de Actualización

Cuando un cambio intencional afecta el layout:

1. Ejecutar tests localmente: `npm run test:e2e:visual`
2. Verificar que las diferencias sean esperadas
3. Actualizar goldens: `npm run test:e2e:visual -- --update-snapshots`
4. Commit los nuevos golden images con el código
5. En CI, los tests pasarán con los nuevos goldens

## Opciones de Configuración

Los snapshots usan opciones consistentes:
- `fullPage: true` — Captura toda la página (no solo viewport)
- `animations: 'disabled'` — Deshabilita animaciones para capturas determinísticas
- `timeout: 10000` — Timeout de espera para estabilización

## Troubleshooting

### Tests fallen con diferencias visuales

1. **Diferencia esperada?** → Actualizar golden images
2. **Diferencia no esperada?** → Investigar regresión visual

### Capturas inconsistentes (flaky)

- Aumentar `page.waitForTimeout()` para permitir más estabilización
- Verificar que MSW fixtures sean determinísticos
- Usar `animations: 'disabled'` para evitar diferencias por timing de animaciones