// Explicit already-running preview only. Uses node:http so adversarial Host
// headers reach the listener (native fetch may normalize/ignore them).
import { request } from 'node:http';
import { writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';
const results = [];
for (const [path, options, expected] of [
  ['/join', {}, 200], ['/client-config.json', {}, 200],
  ['/client-config.json', { headers: { host: 'evil.test:4179' } }, 403],
  ['/client-config.json', { headers: { origin: 'http://evil.test' } }, 403],
  ['/client-config.json', { headers: { 'sec-fetch-site': 'cross-site' } }, 403],
  ['/v1/matches/a1/resume', {}, 401],
  ['/v1/matches/a1/resume', { method: 'POST' }, 404],
  ['/v1/matches/a1/resume?viewer=1', {}, 404],
  ['/v1/matches/a1/commands', { method: 'POST' }, 404],
  ['/presentation/secret.json', {}, 404], ['/generated/private.json', {}, 404],
]) {
  const result = await new Promise((resolve, reject) => {
    const req = request(`http://127.0.0.1:4179${path}`, options, response => {
      response.resume(); response.on('end', () => resolve({ status: response.statusCode, cors: response.headers['access-control-allow-origin'] }));
    }); req.on('error', reject); req.end();
  });
  assert.equal(result.status, expected, path); assert.equal(result.cors, undefined);
  results.push({ path, status: result.status });
}
writeFileSync(new URL('../generated/proxy-security.json', import.meta.url), JSON.stringify({ checks: results }, null, 2) + '\n');
console.log('11 proxy route/security checks passed');
