// Screenshot capture for documentation
// Run via: just docs-screenshots
// Requires server running on PORT (default 3000)

const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const PORT = process.env.PORT || '3000';
const BASE = `http://127.0.0.1:${PORT}`;
const OUT_DIR = path.join(__dirname, '..', '..', 'docs', 'images');

const PAGES = [
  ['01-dashboard', '/'],
  ['02-projects', '/projects'],
  ['04-issues', '/issues'],
  ['05-metrics', '/metrics'],
  ['06-quality-gate', '/quality-gate'],
  ['08-configuration', '/configuration'],
];

(async () => {
  fs.mkdirSync(OUT_DIR, { recursive: true });
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();

  for (const [name, url] of PAGES) {
    try {
      await page.goto(BASE + url, { waitUntil: 'load', timeout: 15000 });
      await page.waitForTimeout(2000);
      await page.screenshot({ path: path.join(OUT_DIR, name + '.png'), fullPage: true });
      console.log('OK:', name);
    } catch (e) {
      console.log('FAIL:', name, e.message);
    }
  }

  await browser.close();
  console.log('\nDone. Screenshots saved to', OUT_DIR);
})().catch(e => console.error('FATAL:', e));
