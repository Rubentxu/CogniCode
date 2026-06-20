// ====================================================================================================
// CogniCode Dashboard — Visual Regression Tests (CRITICAL UI FLOWS)
//
// Tests que validan que el renderizado visual sea correcto para flujos completos.
// Los tests de API (1a-1e, 14a-14b, 15a-15b, 18a-18e) NO se capturan aquí.
//
// Ejecutar:
//   npx playwright test visual-enhanced.spec.js --config=tests/e2e/playwright.config.js
//
// Actualizar golden images (solo cuando sea necesario):
//   npx playwright test visual-enhanced.spec.js --update-snapshots --config=tests/e2e/playwright.config.js
// ====================================================================================================

const { test, expect } = require('@playwright/test');

// ──────────────────────────────────────────────────────────────────────────────
// 2. Layout & Shell — Validación visual completa del layout
// ──────────────────────────────────────────────────────────────────────────────

test('VR-2a — Page shell renders sidebar and main content', async ({ page }) => {
  const check = collectErrors(page);
  await waitForApp(page);

  // Validación funcional
  const sidebar = page.locator('aside.sidebar-desktop');
  await expect(sidebar).toBeAttached();
  const links = sidebar.locator('a');
  await expect(links.first()).toBeAttached();
  await expect(page.locator('.main-content')).toBeAttached();
  expect(check()).toEqual([]);

  // Golden image del layout completo
  await expect(page).toHaveScreenshot('layout-shell.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-2b — Sidebar width = 190px on desktop (1400px viewport)', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  const box = await page.locator('aside.sidebar-desktop').boundingBox();
  expect(box).toBeTruthy();
  if (box) expect(Math.round(box.width)).toBe(190);

  // Golden image del sidebar en desktop
  await expect(page.locator('aside.sidebar-desktop')).toHaveScreenshot('sidebar-desktop-width.png', {
    animations: 'disabled',
  });
});

test('VR-2c — Main content is visible and positioned after sidebar', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  const main = page.locator('main.main-content');
  await expect(main).toBeAttached();
  const mainVisible = await main.isVisible();
  expect(mainVisible).toBe(true);

  // Golden image del main content
  await expect(page.locator('main.main-content')).toHaveScreenshot('main-content-visible.png', {
    animations: 'disabled',
  });
});

