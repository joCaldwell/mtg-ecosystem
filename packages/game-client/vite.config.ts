import { defineConfig } from 'vite';

// No API proxy until the bootstrap/upstream identity mapping is committed/tested.
export default defineConfig({
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
