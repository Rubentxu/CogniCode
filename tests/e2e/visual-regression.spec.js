// ====================================================================================================
// CogniCode Dashboard — Visual Regression Tests
//
// Tests que validan que el renderizado visual sea correcto.
// Estos tests generan golden images (screenshots) que se validan en CI.
//
// Ejecutar:
//   npx playwright test visual-regression.spec.js --config=tests/e2e/playwright.config.js
//
// Actualizar golden images (solo cuando sea necesario):
//   npx playwright test visual-regression.spec.js --update-snapshots --config=tests/e2e/playwright.config.js
// ====================================================================================================

const { test, expect } = require('@playwright/test');

// ──────────────────────────────────────────────────────────────────────────────
// 1. Layout & Shell — Validación visual principal
// ──────────────────────────────────────────────────────────────────────────────

test('VR-1a — Page shell renders sidebar and main content', async ({ page }) => {
  await page.goto('/', { waitUntil: 'networkidle', timeout: 10000 });
  await page.waitForTimeout(2000); // Esperar hidratación completa

  // Validar que el layout sea correcto
  const sidebar = page.locator('aside.sidebar-desktop');
  await expect(sidebar).toBeAttached();

  const mainContent = page.locator('.main-content');
  await expect(mainContent).toBeAttached();

  // Golden image del layout completo
  await expect(page).toHaveScreenshot('layout-shell.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 2. Navegación principal — Sidebar con todos los items
// ──────────────────────────────────────────────────────────────────────────────

test('VR-2a — Sidebar navigation with all items visible', async ({ page }) => {
  await page.goto('/', { waitUntil: 'networkidle', timeout: 10000 });
  await page.waitForTimeout(2000);

  // Validar que todos los items de navegación estén presentes
  const sidebar = page.locator('aside.sidebar-desktop');
  const items = ['Projects', 'Dashboard', 'Issues', 'Metrics', 'Quality Gate', 'Configuration'];

  for (const item of items) {
    await expect(sidebar.locator('a', { hasText: item })).toBeAttached();
  }

  // Golden image del sidebar completo
  await expect(page.locator('aside.sidebar-desktop')).toHaveScreenshot('sidebar-navigation.png', {
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 3. Pages principales — Estado visual de cada página
// ──────────────────────────────────────────────────────────────────────────────

test('VR-3a — Projects page renders correctly', async ({ page }) => {
  await page.goto('/projects', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  await expect(page.locator('h1')).toContainText('Projects');
  await expect(page.locator('button:has-text("+ Add Project")')).toBeVisible();

  // Golden image de la página Projects
  await expect(page).toHaveScreenshot('page-projects.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-3b — Issues page renders correctly', async ({ page }) => {
  await page.goto('/issues', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  await expect(page.locator('h1')).toContainText('Issues');
  await expect(page.locator('button:has-text("Apply")')).toBeAttached();

  // Golden image de la página Issues
  await expect(page).toHaveScreenshot('page-issues.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-3c — Metrics page renders correctly', async ({ page }) => {
  await page.goto('/metrics', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  await expect(page.locator('h1')).toContainText('Metrics');

  // Golden image de la página Metrics
  await expect(page).toHaveScreenshot('page-metrics.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-3d — Quality Gate page renders correctly', async ({ page }) => {
  await page.goto('/quality-gate', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  await expect(page.locator('h1')).toContainText('Quality Gate');
  await expect(page.locator('button:has-text("Edit Conditions")')).toBeAttached();

  // Golden image de la página Quality Gate
  await expect(page).toHaveScreenshot('page-quality-gate.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-3e — Configuration page renders correctly', async ({ page }) => {
  await page.goto('/configuration', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  await expect(page.locator('h1')).toContainText('Configuration');
  await expect(page.locator('input')).toBeAttached();

  // Golden image de la página Configuration
  await expect(page).toHaveScreenshot('page-configuration.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 4. Diagrams pages — Validación visual de nueva funcionalidad
// ──────────────────────────────────────────────────────────────────────────────

test('VR-4a — Diagrams page renders correctly', async ({ page }) => {
  await page.goto('/diagrams', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  await expect(page.locator('h1')).toContainText('Diagrams');
  await expect(page.locator('button', { hasText: /generate/i })).toBeAttached();

  // Golden image de la página Diagrams
  await expect(page).toHaveScreenshot('page-diagrams.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-4b — Diagram Diff page renders correctly', async ({ page }) => {
  await page.goto('/diagrams/diff', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  const body = await page.locator('body').textContent();
  expect(body).toMatch(/comparison|compare|diff|panel/i);

  // Golden image de la página Diagram Diff
  await expect(page).toHaveScreenshot('page-diagrams-diff.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 5. Responsive Design — Validación visual en diferentes viewports
// ──────────────────────────────────────────────────────────────────────────────

test('VR-5a — Mobile view (375px)', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await page.goto('/', { waitUntil: 'load', timeout: 10000 });
  await page.waitForTimeout(2000);

  // Golden image de mobile view
  await expect(page).toHaveScreenshot('viewport-mobile-375px.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-5b — Desktop view (1400px)', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/', { waitUntil: 'load', timeout: 10000 });
  await page.waitForTimeout(2000);

  // Golden image de desktop view
  await expect(page).toHaveScreenshot('viewport-desktop-1400px.png', {
    fullPage: true,
    animations: 'disabled',
  });
});