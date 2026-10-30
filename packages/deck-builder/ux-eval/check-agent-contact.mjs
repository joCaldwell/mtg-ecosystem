// Against a disposable ux-eval server only: read contact, tenant isolation,
// live updates, and the absence of expensive data refetches on MCP reads.
import assert from 'node:assert/strict';
import { chromium } from 'playwright-core';
import { Client, StreamableHTTPClientTransport } from '@modelcontextprotocol/client';

const base = process.env.UX_EVAL_URL ?? 'http://127.0.0.1:8879';
assert.equal(new URL(base).hostname, '127.0.0.1');
const browser = await chromium.launch({ headless: true });
const client = new Client({ name: 'audit-reader', version: '1' });
let credential;
const api = async (path, method = 'GET', body, user = 3) => {
  const response = await fetch(base + path, { method, headers: { Cookie: `uxeval_user=${user}`, Origin: base, 'Content-Type': 'application/json' }, body: body === undefined ? undefined : JSON.stringify(body) });
  assert.equal(response.ok, true, `${method} ${path}: ${response.status}`);
  return response.json();
};
try {
  await api("/api/settings", "PUT", { sidebar_collapsed: false });
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } });
  await context.addCookies([{ name: 'uxeval_user', value: '3', url: base }]);
  const page = await context.newPage();
  await page.clock.install({ time: new Date('2026-10-20T17:00:00Z') });
  const requests = [];
  page.on('request', r => { if (r.url().includes('/api/')) requests.push(new URL(r.url()).pathname); });
  await page.goto(base + '/deck/7');
  const sidebar = page.getByRole('complementary', { name: 'Sidebar' });
  await sidebar.getByRole('link', { name: /^Connect an agent\./ }).waitFor();
  assert.equal(requests.includes('/api/settings'), false);
  credential = await api('/api/settings/tokens', 'POST', { label: 'Audit read-only client' });
  await sidebar.getByText('No agent requests yet', { exact: true }).waitFor();
  await client.connect(new StreamableHTTPClientTransport(new URL(base + '/mcp'), { requestInit: { headers: { Authorization: `Bearer ${credential.token}` } } }));
  await sidebar.getByRole('link', { name: /Last request: Audit read-only client/ }).waitFor();
  await client.callTool({ name: 'start_session', arguments: {} });
  assert.equal((await api('/api/preferences')).has_agent_activity, false);
  assert.ok(!(await api('/api/agents', 'GET', undefined, 2)).some(c => c.label === 'Audit read-only client'));
  // Observe a complete MCP request and its connection-status fetch.
  requests.length = 0;
  const refresh = page.waitForResponse(r => new URL(r.url()).pathname === '/api/agents');
  const deck = await client.callTool({ name: 'get_deck', arguments: { deck: 7, sections: ['state'] } });
  assert.ok(deck.content.some(c => c.type === 'text' && c.text.includes('Computed state')));
  await refresh;
  assert.ok(!requests.some(path => path === '/api/settings' || path === '/api/preferences' || path.startsWith('/api/decks')));
  if (process.env.UX_EVAL_SCREENSHOT) await page.screenshot({ path: process.env.UX_EVAL_SCREENSHOT });
  await page.clock.fastForward(6 * 60_000);
  await sidebar.getByRole('link', { name: /Last request: Audit read-only client · 6 min ago/ }).waitFor();
  await page.getByRole('button', { name: 'Collapse sidebar' }).click();
  await page.getByRole('button', { name: 'Expand sidebar' }).waitFor();
  await sidebar.getByRole('link', { name: /Last request: Audit read-only client/ }).waitFor();
  await api(`/api/settings/tokens/${credential.credential.id}`, 'DELETE');
  credential = undefined;
  await sidebar.getByRole('link', { name: /^Connect an agent\./ }).waitFor();
  console.log('PASS: read-only contact, tenant isolation, live sidebar, elapsed time, collapsed access, revocation, and no data refetch on MCP reads.');
} finally {
  if (credential) await api(`/api/settings/tokens/${credential.credential.id}`, 'DELETE');
  await client.close();
  await browser.close();
}
