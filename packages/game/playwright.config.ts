import { defineConfig } from '@playwright/test';

const port = Number(process.env.MTG_CLIENT_FIXTURE_PORT ?? 4179);
if (!Number.isInteger(port) || port < 1024 || port > 65535) throw new Error('Invalid explicit fixture port');
export default defineConfig({
  testDir: './tests/browser',
  outputDir: './generated/browser-results',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  reporter: [['list'], ['json', { outputFile: './generated/browser-report.json' }]],
  use: { baseURL: `http://127.0.0.1:${port}`, viewport: { width: 1280, height: 720 }, trace: 'retain-on-failure' },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }, { name: 'firefox', use: { browserName: 'firefox' } }],
  webServer: { command: `npm run build && npm exec -- vite preview --port ${port}`, url: `http://127.0.0.1:${port}`, reuseExistingServer: false, timeout: 30_000 },
});
