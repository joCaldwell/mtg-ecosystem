import { defineConfig, type Plugin, type Connect, type ProxyOptions } from 'vite';
import type { ClientConfig } from '@multiverse/game-server-sdk';
import { upstreamOrigin, localRequestAllowed, readRouteAllowed } from './scripts/proxy-policy.ts';
import { mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { relative } from 'node:path';

// Configured by the local operator, never derived from a credential file.
const upstream = process.env.MTG_CLIENT_UPSTREAM ? upstreamOrigin(process.env.MTG_CLIENT_UPSTREAM) : null;
const csp = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'";
const headers = { 'Content-Security-Policy': csp, 'Referrer-Policy': 'no-referrer', 'X-Content-Type-Options': 'nosniff' };
function guard(port: number, production: boolean): Connect.NextHandleFunction {
  return (req, res, next) => {
    const origin = req.headers.origin;
    const site = req.headers['sec-fetch-site'];
    if (!localRequestAllowed(req.headers.host, origin, typeof site === 'string' ? site : undefined, port)) {
      res.writeHead(403, { 'Cache-Control': 'no-store' }); res.end(); return;
    }
    if (req.url === '/client-config.json') {
      if (req.method !== 'GET' || !upstream) { res.writeHead(503, { 'Cache-Control': 'no-store' }); res.end(); return; }
      res.writeHead(200, { ...headers, 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
      res.end(JSON.stringify({ client_config_version: 1, server_origin: upstream } satisfies ClientConfig)); return;
    }
    if (req.url?.startsWith('/v1/') || req.url?.startsWith('/presentation/')) {
      if (!upstream || !readRouteAllowed(req.method, req.url)) { res.writeHead(404, { 'Cache-Control': 'no-store' }); res.end(); return; }
      next(); return;
    }
    const pages = ['/', '/join', '/table', '/about', '/fixtures/opening', '/fixtures/missing-metadata'];
    if (production && (!['GET', 'HEAD'].includes(req.method ?? '') || !(pages.includes(req.url ?? '')
      || /^\/assets\/[a-zA-Z0-9._-]+\.(js|css|woff2|png|webp|jpe?g)$/.test(req.url ?? '')))) {
      // Vite's generic SPA fallback otherwise returns index.html for arbitrary
      // private-looking paths. Mirror the native host's explicit SPA surface.
      res.writeHead(404, { 'Cache-Control': 'no-store' }); res.end(); return;
    }
    next();
  };
}
const delivery: Plugin = {
  name: 'local-read-only-delivery',
  configureServer(server) { server.middlewares.use(guard(server.config.server.port ?? 5173, false)); },
  configurePreviewServer(server) { server.middlewares.use(guard(server.config.preview.port ?? 4179, true)); },
};
function proxy(): Record<string, ProxyOptions> {
  if (!upstream) return {};
  return { '^/(v1/|presentation/)': {
    target: upstream, changeOrigin: true, timeout: 30_000, proxyTimeout: 30_000,
    configure(server) {
      server.on('proxyReq', request => {
        // Only after guard validated the incoming local origin. Native host checks
        // the rewritten origin against its own explicitly configured listener.
        request.setHeader('origin', new URL(upstream).origin);
        request.removeHeader('cookie'); request.removeHeader('referer');
      });
    },
  } };
}
export default defineConfig({
  plugins: [delivery, {
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
          || local === '../game-server/sdk/dist/browser.js'
          || /^fixtures\/foundations-opening\/(opening-seat0-http|presentation)\.json$/.test(local))) {
          throw new Error(`Unreviewed browser runtime module: ${local}`);
        }
        report.push(local);
      }
      await mkdir(new URL('./generated/', import.meta.url), { recursive: true });
      await writeFile(new URL('./generated/bundle-inputs.json', import.meta.url), JSON.stringify({ runtime_inputs: report }, null, 2) + '\n');
    },
  }],
  server: { host: '127.0.0.1', port: 5173, strictPort: true, cors: false, proxy: proxy(), fs: { deny: ['.env', '.env.*', '*.{crt,pem}', '**/.git/**', '**/generated/**', '**/private/**'] }, headers: { ...headers, 'Content-Security-Policy': csp.replace("style-src 'self'", "style-src 'self' 'nonce-multiverse-local-dev'") } },
  // Vite injects this nonce for development CSS. Production remains external-only.
  html: { cspNonce: 'multiverse-local-dev' },
  preview: { host: '127.0.0.1', port: 4179, strictPort: true, cors: false, headers, proxy: proxy() },
  build: { target: 'es2023' },
});
