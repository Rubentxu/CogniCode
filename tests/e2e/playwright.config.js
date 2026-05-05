// Playwright configuration for CogniCode Dashboard e2e tests

const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
  testDir: '.',
  timeout: 45000,
  retries: 0,
  reporter: [
    ['list'],
    ['json', { outputFile: 'report/results.json' }],
    ['html', { outputFolder: 'report/html', open: 'never' }],
  ],
  use: {
    baseURL: process.env.DASHBOARD_URL || 'http://127.0.0.1:3000',
    headless: true,
    viewport: { width: 1400, height: 900 },
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { browserName: 'chromium' },
    },
  ],
  webServer: process.env.SKIP_SERVER ? undefined : {
    command: 'DIST_DIR=dist ../../target/debug/cognicode-dashboard-server',
    url: 'http://127.0.0.1:3000/health',
    timeout: 10000,
    reuseExistingServer: true,
  },
});
