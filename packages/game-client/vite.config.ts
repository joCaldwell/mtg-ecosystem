import { defineConfig } from 'vite';
import { mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { relative } from 'node:path';

// No API proxy until the bootstrap/upstream identity mapping is committed/tested.
export default defineConfig({
  plugins: [{
    name: 'client-runtime-boundary',
    async generateBundle() {
      const root = fileURLToPath(new URL('.', import.meta.url));
      const inputs = [...this.getModuleIds()].sort();
      const allowedRuntime = /^node_modules\/(react\/|react-dom\/|scheduler\/|react-router\/|cookie-es\/|@remix-run\/route-pattern\/)/;
      const report: string[] = [];
      for (const id of inputs) {
        if (id.includes('browser-external') || id.startsWith('node:')) throw new Error('Node runtime entered browser bundle');
        if (id.startsWith('\0')) continue;
        const local = relative(root, id).replaceAll('\\', '/');
        if (!(local === 'index.html' || local.startsWith('src/') || allowedRuntime.test(local)
          || /^fixtures\/foundations-opening\/(opening-seat0-http|presentation)\.json$/.test(local))) {
          throw new Error(`Unreviewed browser runtime module: ${local}`);
        }
        report.push(local);
      }
      await mkdir(new URL('./generated/', import.meta.url), { recursive: true });
      await writeFile(new URL('./generated/bundle-inputs.json', import.meta.url), JSON.stringify({ runtime_inputs: report }, null, 2) + '\n');
    },
  }],
  server: { host: '127.0.0.1', strictPort: true },
  preview: {
    host: '127.0.0.1', port: 4179, strictPort: true,
    headers: {
      // Fixture proof only. Live same-origin API policy belongs to integration.
      'Content-Security-Policy': "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'",
      'Referrer-Policy': 'no-referrer',
      'X-Content-Type-Options': 'nosniff',
    },
  },
  build: { target: 'es2023' },
});
