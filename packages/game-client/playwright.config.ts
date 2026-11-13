import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests/browser',
  outputDir: './generated/browser-results',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list'], ['json', { outputFile: './generated/browser-report.json' }]],
  use: { baseURL: 'http://127.0.0.1:4179', viewport: { width: 1280, height: 720 }, trace: 'retain-on-failure' },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }, { name: 'firefox', use: { browserName: 'firefox' } }],
  webServer: { command: 'npm run build && npm exec -- vite preview', url: 'http://127.0.0.1:4179', reuseExistingServer: false, timeout: 30_000 },
});
