// e2e test suite for CogniCode Dashboard
const { chromium } = require('playwright');
const fs = require('fs');

const BASE = 'http://127.0.0.1:3000';
const REPORT_DIR = '/home/rubentxu/Proyectos/rust/CogniCode/tests/e2e/report';
fs.mkdirSync(REPORT_DIR, { recursive: true });

const results = [];
const screenshots = [];

function addResult(test, status, detail = '') {
  results.push({ test, status, detail, timestamp: new Date().toISOString() });
  console.log(`  ${status === 'PASS' ? '✅' : '❌'} ${test} ${detail ? '- ' + detail : ''}`);
}

async function runTests() {
  const browser = await chromium.launch({ headless: true });
  
  // Helper: create page, collect errors, navigate
  async function openPage(url, waitMs = 2000) {
    const page = await browser.newPage();
    const errors = [];
    page.on('console', msg => { if (msg.type() === 'error') errors.push(msg.text()); });
    page.on('pageerror', err => errors.push(err.message));
    try {
      await page.goto(url, { waitUntil: 'networkidle', timeout: 15000 });
      await page.waitForTimeout(waitMs);
    } catch(e) { errors.push('NAVIGATION ERROR: ' + e.message); }
    return { page, errors };
  }

  // ===== TEST 1: Health Check =====
  try {
    const { page, errors } = await openPage(BASE + '/health');
    const body = await page.textContent('body');
    if (body === 'OK') addResult('Health endpoint', 'PASS');
    else addResult('Health endpoint', 'FAIL', 'Expected OK got: ' + body);
    await page.close();
  } catch(e) { addResult('Health endpoint', 'FAIL', e.message); }

  // ===== TEST 2: Homepage loads =====
  try {
    const { page, errors } = await openPage(BASE + '/');
    const title = await page.title();
    const hasApp = await page.evaluate(() => !!document.querySelector('#app'));
    const hasNav = await page.evaluate(() => !!document.querySelector('nav'));
    if (title === 'CogniCode Dashboard' && hasApp && hasNav) {
      addResult('Homepage loads', 'PASS');
    } else {
      addResult('Homepage loads', 'FAIL', `title=${title} app=${hasApp} nav=${hasNav}`);
    }
    if (errors.length) addResult('Homepage console errors', 'FAIL', errors.join('; '));
    await page.screenshot({ path: REPORT_DIR + '/01-homepage.png', fullPage: true });
    await page.close();
  } catch(e) { addResult('Homepage loads', 'FAIL', e.message); }

  // ===== TEST 3: Projects page =====
  try {
    const { page, errors } = await openPage(BASE + '/projects');
    const title = await page.title();
    const hasHeading = await page.evaluate(() => {
      const h1 = document.querySelector('h1');
      return h1 ? h1.textContent : null;
    });
    if (hasHeading === 'Projects') {
      addResult('Projects page loads', 'PASS');
    } else {
      addResult('Projects page loads', 'FAIL', `heading=${hasHeading}`);
    }
    if (errors.length) addResult('Projects console errors', 'FAIL', errors.join('; '));
    await page.screenshot({ path: REPORT_DIR + '/02-projects.png', fullPage: true });
    await page.close();
  } catch(e) { addResult('Projects page loads', 'FAIL', e.message); }

  // ===== TEST 4: Issues page =====
  try {
    const { page, errors } = await openPage(BASE + '/issues');
    const hasHeading = await page.evaluate(() => {
      const h1 = document.querySelector('h1');
      return h1 ? h1.textContent : null;
    });
    if (hasHeading === 'Issues') {
      addResult('Issues page loads', 'PASS');
    } else {
      addResult('Issues page loads', 'FAIL', `heading=${hasHeading}`);
    }
    if (errors.length) addResult('Issues console errors', 'FAIL', errors.join('; '));
    await page.screenshot({ path: REPORT_DIR + '/03-issues.png', fullPage: true });
    await page.close();
  } catch(e) { addResult('Issues page loads', 'FAIL', e.message); }

  // ===== TEST 5: Metrics page =====
  try {
    const { page, errors } = await openPage(BASE + '/metrics');
    const hasHeading = await page.evaluate(() => {
      const h1 = document.querySelector('h1');
      return h1 ? h1.textContent : null;
    });
    if (hasHeading === 'Metrics') {
      addResult('Metrics page loads', 'PASS');
    } else {
      addResult('Metrics page loads', 'FAIL', `heading=${hasHeading}`);
    }
    if (errors.length) addResult('Metrics console errors', 'FAIL', errors.join('; '));
    await page.screenshot({ path: REPORT_DIR + '/04-metrics.png', fullPage: true });
    await page.close();
  } catch(e) { addResult('Metrics page loads', 'FAIL', e.message); }

  // ===== TEST 6: Quality Gate page =====
  try {
    const { page, errors } = await openPage(BASE + '/quality-gate');
    const hasHeading = await page.evaluate(() => {
      const h1 = document.querySelector('h1');
      return h1 ? h1.textContent : null;
    });
    if (hasHeading === 'Quality Gate') {
      addResult('Quality Gate page loads', 'PASS');
    } else {
      addResult('Quality Gate page loads', 'FAIL', `heading=${hasHeading}`);
    }
    if (errors.length) addResult('Quality Gate console errors', 'FAIL', errors.join('; '));
    await page.screenshot({ path: REPORT_DIR + '/05-quality-gate.png', fullPage: true });
    await page.close();
  } catch(e) { addResult('Quality Gate page loads', 'FAIL', e.message); }

  // ===== TEST 7: Configuration page =====
  try {
    const { page, errors } = await openPage(BASE + '/configuration');
    const hasHeading = await page.evaluate(() => {
      const h1 = document.querySelector('h1');
      return h1 ? h1.textContent : null;
    });
    if (hasHeading === 'Configuration') {
      addResult('Configuration page loads', 'PASS');
    } else {
      addResult('Configuration page loads', 'FAIL', `heading=${hasHeading}`);
    }
    if (errors.length) addResult('Configuration console errors', 'FAIL', errors.join('; '));
    await page.screenshot({ path: REPORT_DIR + '/06-configuration.png', fullPage: true });
    await page.close();
  } catch(e) { addResult('Configuration page loads', 'FAIL', e.message); }

  // ===== TEST 8: 404 page =====
  try {
    const { page, errors } = await openPage(BASE + '/nonexistent-page-12345');
    const has404 = await page.evaluate(() => {
      return document.body.textContent.includes('404') || document.body.textContent.includes('not found');
    });
    if (has404) {
      addResult('404 page shows correctly', 'PASS');
    } else {
      addResult('404 page shows correctly', 'FAIL', 'No 404 content found');
    }
    await page.screenshot({ path: REPORT_DIR + '/07-404.png', fullPage: true });
    await page.close();
  } catch(e) { addResult('404 page shows correctly', 'FAIL', e.message); }

  // ===== TEST 9: SPA navigation =====
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', err => errors.push(err.message));
    await page.goto(BASE + '/', { waitUntil: 'networkidle', timeout: 15000 });
    await page.waitForTimeout(1000);

    // Click Projects nav link
    await page.click('a[href="/projects"]');
    await page.waitForTimeout(1000);
    const projectsHeading = await page.textContent('h1');

    // Click Issues nav link
    await page.click('a[href="/issues"]');
    await page.waitForTimeout(1000);
    const issuesHeading = await page.textContent('h1');

    if (projectsHeading === 'Projects' && issuesHeading === 'Issues') {
      addResult('SPA navigation works', 'PASS');
    } else {
      addResult('SPA navigation works', 'FAIL', `projects=${projectsHeading} issues=${issuesHeading}`);
    }
    if (errors.length) addResult('SPA navigation console errors', 'FAIL', errors.join('; '));
    await page.screenshot({ path: REPORT_DIR + '/08-spa-nav.png', fullPage: true });
    await page.close();
  } catch(e) { addResult('SPA navigation works', 'FAIL', e.message); }

  // ===== TEST 10: CSS loads correctly =====
  try {
    const { page, errors } = await openPage(BASE + '/style/main.css', 500);
    const body = await page.textContent('body');
    if (body.includes('@import') || body.includes('tailwindcss') || body.includes('--color-brand')) {
      addResult('CSS file served correctly', 'PASS');
    } else {
      addResult('CSS file served correctly', 'FAIL', 'CSS content missing');
    }
    await page.close();
  } catch(e) { addResult('CSS file served correctly', 'FAIL', e.message); }

  // ===== TEST 11: Dark mode toggle =====
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on('pageerror', err => errors.push(err.message));
    await page.goto(BASE + '/', { waitUntil: 'networkidle', timeout: 15000 });
    await page.waitForTimeout(1000);

    // Find and click the dark mode toggle button
    const toggleBtn = await page.evaluate(() => {
      const buttons = document.querySelectorAll('button');
      for (const btn of buttons) {
        if (btn.textContent.includes('Dark Mode') || btn.textContent.includes('Light Mode')) {
          return true;
        }
      }
      return false;
    });

    // Click the dark mode button if found
    const dmBtn = await page.$('button:has-text("Dark Mode")');
    if (dmBtn) {
      await dmBtn.click();
      await page.waitForTimeout(500);
      const isLightMode = await page.$('button:has-text("Light Mode")');
      if (isLightMode) {
        addResult('Dark mode toggle works', 'PASS');
      } else {
        addResult('Dark mode toggle works', 'FAIL', 'Toggle did not change');
      }
    } else {
      addResult('Dark mode toggle works', 'WARN', 'Dark mode button not found');
    }
    await page.screenshot({ path: REPORT_DIR + '/09-dark-mode.png', fullPage: true });
    await page.close();
  } catch(e) { addResult('Dark mode toggle works', 'FAIL', e.message); }

  // ===== TEST 12: Sidebar navigation items =====
  try {
    const { page, errors } = await openPage(BASE + '/');
    const navItems = await page.evaluate(() => {
      const links = document.querySelectorAll('nav a, aside a');
      return Array.from(links).map(a => ({ text: a.textContent.trim(), href: a.getAttribute('href') }));
    });
    const expectedItems = ['Projects', 'Dashboard', 'Issues', 'Metrics', 'Quality Gate', 'Configuration'];
    const found = expectedItems.filter(e => navItems.some(n => n.text === e));
    if (found.length === expectedItems.length) {
      addResult('Sidebar has all nav items', 'PASS', found.join(', '));
    } else {
      const missing = expectedItems.filter(e => !navItems.some(n => n.text === e));
      addResult('Sidebar has all nav items', 'FAIL', `Missing: ${missing.join(', ')}`);
    }
    await page.close();
  } catch(e) { addResult('Sidebar has all nav items', 'FAIL', e.message); }

  // ===== TEST 13: WASM binary loads =====
  try {
    const { page, errors } = await openPage(BASE + '/');
    const hasWasm = await page.evaluate(() => {
      return performance.getEntriesByType('resource').some(r => r.name.includes('.wasm'));
    });
    if (hasWasm) {
      addResult('WASM binary loads', 'PASS');
    } else {
      // Check if TrunkApplicationStarted event fired
      const trunkStarted = await page.evaluate(() => {
        return window.wasmBindings !== undefined;
      });
      if (trunkStarted) addResult('WASM binary loads', 'PASS', 'Trunk bindings found');
      else addResult('WASM binary loads', 'FAIL', 'No WASM or trunk bindings detected');
    }
    await page.close();
  } catch(e) { addResult('WASM binary loads', 'FAIL', e.message); }

  // ===== GENERATE REPORT =====
  const passed = results.filter(r => r.status === 'PASS').length;
  const failed = results.filter(r => r.status === 'FAIL').length;
  const warned = results.filter(r => r.status === 'WARN').length;
  const total = results.filter(r => r.status !== 'WARN').length;

  const report = {
    summary: { total, passed, failed, warned, score: Math.round(passed / total * 100) },
    results,
    timestamp: new Date().toISOString(),
  };
  fs.writeFileSync(
    REPORT_DIR + '/report.json',
    JSON.stringify(report, null, 2)
  );
  console.log(`\n=== REPORT ===`);
  console.log(`Total: ${total}, Passed: ${passed}, Failed: ${failed}, Warnings: ${warned}`);
  console.log(`Score: ${report.summary.score}%`);
  console.log(`Report saved to ${REPORT_DIR}/report.json`);

  await browser.close();
}

runTests().catch(e => { console.error('FATAL:', e); process.exit(1); });