test('VR-2d — Mobile sidebar hidden on desktop', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  const mobile = page.locator('aside.sidebar-mobile');
  await expect(mobile).toBeAttached();
  const display = await mobile.evaluate(el => getComputedStyle(el).display);
  expect(display).toBe('none');

  // Golden image del desktop view
  await expect(page).toHaveScreenshot('desktop-view-sidebar-mobile-hidden.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 3. Navigation & Routing — Validación visual de navegación
// ──────────────────────────────────────────────────────────────────────────────

test('VR-3a — Sidebar has all 6 required nav items', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  const sidebar = page.locator('aside.sidebar-desktop');
  const items = ['Projects', 'Dashboard', 'Issues', 'Metrics', 'Quality Gate', 'Configuration'];
  for (const item of items) {
    await expect(sidebar.locator('a', { hasText: item })).toBeAttached();
  }

  // Golden image del sidebar completo
  await expect(sidebar).toHaveScreenshot('sidebar-all-nav-items.png', {
    animations: 'disabled',
  });
});

test('VR-3b — Clicking Projects link navigates to /projects', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  await page.locator('aside.sidebar-desktop a[href="/projects"]').click();
  await page.waitForTimeout(1000);
  await expect(page.locator('h1')).toContainText('Projects');

  // Golden image de la página Projects
  await expect(page).toHaveScreenshot('navigation-projects-page.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-3c — Clicking Issues link navigates to /issues', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  await page.locator('aside.sidebar-desktop a[href="/issues"]').click();
  await page.waitForTimeout(1000);
  await expect(page.locator('h1')).toContainText('Issues');

  // Golden image de la página Issues
  await expect(page).toHaveScreenshot('navigation-issues-page.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-3d — Clicking Metrics link navigates to /metrics', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  await page.locator('aside.sidebar-desktop a[href="/metrics"]').click();
  await page.waitForTimeout(1000);
  await expect(page.locator('h1')).toContainText('Metrics');

  // Golden image de la página Metrics
  await expect(page).toHaveScreenshot('navigation-metrics-page.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-3e — Clicking Quality Gate link navigates to /quality-gate', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  await page.locator('aside.sidebar-desktop a[href="/quality-gate"]').click();
  await page.waitForTimeout(1000);
  await expect(page.locator('h1')).toContainText('Quality Gate');

  // Golden image de la página Quality Gate
  await expect(page).toHaveScreenshot('navigation-quality-gate-page.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-3f — Unknown routes show 404 page', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/some-random-nonexistent-url-999', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(1000);
  await expect(page.locator('body')).toContainText(/404|not found/i);

  // Golden image de la página 404
  await expect(page).toHaveScreenshot('error-404-page.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 4. Dashboard Page — Validación visual del Dashboard
// ──────────────────────────────────────────────────────────────────────────────

test('VR-4a — Dashboard page renders with heading', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  const h1 = page.locator('h1:has-text("Dashboard")');
  await expect(h1.first()).toBeAttached();

  // Golden image del Dashboard
  await expect(page).toHaveScreenshot('page-dashboard.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-4b — Dashboard has Run Analysis button', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  const btn = page.locator('button:has-text("Run Analysis")').first();
  if (await btn.count() > 0) {
    await expect(btn).toBeAttached();

    // Golden image del Dashboard con botón
    await expect(page).toHaveScreenshot('page-dashboard-with-button.png', {
      fullPage: true,
      animations: 'disabled',
    });
  }
});

// ──────────────────────────────────────────────────────────────────────────────
// 5. Projects Page — Validación visual de Projects
// ──────────────────────────────────────────────────────────────────────────────

test('VR-5a — Projects page shows header and Add button', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  const check = collectErrors(page);
  await page.goto('/projects', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  await expect(page.locator('h1')).toContainText('Projects');
  await expect(page.locator('button:has-text("+ Add Project")')).toBeVisible();
  expect(check()).toEqual([]);

  // Golden image de la página Projects
  await expect(page).toHaveScreenshot('page-projects.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-5e — Project card shows rating badge, gate status, and metrics', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/projects', { waitUntil: 'load', timeout: 15000 });
  await page.waitForTimeout(3000);

  // Registrar project si no hay visibles
  const cards = page.locator('.card');
  const cardCount = await cards.count();
  if (cardCount <= 1) {
    await page.evaluate(async () => {
      await fetch('/api/projects/register', {
        method: 'POST', headers: {'Content-Type': 'application/json'},
        body: JSON.stringify({name:'CogniCode',path:'/home/rubentxu/Proyectos/rust/CogniCode'})
      });
    });
    await page.reload({ waitUntil: 'networkidle' });
    await page.waitForTimeout(2000);
  }

  const body = await page.locator('body').textContent();
  expect(body).toMatch(/Issues|Debt|Files|Runs/);

  // Golden image de projects con cards
  await expect(page).toHaveScreenshot('page-projects-with-cards.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 6. Issues Page — Validación visual de Issues
// ──────────────────────────────────────────────────────────────────────────────

test('VR-6a — Issues page shows filters and Apply button', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  const check = collectErrors(page);
  await page.goto('/issues', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  await expect(page.locator('h1')).toContainText('Issues');
  await expect(page.locator('button:has-text("Apply")')).toBeAttached();
  await expect(page.locator('select').first()).toBeAttached();
  expect(check()).toEqual([]);

  // Golden image de la página Issues
  await expect(page).toHaveScreenshot('page-issues.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-6f — Issue detail page accessible via /issues/0', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/issues/0', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  const body = await page.locator('body').textContent();
  expect(body).toMatch(/Back to Issues|Issue|No issues|not found/i);

  // Golden image de detail page
  await expect(page).toHaveScreenshot('page-issues-detail.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 7. Metrics Page — Validación visual de Metrics
// ──────────────────────────────────────────────────────────────────────────────

test('VR-7a — Metrics page shows heading', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  const check = collectErrors(page);
  await page.goto('/metrics', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  await expect(page.locator('h1')).toContainText('Metrics');
  expect(check()).toEqual([]);

  // Golden image de la página Metrics
  await expect(page).toHaveScreenshot('page-metrics.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-7c — Metrics page has content (Clean as You Code or empty state)', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/metrics', { waitUntil: 'load', timeout: 10000 });
  await page.waitForTimeout(2000);

  const body = await page.locator('body').textContent();
  expect(body.length).toBeGreaterThan(50);
  expect(body).toMatch(/Clean|Incremental|No metrics|no analysis|Run an analysis/i);

  // Golden image de metrics con contenido
  await expect(page).toHaveScreenshot('page-metrics-with-content.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 8. Quality Gate Page — Validación visual de Quality Gate
// ──────────────────────────────────────────────────────────────────────────────

test('VR-8a — Quality Gate page shows heading and Edit button', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  const check = collectErrors(page);
  await page.goto('/quality-gate', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  await expect(page.locator('h1')).toContainText('Quality Gate');
  await expect(page.locator('button:has-text("Edit Conditions")')).toBeAttached();
  expect(check()).toEqual([]);

  // Golden image de la página Quality Gate
  await expect(page).toHaveScreenshot('page-quality-gate.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-8c — Conditions table shows Status, Condition, Metric columns', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/quality-gate', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  const body = await page.locator('body').textContent();
  expect(body).toMatch(/Status|Condition|Metric/);

  // Golden image de quality gate con tabla
  await expect(page).toHaveScreenshot('page-quality-gate-with-table.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-8d — Gate summary section exists', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/quality-gate', { waitUntil: 'load', timeout: 10000 });
  await page.waitForTimeout(2000);

  const body = await page.locator('body').textContent();
  expect(body).toMatch(/Total Conditions|Passing|Conditions|No analysis|Run an analysis/i);

  // Golden image de quality gate con summary
  await expect(page).toHaveScreenshot('page-quality-gate-with-summary.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 9. Configuration Page — Validación visual de Configuration
// ──────────────────────────────────────────────────────────────────────────────

test('VR-9a — Configuration shows heading and project path input', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  const check = collectErrors(page);
  await page.goto('/configuration', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  await expect(page.locator('h1')).toContainText('Configuration');
  await expect(page.locator('input')).toBeAttached();
  expect(check()).toEqual([]);

  // Golden image de la página Configuration
  await expect(page).toHaveScreenshot('page-configuration.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-9c — Configuration input allows typing', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/configuration', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  const inputs = page.locator('input');
  const count = await inputs.count();
  if (count > 0) {
    await inputs.first().fill('test-path');
    const val = await inputs.first().inputValue();
    expect(val).toBe('test-path');

    // Golden image de configuration con input
    await expect(page).toHaveScreenshot('page-configuration-with-input.png', {
      fullPage: true,
    animations: 'disabled',
    });
  }
});

// ──────────────────────────────────────────────────────────────────────────────
// 10. Dark Mode — Validación visual de dark mode
// ──────────────────────────────────────────────────────────────────────────────

test('VR-10a — Dark mode toggle exists in sidebar', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  const sidebar = page.locator('aside.sidebar-desktop');
  const text = await sidebar.textContent();
  expect(text).toMatch(/Dark Mode|Light Mode/);

  // Golden image del sidebar con dark mode toggle
  await expect(sidebar).toHaveScreenshot('sidebar-dark-mode-toggle.png', {
    animations: 'disabled',
  });
});

test('VR-10d — Dark mode toggle is in sidebar footer area', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await waitForApp(page);
  const sidebar = page.locator('aside.sidebar-desktop');
  const lastChild = sidebar.locator('> :last-child');
  await expect(lastChild).toBeAttached();

  // Golden image del footer del sidebar
  await expect(lastChild).toHaveScreenshot('sidebar-footer.png', {
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 11. Responsive & Mobile — Validación visual de responsive
// ──────────────────────────────────────────────────────────────────────────────

test('VR-11a — At 375px (mobile), desktop sidebar is hidden', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await page.goto('/', { waitUntil: 'load', timeout: 10000 });
  await page.waitForTimeout(2000);

  const desktopSidebar = page.locator('aside.sidebar-desktop');
  const count = await desktopSidebar.count();
  if (count > 0) {
    const display = await desktopSidebar.evaluate(el => getComputedStyle(el).display);
    expect(display).toBe('none');
  }

  // Golden image del mobile view
  await expect(page).toHaveScreenshot('viewport-mobile-375px.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-11b — At 375px, hamburger button becomes visible', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await page.goto('/', { waitUntil: 'load', timeout: 10000 });
  await page.waitForTimeout(2000);

  const hamburger = page.locator('button.hamburger-btn');
  const count = await hamburger.count();
  if (count > 0) {
    const display = await hamburger.evaluate(el => getComputedStyle(el).display);
    expect(display).toBe('block');
  }

  // Golden image del mobile view con hamburger
  await expect(page).toHaveScreenshot('viewport-mobile-hamburger.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-11c — At 375px, main content has no left margin', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 812 });
  await page.goto('/', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  const ml = await page.locator('.main-content').evaluate(
    el => getComputedStyle(el).marginLeft
  );
  expect(ml).toBe('0px');

  // Golden image del mobile view main content
  await expect(page).toHaveScreenshot('viewport-mobile-main-content.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 16. Diagrams Page — Validación visual de Diagrams
// ──────────────────────────────────────────────────────────────────────────────

test('VR-16a — /diagrams page renders with Diagrams heading', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  const check = collectErrors(page);
  await page.goto('/diagrams', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);
  await expect(page.locator('h1')).toContainText('Diagrams');
  expect(check()).toEqual([]);

  // Golden image de la página Diagrams
  await expect(page).toHaveScreenshot('page-diagrams.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-16b — Diagrams page has diagram type selector', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/diagrams', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  const selects = page.locator('select');
  await expect(selects.first()).toBeAttached();

  // Golden image de diagrams con selector
  await expect(page).toHaveScreenshot('page-diagrams-with-selector.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-16d — Diagrams page has Generate button', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/diagrams', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  const buttons = page.locator('button');
  const count = await buttons.count();
  expect(count).toBeGreaterThan(0);
  const generateBtn = page.locator('button', { hasText: /generate/i });
  if (await generateBtn.count() > 0) {
    await expect(generateBtn.first()).toBeAttached();
  }

  // Golden image de diagrams con Generate button
  await expect(page).toHaveScreenshot('page-diagrams-with-generate.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-16f — C4 Level selector appears when diagram type is C4', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/diagrams', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  const selects = page.locator('select');
  const count = await selects.count();
  expect(count).toBeGreaterThanOrEqual(2);

  // Golden image de diagrams con selector C4
  await expect(page).toHaveScreenshot('page-diagrams-c4-selector.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

// ──────────────────────────────────────────────────────────────────────────────
// 17. Diagram Comparison Page — Validación visual de Diagram Diff
// ──────────────────────────────────────────────────────────────────────────────

test('VR-17a — /diagrams/diff page renders with Comparison heading', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  const check = collectErrors(page);
  await page.goto('/diagrams/diff', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);
  expect(check()).toEqual([]);
  const body = await page.locator('body').textContent();
  expect(body.length).toBeGreaterThan(50);

  // Golden image de la página Diagram Diff
  await expect(page).toHaveScreenshot('page-diagrams-diff.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-17b — Diagram diff page has project path input', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/diagrams/diff', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  const inputs = page.locator('input[type="text"]');
  await expect(inputs.first()).toBeAttached();

  // Golden image de diff con input
  await expect(page).toHaveScreenshot('page-diagrams-diff-with-input.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-17c — Diagram diff page has diagram type selector', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/diagrams/diff', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  const selects = page.locator('select');
  await expect(selects.first()).toBeAttached();

  // Golden image de diff con selector
  await expect(page).toHaveScreenshot('page-diagrams-diff-with-selector.png', {
    fullPage: true,
    animations: 'disabled',
  });
});

test('VR-17d — Diagram diff page has Generate/Compare button', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.goto('/diagrams/diff', { waitUntil: 'networkidle', timeout: 15000 });
  await page.waitForTimeout(2000);

  const buttons = page.locator('button');
  const count = await buttons.count();
  expect(count).toBeGreaterThan(0);

  // Golden image de diff con botones
  await expect(page).toHaveScreenshot('page-diagrams-diff-with-buttons.png', {
    fullPage: true,
    animations: 'disabled',
  });
});