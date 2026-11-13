import { defineConfig } from '@playwright/test';

if (!process.env.MTG_CLIENT_LIVE_ORIGIN) throw new Error('Set the explicit native or proxy client origin');
export default defineConfig({
  testDir: './tests/live', outputDir: './generated/live-browser-results',
  fullyParallel: false, workers: 1, retries: 0, timeout: 45_000,
  reporter: [['list'], ['json', { outputFile: './generated/live-browser-report.json' }]],
  // Authorization headers must never be written to traces, HARs or videos.
  use: { baseURL: process.env.MTG_CLIENT_LIVE_ORIGIN, viewport: { width: 1280, height: 720 }, trace: 'off', screenshot: 'off', video: 'off' },
  projects: [{ name: 'chromium', use: { browserName: 'chromium' } }, { name: 'firefox', use: { browserName: 'firefox' } }],
});
